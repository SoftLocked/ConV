/// <reference lib="webworker" />
import { Engine, ready, now, type Update } from '../engine';
import { load, save } from '../storage/local';
const worker = self as unknown as DedicatedWorkerGlobalScope;
let engine: Engine, room: string, writer: string, name = 'Untitled document', nameChangedAt = 0;
let history = 0, localSerial = 0, nextSync = 0n, tickPending = false;
let knownPeers: string[] = [], knownMembers: string[] = [];
let dirty = false, saving = false, saveTimer: ReturnType<typeof setTimeout> | undefined;
function shuffled(ids: string[]) {
  const output = ids.slice();
  for (let index = output.length - 1; index > 0; index--) {
    const other = crypto.getRandomValues(new Uint32Array(1))[0] % (index + 1);
    [output[index], output[other]] = [output[other], output[index]];
  }
  return output;
}
function dispatch(update: Update) {
  if (update.outbound.length) worker.postMessage({ type: 'outbound', frames: update.outbound });
  return update;
}
function scheduleSave() {
  if (!dirty) worker.postMessage({ type: 'saving', serial: localSerial });
  dirty = true;
  if (saveTimer || saving) return;
  saveTimer = setTimeout(async () => {
    saveTimer = undefined; saving = true; dirty = false;
    const serial = localSerial;
    try {
      // Capture a complete immutable snapshot before yielding. New edits can be
      // processed while this transaction commits, and get a subsequent save.
      const value = { key: `${room}:${writer}`, room, backup: engine.backup(), name, nameChangedAt };
      await save(value);
      if (!dirty && serial === localSerial) worker.postMessage({ type: 'saved', serial });
    } catch (error) {
      dirty = true;
      worker.postMessage({ type: 'error', message: String(error) });
      // A later edit retries. Do not falsely acknowledge or spin on quota errors.
      return;
    } finally { saving = false; }
    if (dirty) scheduleSave();
  }, 50);
}
// Serial core operations, with at most one queued timer and one durable write.
// Incoming changes do not wait for every intermediate full-history snapshot.
let queue = Promise.resolve();
function enqueue(data: any) {
  queue = queue.then(async () => {
    if (data.type === 'init') {
      await ready;
      room = data.room; writer = data.writer;
      const saved = (await load(room)).sort((a, b) => (a.nameChangedAt ?? 0) - (b.nameChangedAt ?? 0) || a.key.localeCompare(b.key));
      name = saved.at(-1)?.name ?? name;
      nameChangedAt = saved.at(-1)?.nameChangedAt ?? 0;
      engine = new Engine(room, writer, saved[0]?.backup);
      for (const entry of saved.slice(1)) engine.merge_backup(entry.backup);
      history = engine.history_len();
      worker.postMessage({ type: 'ready', backup: engine.backup(), name, nameChangedAt });
      setInterval(() => { if (!tickPending) { tickPending = true; enqueue({ type: 'tick' }); } }, 20);
    } else if (data.type === 'local') {
      for (const frame of data.frames) dispatch(engine.receive(new Uint8Array(frame.data), now()) as Update);
      history = engine.history_len(); localSerial = data.serial; scheduleSave();
    } else if (data.type === 'receive') {
      const update = dispatch(engine.receive(new Uint8Array(data.frame), now()) as Update);
      if (update.history !== history) {
        const frame = new Uint8Array(data.frame);
        if (new DataView(frame.buffer, frame.byteOffset, frame.byteLength).getUint32(44, true) === 1) worker.postMessage({ type: 'remote', frame: data.frame });
        else worker.postMessage({ type: 'snapshot', backup: engine.backup() });
        history = update.history; scheduleSave();
      }
    } else if (data.type === 'peers') {
      const ids: string[] = [...data.ids].sort(), members: string[] = [...data.members].sort();
      const changed = ids.join() !== knownPeers.join(), membershipChanged = members.join() !== knownMembers.join();
      engine.overlay(data.overlay);
      if (changed) {
        const added = ids.filter(id => !knownPeers.includes(id)); knownPeers = ids;
        engine.connect(shuffled(knownPeers));
        for (const id of added) dispatch(engine.hello(id) as Update);
      }
      if (changed || membershipChanged) { knownMembers = members; engine.reshuffle_probes(shuffled(knownMembers)); }
    } else if (data.type === 'tick') {
      tickPending = false;
      if (engine.needs_probe_order()) engine.reshuffle_probes(shuffled(knownMembers));
      const time = now(), sync = time >= nextSync;
      if (sync) nextSync = time + 1_000n;
      dispatch(engine.tick(time, sync) as Update);
    } else if (data.type === 'snapshot') { worker.postMessage({ type: 'snapshot', backup: engine.backup() });
    } else if (data.type === 'name') {
      name = String(data.name).slice(0, 120); nameChangedAt = Date.now(); localSerial = data.serial; scheduleSave();
    }
  }).catch(error => worker.postMessage({ type: 'error', message: String(error) }));
}
worker.onmessage = ({ data }) => enqueue(data);
