import React, { useEffect, useRef, useState } from 'react';
import { createRoot } from 'react-dom/client';
import { Annotation, EditorState } from '@codemirror/state';
import { EditorView, drawSelection, keymap, lineNumbers, highlightActiveLine } from '@codemirror/view';
import { defaultKeymap, indentWithTab } from '@codemirror/commands';
import { syntaxHighlighting, defaultHighlightStyle, bracketMatching } from '@codemirror/language';
import { markdown } from '@codemirror/lang-markdown';
import { Engine, ready, randomId, now, type Update, type Frame } from './engine';
import { Mesh, type Person } from './network/rtc';
import { cursors, presence } from './editor/presence';
import './style.css';
import { save as saveBackup } from './storage/local';
const remote = Annotation.define<boolean>();
const palette = ['#a3492e', '#306b92', '#7e4d92', '#577d32', '#aa5b81', '#208278', '#8c6a28'];
function roomId() {
  let room = location.hash.slice(1);
  if (!/^[a-f0-9]{32}$/i.test(room)) { room = randomId(); history.replaceState(null, '', `#${room}`); }
  return room.toLowerCase();
}
function download(name: string, bytes: BlobPart, type = 'text/plain') {
  const url = URL.createObjectURL(new Blob([bytes], { type }));
  const link = document.createElement('a'); link.href = url; link.download = name; link.click(); setTimeout(() => URL.revokeObjectURL(url), 1_000);
}
function App() {
  const [room] = useState(roomId), [writer] = useState(randomId);
  const [name, setName] = useState('Untitled document');
  const [people, setPeople] = useState<Person[]>([]), [connection, setConnection] = useState('Connecting');
  const [saved, setSaved] = useState('Loading local document'), [error, setError] = useState('');
  const [offline, setOffline] = useState(false);
  const [notice, setNotice] = useState(''), [diagnostics, setDiagnostics] = useState(false);
  const [length, setLength] = useState(0);
  const container = useRef<HTMLDivElement>(null), view = useRef<EditorView | null>(null), engine = useRef<Engine | null>(null);
  const worker = useRef<Worker | null>(null), mesh = useRef<Mesh | null>(null), serial = useRef(0);
  const peers = useRef<Person[]>([]), cursorState = useRef(new Map<string, { bytes: number[]; seen: number }>());
  useEffect(() => {
    let stopped = false;
    const host = new Worker(new URL('./worker/core.ts', import.meta.url), { type: 'module' }); worker.current = host;
    const participant = sessionStorage.getItem('conv-name') ?? `Writer ${writer.slice(0, 4)}`;
    sessionStorage.setItem('conv-name', participant);
    const network = new Mesh(room, writer, participant); mesh.current = network;
    const refreshCursors = () => {
      if (!engine.current || !view.current) return;
      const values = [];
      for (const [id, cursor] of cursorState.current) {
        if (Date.now() - cursor.seen > 10_000) { cursorState.current.delete(id); continue; }
        const index = peers.current.findIndex(p => p.id === id);
        if (index < 0) continue;
        try { values.push({ position: engine.current.resolve_anchor(new Uint8Array(cursor.bytes)), name: peers.current[index].name, color: palette[index % palette.length] }); } catch { /* dependency has not arrived yet */ }
      }
      view.current.dispatch({ effects: cursors.of(values) });
    };
    network.onPeople = list => {
      peers.current = list; setPeople(list);
      host.postMessage({ type: 'peers', ids: list.filter(p => p.connected).map(p => p.id) });
    };
    network.onStatus = setConnection;
    network.onFrame = frame => host.postMessage({ type: 'receive', frame });
    network.onCursor = (id, bytes) => { cursorState.current.set(id, { bytes, seen: Date.now() }); refreshCursors(); };
    const paint = (text: string, selection?: { anchor: number; head: number }) => {
      const editor = view.current; if (!editor) return;
      const before = editor.state.doc.toString();
      if (before !== text) {
        let start = 0, oldEnd = before.length, newEnd = text.length;
        while (start < oldEnd && start < newEnd && before[start] === text[start]) start++;
        if (start > 0 && /[\uD800-\uDBFF]/.test(before[start - 1])) start--;
        while (oldEnd > start && newEnd > start && before[oldEnd - 1] === text[newEnd - 1]) { oldEnd--; newEnd--; }
        if (oldEnd < before.length && /[\uDC00-\uDFFF]/.test(before[oldEnd])) { oldEnd++; newEnd++; }
        editor.dispatch({ changes: { from: start, to: oldEnd, insert: text.slice(start, newEnd) }, annotations: remote.of(true), selection });
      }
      setLength(text.length); refreshCursors();
    };
    host.onmessage = async ({ data }) => {
      if (stopped) return;
      try {
        if (data.type === 'ready') {
          await ready;
          if (stopped) return;
          const local = new Engine(room, writer, data.backup); local.editor_lane(); local.connect([writer]); engine.current = local;
          setName(data.name); setSaved('Saved on this device');
          const initial = sessionStorage.getItem(`conv-import:${room}`);
          const editor = new EditorView({ parent: container.current!, state: EditorState.create({ doc: local.text(), extensions: [
            lineNumbers(), drawSelection(), highlightActiveLine(), keymap.of([{ key: 'Mod-z', run: () => true }, { key: 'Mod-Shift-z', run: () => true }, { key: 'Mod-y', run: () => true }, ...defaultKeymap, indentWithTab]),
            syntaxHighlighting(defaultHighlightStyle), bracketMatching(), markdown(), presence, EditorView.lineWrapping,
            EditorView.contentAttributes.of({ 'aria-label': 'Document editor', spellcheck: 'true' }),
            EditorView.updateListener.of(update => {
              if (update.docChanged && !update.transactions.some(t => t.annotation(remote))) {
                const changes: { from: number; to: number; text: string }[] = [];
                update.changes.iterChanges((from, to, _c, _d, insert) => changes.push({ from, to, text: insert.toString() }));
                const frames: Frame[] = [];
                try {
                  for (const change of changes.reverse()) frames.push(...(local.edit(change.from, change.to, change.text) as Update).outbound);
                  setSaved('Saving locally'); setLength(update.state.doc.length);
                  host.postMessage({ type: 'local', frames, serial: ++serial.current });
                } catch (failure) { setError(`Edit failed: ${String(failure)}`); }
              }
              if (update.selectionSet) { try { network.cursor(Array.from(local.anchor(update.state.selection.main.head))); } catch { /* invalid composition boundary */ } }
            }),
          ] }) });
          view.current = editor; setLength(editor.state.doc.length);
          editor.dom.addEventListener('compositionend', () => setTimeout(() => host.postMessage({ type: 'snapshot' }), 0));
          if (initial !== null) { sessionStorage.removeItem(`conv-import:${room}`); editor.dispatch({ changes: { from: 0, insert: initial } }); }
          network.start().catch(failure => setError(String(failure)));
        } else if (data.type === 'saved' && data.serial === serial.current) setSaved('Saved on this device');
        else if (data.type === 'outbound') for (const frame of data.frames) network.send(frame.to, new Uint8Array(frame.data));
        else if (data.type === 'broadcast') for (const frame of data.frames) network.broadcast(new Uint8Array(frame.data));
        else if ((data.type === 'remote' || data.type === 'snapshot') && engine.current) {
          if (view.current?.composing) return;
          const cursor = view.current?.state.selection.main;
          const anchor = cursor && engine.current.anchor(cursor.anchor), head = cursor && engine.current.anchor(cursor.head);
          let text: string;
          if (data.type === 'snapshot') { engine.current.merge_backup(data.backup); text = engine.current.text(); }
          else text = (engine.current.receive(new Uint8Array(data.frame), now()) as Update).text;
          paint(text, anchor && head ? { anchor: engine.current.resolve_anchor(anchor), head: engine.current.resolve_anchor(head) } : undefined);
        }
        else if (data.type === 'tick-needed') host.postMessage({ type: 'tick' });
        else if (data.type === 'error') { setError(data.message); setSaved('Local save failed'); }
      } catch (failure) { setError(String(failure)); }
    };
    host.postMessage({ type: 'init', room, writer });
    const heartbeat = setInterval(() => {
      refreshCursors();
      if (engine.current && view.current) { try { network.cursor(Array.from(engine.current.anchor(view.current.state.selection.main.head))); } catch { /* composing */ } }
    }, 1_000);
    return () => { stopped = true; clearInterval(heartbeat); host.terminate(); network.close(); view.current?.destroy(); engine.current?.free(); };
  }, [room, writer]);
  async function invite() { try { await navigator.clipboard.writeText(location.href); setNotice('Invite link copied'); } catch { setNotice('Copy the address in your browser to invite someone'); } }
  async function importText(file?: File) {
    if (!file) return;
    const next = randomId(); sessionStorage.setItem(`conv-import:${next}`, await file.text()); location.hash = next; location.reload();
  }
  async function restoreBackup(file?: File) {
    if (!file) return;
    try {
      if (file.size > 64 * 1024 * 1024) throw new Error('Backup exceeds 64 MiB');
      const value = JSON.parse(await file.text());
      if (value.version !== 1 || !/^[a-f0-9]{32}$/i.test(value.room) || !Array.isArray(value.backup) || !value.backup.every((n: unknown) => Number.isInteger(n) && Number(n) >= 0 && Number(n) <= 255)) throw new Error('Invalid ConV backup');
      await ready;
      const bytes = new Uint8Array(value.backup);
      const check = new Engine(value.room, randomId(), bytes); check.free();
      await saveBackup({ key: `${value.room}:import-${randomId()}`, room: value.room, name: String(value.name).slice(0, 120), backup: bytes });
      location.hash = value.room; location.reload();
    } catch (failure) { setError(String(failure)); }
  }
  return <div className="app">
    <header><a className="wordmark" href="/" aria-label="Create a new ConV document">ConV<span>↗</span></a><span className="tagline">A shared space for your words.</span><button className="invite" onClick={invite}>Invite someone <span>↗</span></button></header>
    <div className="workspace"><aside><div className="eyebrow">YOUR SPACE</div><button className="new" onClick={() => { location.hash = randomId(); location.reload(); }}>+ New document</button><div className="document-item">▤ <span>{name}</span></div><div className="aside-bottom"><span className="dot"/> Local-first. Yours to keep.<p>Saved on your device.<br/>Shared directly with your peers.</p></div></aside>
    <main><div className="document-toolbar"><div className="breadcrumbs">Workspace <span>/</span> Plain text</div><div className="tools"><label className="file-button">Import<input type="file" accept="text/*,.md" onChange={e => importText(e.target.files?.[0])}/></label><label className="file-button">Restore<input type="file" accept=".conv.json,application/json" onChange={e => restoreBackup(e.target.files?.[0])}/></label><button onClick={() => download(`${name}.txt`, view.current?.state.doc.toString() ?? '')}>Export text</button><button onClick={() => { if (engine.current) download(`${name}.conv.json`, JSON.stringify({ version: 1, room, name, backup: Array.from(engine.current.backup()) }), 'application/json'); }}>Backup</button></div></div>
    <section className="writing"><div className="document-heading"><span className="eyebrow">MAKE ROOM FOR AN IDEA</span><input aria-label="Document name" className="title" value={name} maxLength={120} onChange={e => setName(e.target.value)} onBlur={() => worker.current?.postMessage({ type: 'name', name })}/><div className="save-status"><span className="dot"/>{saved}</div></div><div className="editor" ref={container}/></section>
    <footer><span>{length.toLocaleString()} UTF-16 units</span><span>Plain text · Markdown</span><button onClick={() => setDiagnostics(!diagnostics)}>Connection details</button></footer>{diagnostics && <div className="diagnostics"><strong>{connection}</strong><p>{people.filter(p => p.connected).length} direct peer connections. Each device saves its own copy. A room link is an invitation, not a cloud backup.</p><code>Room {room.slice(0, 8)}…</code><p><button onClick={() => { if (offline) mesh.current?.resume(); else mesh.current?.pause(); setOffline(!offline); }}>{offline ? 'Reconnect' : 'Work offline'}</button></p></div>}</main>
    <nav className="participants" aria-label="Participants"><div className="eyebrow">IN THIS DOCUMENT</div><div className="person"><span className="avatar">Y</span><div>You<span>Writing locally</span></div><span className="dot"/></div>{people.map((person, i) => <div className="person" key={person.id}><span className="avatar" style={{ background: palette[i % palette.length], color: '#fff' }}>{person.name.slice(0, 1)}</span><div>{person.name}<span>{person.connected ? (person.relay ? 'Connected via relay' : 'Connected') : 'Connecting'}</span></div></div>)}<div className="invite-note"><h3>Better, together.</h3><p>Share a link to bring someone into the same document.</p><button onClick={invite}>Copy invite link ↗</button></div><p className="connection">{navigator.onLine ? connection : 'Offline'}</p></nav></div>
    {notice && <div className="toast" role="status" onClick={() => setNotice('')}>{notice}</div>}{error && <div className="error" role="alert">{error}</div>}
  </div>;
}
createRoot(document.getElementById('root')!).render(<App/>);

if (import.meta.env.PROD && 'serviceWorker' in navigator) navigator.serviceWorker.register('/sw.js').catch(console.error);
