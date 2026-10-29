export type Saved = { key: string; room: string; backup: Uint8Array; name: string; nameChangedAt?: number };
let opening: Promise<IDBDatabase> | undefined;
function open() {
  return opening ??= new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open('conv-v1', 1);
    request.onupgradeneeded = () => {
      const store = request.result.createObjectStore('sessions', { keyPath: 'key' });
      store.createIndex('room', 'room');
    };
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
export async function load(room: string): Promise<Saved[]> {
  const db = await open();
  return new Promise((resolve, reject) => {
    const request = db.transaction('sessions').objectStore('sessions').index('room').getAll(room);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error);
  });
}
export async function save(value: Saved) {
  const db = await open();
  return new Promise<void>((resolve, reject) => {
    const transaction = db.transaction('sessions', 'readwrite', { durability: 'strict' });
    transaction.objectStore('sessions').put(value);
    transaction.oncomplete = () => resolve();
    transaction.onerror = () => reject(transaction.error);
    transaction.onabort = () => reject(transaction.error ?? new Error('Local save aborted'));
  });
}
