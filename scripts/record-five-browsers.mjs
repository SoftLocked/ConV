// Five isolated browser replicas, captured live together. No mirrored document DOM.
import { chromium, expect } from '@playwright/test';
import { spawn, execFileSync } from 'node:child_process';
import { once } from 'node:events';
import { mkdir, readFile, rename, writeFile } from 'node:fs/promises';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';

const root = fileURLToPath(new URL('../', import.meta.url));
const output = new URL('../demo/', import.meta.url);
const countArgument = process.argv.indexOf('--participants');
const participantCount = countArgument < 0 ? 5 : Number(process.argv[countArgument + 1]);
if (!Number.isInteger(participantCount) || participantCount < 5 || participantCount > 50) throw new Error('Use 5..50 participants');
const organic = process.argv.includes('--organic');
const artifact = organic ? `organic-${participantCount}-browsers` : 'five-browsers';
const names = ['Alice', 'Bob', 'Carol', 'Dev', 'Eve', ...Array.from({ length: participantCount - 5 }, (_, i) => `Reviewer ${String(i + 6).padStart(2, '0')}`)];
const colors = ['#a75035', '#316d91', '#855792', '#557b39', '#208479'];
const children = [];
const checkpoints = [];
const delay = ms => new Promise(resolve => setTimeout(resolve, ms));
const source = execFileSync('git', ['rev-parse', 'HEAD'], { cwd: root }).toString().trim();
const dirty = !!execFileSync('git', ['status', '--porcelain', '--untracked-files=no'], { cwd: root }).length;
await mkdir(output, { recursive: true });
function start(command, args, options = {}) {
  const child = spawn(command, args, { cwd: root, stdio: ['ignore', 'ignore', 'pipe'], ...options });
  child.diagnostics = '';
  child.stderr.on('data', bytes => { child.diagnostics = (child.diagnostics + bytes).slice(-8_000); });
  children.push(child);
  return child;
}
async function stop(child) {
  if (child.exitCode !== null || child.signalCode !== null) return;
  const done = once(child, 'exit'); child.kill('SIGTERM'); await done;
}
async function wait(url) {
  for (let n = 0; n < 150; n++) {
    for (const child of children) if (child.exitCode !== null) throw new Error(child.diagnostics);
    try { if ((await fetch(url)).ok) return; } catch {}
    await delay(100);
  }
  throw new Error(`Server did not start: ${url}`);
}

