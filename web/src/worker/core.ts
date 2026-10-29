/// <reference lib="webworker" />
import { Engine, ready, now, type Update } from '../engine';
import { load, save } from '../storage/local';
const worker = self as unknown as DedicatedWorkerGlobalScope;
let engine: Engine, room: string, writer: string, name = 'Untitled document', nameChangedAt = 0;
let history = 0, ticks = 0;
let knownPeers: string[] = [];
let knownMembers: string[] = [];
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
async function persist() {
  await save({ key: `${room}:${writer}`, room, backup: engine.backup(), name, nameChangedAt });
}
// A serial task queue makes each save acknowledgement correspond to a completed
// IndexedDB transaction, including all previously accepted local mutations.
let queue = Promise.resolve();
worker.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    if (data.type === 'init') {
      await ready;
      room = data.room; writer = data.writer;
      const saved = (await load(room)).sort((a, b) => (a.nameChangedAt ?? 0) - (b.nameChangedAt ?? 0) || a.key.localeCompare(b.key));
      name = saved.at(-1)?.name ?? name;
      nameChangedAt = saved.at(-1)?.nameChangedAt ?? 0;
      engine = new Engine(room, writer, saved[0]?.backup);
      for (const entry of saved.slice(1)) engine.merge_backup(entry.backup);
      worker.postMessage({ type: 'ready', backup: engine.backup(), name, nameChangedAt });
      setInterval(() => worker.postMessage({ type: 'tick-needed' }), 20);
    } else if (data.type === 'local') {
      const outgoing = [];
      for (const frame of data.frames) outgoing.push(...(engine.receive(new Uint8Array(frame.data), now()) as Update).outbound);
      history = engine.history_len();
      await persist();
      worker.postMessage({ type: 'saved', serial: data.serial });
      if (outgoing.length) worker.postMessage({ type: 'outbound', frames: outgoing });
    } else if (data.type === 'receive') {
      const update = dispatch(engine.receive(new Uint8Array(data.frame), now()) as Update);
      if (update.history !== history) {
        // Forward complete document changes only. Control/duplicate frames need
        // no editor merge; a completed chunked transfer becomes an atomic backup.
        const frame = new Uint8Array(data.frame);
        if (new DataView(frame.buffer, frame.byteOffset, frame.byteLength).getUint32(44, true) === 1) worker.postMessage({ type: 'remote', frame: data.frame });
        else worker.postMessage({ type: 'snapshot', backup: engine.backup() });
        history = update.history; await persist();
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
      if (engine.needs_probe_order()) engine.reshuffle_probes(shuffled(knownMembers));
      dispatch(engine.tick(now(), ++ticks % 50 === 0) as Update);
    } else if (data.type === 'snapshot') { worker.postMessage({ type: 'snapshot', backup: engine.backup() });
    } else if (data.type === 'name') { name = String(data.name).slice(0, 120); nameChangedAt = Date.now(); await persist(); worker.postMessage({ type: 'saved', serial: data.serial }); }
  }).catch(error => worker.postMessage({ type: 'error', message: String(error) }));
};
