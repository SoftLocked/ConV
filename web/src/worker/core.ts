/// <reference lib="webworker" />
import { Engine, ready, now, type Update } from '../engine';
import { load, save } from '../storage/local';
const worker = self as unknown as DedicatedWorkerGlobalScope;
let engine: Engine, room: string, writer: string, name = 'Untitled document';
let history = 0, ticks = 0;
function dispatch(update: Update) {
  if (update.outbound.length) worker.postMessage({ type: 'outbound', frames: update.outbound });
  return update;
}
async function persist() {
  await save({ key: `${room}:${writer}`, room, backup: engine.backup(), name });
}
// A serial task queue makes each save acknowledgement correspond to a completed
// IndexedDB transaction, including all previously accepted local mutations.
let queue = Promise.resolve();
worker.onmessage = ({ data }) => {
  queue = queue.then(async () => {
    if (data.type === 'init') {
      await ready;
      room = data.room; writer = data.writer;
      const saved = await load(room);
      name = saved.at(-1)?.name ?? name;
      engine = new Engine(room, writer, saved[0]?.backup);
      for (const entry of saved.slice(1)) engine.merge_backup(entry.backup);
      worker.postMessage({ type: 'ready', backup: engine.backup(), name });
      setInterval(() => worker.postMessage({ type: 'tick-needed' }), 100);
    } else if (data.type === 'local') {
      for (const frame of data.frames) engine.receive(new Uint8Array(frame.data), now());
      history = engine.history_len();
      await persist();
      worker.postMessage({ type: 'saved', serial: data.serial });
      worker.postMessage({ type: 'broadcast', frames: data.frames });
    } else if (data.type === 'receive') {
      const update = dispatch(engine.receive(new Uint8Array(data.frame), now()) as Update);
      worker.postMessage({ type: 'remote', frame: data.frame });
      if (update.history !== history) { history = update.history; await persist(); }
    } else if (data.type === 'peers') {
      engine.connect(data.ids);
      for (const id of data.ids) dispatch(engine.hello(id) as Update);
    } else if (data.type === 'tick') {
      dispatch(engine.tick(now(), ++ticks % 10 === 0) as Update);
    } else if (data.type === 'snapshot') { worker.postMessage({ type: 'snapshot', backup: engine.backup() });
    } else if (data.type === 'name') { name = String(data.name).slice(0, 120); await persist(); }
  }).catch(error => worker.postMessage({ type: 'error', message: String(error) }));
};