let browser, capture, stopCapture = false, recording;
try {
  start('target/debug/conv-signaling', [], { env: { ...process.env, CONV_SIGNAL_BIND: '127.0.0.1:3006', CONV_ORIGIN: 'http://127.0.0.1:5176' } });
  start(process.execPath, ['node_modules/vite/bin/vite.js', '--host', '127.0.0.1', '--port', '5176', '--strictPort'], {
    cwd: root + 'web', env: { ...process.env, VITE_SIGNAL_URL: 'ws://127.0.0.1:3006/signal', VITE_ICE_URL: 'http://127.0.0.1:3006/ice' },
  });
  await wait('http://127.0.0.1:3006/health'); await wait('http://127.0.0.1:5176');
  browser = await chromium.launch();
  const contexts = await Promise.all(names.map(() => browser.newContext({ viewport: { width: 610, height: 360 } })));
  for (let i = 0; i < names.length; i++) await contexts[i].addInitScript(name => sessionStorage.setItem('conv-name', name), names[i]);
  const pages = await Promise.all(contexts.map(context => context.newPage()));
  const room = crypto.randomUUID().replaceAll('-', '');
  const editors = pages.map(page => page.getByRole('textbox', { name: 'Document editor' }));
  for (let index = 0; index < pages.length; index += 5) await Promise.all(pages.slice(index, index + 5).map(async page => {
    await page.goto(`http://127.0.0.1:5176/#${room}`);
    await expect(page.getByRole('textbox', { name: 'Document editor' })).toBeVisible({ timeout: 30_000 });
  }));
  await Promise.all(editors.map(editor => expect(editor).toBeVisible()));
  await Promise.all(pages.map(page => page.getByRole('button', { name: 'Connection details' }).click()));
  await expect.poll(async () => Promise.all(pages.map(page => page.locator('.diagnostics').getAttribute('data-participants'))), { timeout: 45_000 }).toEqual(pages.map(() => String(participantCount)));
  await expect.poll(async () => (await Promise.all(pages.map(page => page.locator('.diagnostics').getAttribute('data-direct-links')))).every(value => Number(value) >= 2 && Number(value) <= 8), { timeout: 45_000 }).toBe(true);
  const degrees = await Promise.all(pages.map(page => page.locator('.diagnostics').getAttribute('data-direct-links').then(Number)));
  await Promise.all(pages.map(page => page.getByRole('button', { name: 'Connection details' }).click()));
  // Compact presentation only; each image is a screenshot of its real app/engine.
  for (const page of pages) {
    await page.addStyleTag({ content: `
      header{height:43px;padding:0 15px}.wordmark{font-size:24px}.invite{padding:7px 10px;font-size:10px}
      .document-toolbar{height:31px;padding:0 15px}.tools{gap:11px;font-size:9px}
      .writing{padding:12px 15px;min-height:0}.document-heading>.eyebrow{display:none}
      .title{font-size:22px;margin:0 0 5px}.save-status{font-size:9px}
      .editor{margin-top:12px;min-height:210px}.cm-editor{font-size:14px;line-height:1.55}.cm-scroller{max-height:210px;overflow:auto}
      .workspace{min-height:0}footer{height:27px}.diagnostics{padding:8px 15px}
    ` });
    await page.getByRole('textbox', { name: 'Document name' }).fill(organic ? 'A garden, together' : 'Launch notes');
    await page.getByRole('textbox', { name: 'Document name' }).press('Tab');
  }
  const recorder = await browser.newContext({ viewport: { width: 1920, height: 1080 }, recordVideo: { dir: fileURLToPath(output), size: { width: 1920, height: 1080 } } });
  const board = await recorder.newPage();
  await board.setContent(`<!doctype html><html><head><title>ConV — live collaboration</title><style>
    *{box-sizing:border-box}body{margin:0;background:#142a20;color:#edf1e8;font:18px/1.45 system-ui,sans-serif;padding:25px 28px}
    header{display:flex;align-items:center;gap:22px;margin-bottom:8px}h1{font:500 34px Georgia,serif;margin:0}header span{color:#b7c9b9;font-size:16px}
    #phase{font-size:23px;margin:8px 0 18px;min-height:34px}.grid{display:grid;grid-template-columns:repeat(3,1fr);gap:16px}
    article{background:#faf9f5;border:3px solid transparent;border-radius:9px;overflow:hidden;box-shadow:0 8px 24px #0002}
    article.active{border-color:#e8be6f}article.offline{border-color:#be7357}.label{display:flex;align-items:center;gap:10px;padding:9px 14px;color:#263c2e;font-size:17px;font-weight:600;height:43px;background:#e9eee3}
    .dot{height:11px;width:11px;border-radius:50%}.badge{margin-left:auto;font-size:12px;font-weight:400;color:#60705d}img{display:block;width:100%;aspect-ratio:610/360;background:#faf9f5}
    .story{padding:22px 25px;background:#223c2d;color:#edf1e8;border-color:#4b6550}.story h2{font:500 28px Georgia,serif;margin:0 0 12px}.story p{font-size:17px;color:#bfcebd;margin:12px 0}
    #checks{padding:0;list-style:none;font-size:16px;line-height:1.7}#checks li{margin-top:9px}#checks li::before{content:'✓ ';color:#c4dcab}
    .note{color:#a7bda7;font-size:14px;margin:16px 0 0}code{font-size:15px;color:#d9e7cd}
  </style></head><body><header><h1>ConV · ${participantCount} real browser sessions</h1><span>Five live views · ${participantCount - 5} background sessions · peer-to-peer WebRTC</span></header>
  <div id="phase">One shared draft. Every session runs its own editor and replica.</div><div class="grid">
  ${names.slice(0, 5).map((name, i) => `<article id="tile-${i}"><div class="label"><span class="dot" style="background:${colors[i]}"></span>${name}<span class="badge" id="badge-${i}">Live browser view</span></div><img id="view-${i}" alt="${name}'s live browser"></article>`).join('')}
  <article class="story"><h2>${organic ? 'A shared paragraph. Five perspectives.' : 'Changes travel peer to peer.'}</h2><p>${organic ? 'Watch concurrent writing, deletion and rewriting. Bob and Eve fix typos; Dev introduces one that Bob corrects.' : 'Each tile captures a different live editor and replica. Watch five writers contribute and recover offline edits.'}</p><p>${participantCount > 5 ? `${participantCount - 5} additional live browser sessions edit their own review notes in this same document, below the visible paragraph.` : 'Five independent browser sessions, with isolated local storage.'}</p><ul id="checks"></ul><p class="note">Checkpoints compare all ${participantCount} actual editor states. This is a local demonstration, not a latency benchmark.</p></article>
  </div><div class="note">Live viewport captures; document text is never copied from one tile into another.</div></body></html>`);
  recording = board.video();
  let captureError;
  capture = (async () => {
    try {
      while (!stopCapture) {
        const frames = await Promise.all(pages.slice(0, 5).map(page => page.screenshot({ type: 'jpeg', quality: 80 })));
        await board.evaluate(frames => frames.forEach((frame, i) => { document.getElementById('view-' + i).src = 'data:image/jpeg;base64,' + frame; }), frames.map(frame => frame.toString('base64')));
        await delay(90);
      }
    } catch (error) { captureError = error; }
  })();
  const text = index => pages[index].evaluate(async () => {
    const { EditorView } = await import('/node_modules/.vite/deps/@codemirror_view.js');
    return EditorView.findFromDOM(document.querySelector('.cm-editor')).state.doc.toString();
  });
  async function phase(message, active = [], offline = [], labels = []) {
    if (captureError) throw captureError;
    await board.evaluate(({ message, active, offline, labels }) => {
      document.getElementById('phase').textContent = message;
      for (let i = 0; i < 5; i++) {
        document.getElementById('tile-' + i).className = [active.includes(i) ? 'active' : '', offline.includes(i) ? 'offline' : ''].join(' ');
        document.getElementById('badge-' + i).textContent = labels[i] ?? (offline.includes(i) ? 'Offline · writing locally' : active.includes(i) ? 'Typing now' : 'Live browser view');
      }
    }, { message, active, offline, labels });
  }
  async function verify(label, expected, reviewState) {
    await expect.poll(async () => {
      const values = await Promise.all(names.map((_, i) => text(i)));
      if (!values.every(value => value === values[0])) return false;
      if (values[0].split('\n\nReview log:')[0] !== expected) return false;
      if (participantCount > 5 && reviewState) {
        const actual = values[0].split('\n\nReview log:')[1].split('\n').map(line => line.trim()).filter(Boolean).sort();
        const wanted = names.slice(5).map(name => `${name}: ${reviewState}`).sort();
        if (JSON.stringify(actual) !== JSON.stringify(wanted)) return false;
      }
      return true;
    }, { timeout: 45_000 }).toBe(true);
    const fullText = await text(0);
    const hash = createHash('sha256').update(fullText).digest('hex');
    checkpoints.push({ label, replicas: participantCount, identical: true, sha256: hash, text: fullText });
    await board.evaluate(label => { const item = document.createElement('li'); item.textContent = label; document.getElementById('checks').append(item); }, label);
    await delay(900);
  }
  async function writeLine(index, content) {
    const lines = await editors[index].locator('.cm-line').allTextContents();
    const line = lines.findIndex(line => line.startsWith(names[index] + ':'));
    if (line < 0) throw new Error('Writer line is missing');
    await editors[index].press('Control+Home');
    for (let n = 0; n < line; n++) await editors[index].press('ArrowDown');
    await editors[index].press('End');
    await editors[index].pressSequentially(content, { delay: 85 });
  }
  async function select(index, needle) {
    await pages[index].evaluate(async needle => {
      const { EditorView } = await import('/node_modules/.vite/deps/@codemirror_view.js');
      const view = EditorView.findFromDOM(document.querySelector('.cm-editor'));
      const from = view.state.doc.toString().indexOf(needle);
      if (from < 0) throw new Error(`Missing selection: ${needle}`);
      view.dispatch({ selection: { anchor: from, head: from + needle.length }, scrollIntoView: true }); view.focus();
    }, needle);
  }
  if (organic) {
    let draft = 'We are planing a neighborhood garden. It opens on Saterday.\nBring seeds, gloves, and a snack. Beginners are welcome.\nMeet at the old gate.';
    await editors[0].fill(draft + (participantCount > 5 ? '\n\nReview log:' : ''));
    await verify(`${participantCount} sessions loaded the same draft`, draft);
    if (participantCount > 5) {
      await phase(`${participantCount - 5} background participants add real review notes to the same document.`);
      await Promise.all(editors.slice(5).map(async (editor, i) => { await editor.press('Control+End'); await pages[i + 5].keyboard.insertText(`\n${names[i + 5]}: needs review`); }));
      await verify('Every background session contributed a review note', draft, 'needs review');
    }
    const rounds = [
      { message: 'Five writers edit the same draft. Background reviewers rewrite their notes.', label: 'Concurrent writes, deletions and rewrites agree everywhere',
        actions: [['neighborhood', 'community', 'Rewriting the plan'], ['planing', 'planning', 'Fixing “planing”'], [', and a snack', '', 'Deleting extra words'], ['Beginners are welcome.', 'Please bring watre. Beginners are welcome.', 'Adding a sentence + typo'], ['old gate', 'north gate', 'Changing the meeting point']] },
      { message: 'Bob fixes Dev’s “watre”; Eve fixes “Saterday”. The others refine the paragraph.', label: 'Typos corrected; all shared text matches',
        actions: [['We are planning', 'Together, we are planning', 'Refining the opening'], ['watre', 'water', 'Fixing Dev’s typo'], ['seeds, gloves', 'seeds and gloves', 'Smoothing the wording'], ['Beginners are welcome.', 'New gardeners are welcome.', 'Rewriting the welcome'], ['Saterday', 'Saturday', 'Fixing the date']] },
    ];
    for (let r = 0; r < rounds.length; r++) {
      const round = rounds[r];
      await phase(round.message, [0, 1, 2, 3, 4], [], round.actions.map(action => action[2]));
      await Promise.all(round.actions.map((action, i) => select(i, action[0])));
      if (r === 0) await Promise.all(names.slice(5).map((name, i) => select(i + 5, `${name}: needs review`)));
      await delay(650);
      await Promise.all([
        ...round.actions.map((action, i) => action[1] ? editors[i].pressSequentially(action[1], { delay: 100 }) : editors[i].press('Backspace')),
        ...(r === 0 ? names.slice(5).map((name, i) => pages[i + 5].keyboard.insertText(`${name}: reviewed`)) : []),
      ]);
      for (const [before, after] of round.actions) draft = draft.replace(before, after);
      await verify(round.label, draft, 'reviewed');
    }
    await phase(`${participantCount} real replicas agree. Every writer contributed; every review is retained.`);
  } else {
  const lines = ['# Our launch checklist', '', ...names.map(name => name + ':')];
  await editors[0].fill(lines.join('\n'));
  await verify('All five joined the same document', lines.join('\n'));
  const tasks = [' Draft the outline.', ' Review the wording.', ' Check the examples.', ' Test offline edits.', ' Publish the notes.'];
  for (let i = 0; i < names.length; i++) {
    await phase(`${names[i]} writes. Watch the other four replicas update.`, [i]);
    await writeLine(i, tasks[i]); lines[i + 2] += tasks[i];
    await expect.poll(async () => Promise.all(names.map((_, n) => text(n))), { timeout: 15_000 }).toEqual(names.map(() => lines.join('\n')));
    await delay(500);
  }
  await verify('Every writer contributed; all five texts match', lines.join('\n'));
  await phase('All five type at once — on their own checklist lines.', [0, 1, 2, 3, 4]);
  await Promise.all(names.map((_, i) => writeLine(i, ' Ready!')));
  for (let i = 0; i < names.length; i++) lines[i + 2] += ' Ready!';
  await verify('Simultaneous typing retained all five contributions', lines.join('\n'));
  await pages[4].getByRole('button', { name: 'Connection details' }).click();
  await pages[4].getByRole('button', { name: 'Work offline' }).click();
  await pages[4].getByRole('button', { name: 'Connection details' }).click();
  await phase('Eve disconnects. Alice and Eve keep writing on separate replicas.', [0, 4], [4]);
  await Promise.all([writeLine(0, ' Online.'), writeLine(4, ' Offline.')]);
  lines[2] += ' Online.'; lines[6] += ' Offline.';
  expect(await text(4)).not.toEqual(await text(0));
  await delay(1_500);
  await phase('Eve reconnects. Both sides exchange their missing edits.', [], [4]);
  await pages[4].getByRole('button', { name: 'Connection details' }).click();
  await pages[4].getByRole('button', { name: 'Reconnect', exact: true }).click();
  await pages[4].getByRole('button', { name: 'Connection details' }).click();
  await verify('Offline recovery kept both sides; all five texts match', lines.join('\n'));
  await phase('Five independent replicas. Every contribution retained.');
  }
  await delay(2_500);
  const backupHashes = await Promise.all(pages.map(async page => {
    const pending = page.waitForEvent('download'); await page.getByRole('button', { name: 'Backup', exact: true }).click();
    const download = await pending, bytes = await readFile(await download.path());
    return createHash('sha256').update(bytes.subarray(bytes.indexOf(10, 12) + 1)).digest('hex');
  }));
  expect(new Set(backupHashes).size).toBe(1);
  stopCapture = true; await capture; if (captureError) throw captureError;
  await board.screenshot({ path: fileURLToPath(new URL(artifact + '.png', output)) });
  await writeFile(new URL(artifact + '.json', output), JSON.stringify({ commit: source, source_dirty: dirty, browser: browser.version(), sessions: participantCount, visible_sessions: 5,
    isolation: 'independent Chromium contexts; an additional context records five live viewport screenshots',
    transport: 'loopback WebRTC, full mesh up to eight; bounded overlay above eight', direct_degrees_before_edits: degrees,
    canonical_backup_sha256: backupHashes[0], identical_full_backups: true,
    checkpoint_method: 'all actual CodeMirror states agree with the expected text and every review note; complete CRDT backup bytes also agree', checkpoints }, null, 2));
  await recorder.close();
  await rename(await recording.path(), new URL(artifact + '.webm', output));
  console.log(`Recorded demo/${artifact}.webm; all ${participantCount} replicas passed every checkpoint`);
} finally {
  stopCapture = true;
  if (capture) await capture;
  if (browser) await browser.close();
  for (const child of children) await stop(child).catch(() => {});
}
