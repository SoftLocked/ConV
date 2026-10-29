import { readdir, writeFile } from 'node:fs/promises';
const assets = await readdir(new URL('../web/dist/assets/', import.meta.url));
const urls = ['/', '/index.html', ...assets.map(name => `/assets/${name}`)];
const version = `conv-${Date.now()}`;
await writeFile(new URL('../web/dist/sw.js', import.meta.url), `
const VERSION = ${JSON.stringify(version)};
const ASSETS = ${JSON.stringify(urls)};
self.addEventListener('install', event => { event.waitUntil(caches.open(VERSION).then(cache => cache.addAll(ASSETS)).then(() => self.skipWaiting())); });
self.addEventListener('activate', event => { event.waitUntil(caches.keys().then(keys => Promise.all(keys.filter(key => key.startsWith('conv-') && key !== VERSION).map(key => caches.delete(key)))).then(() => self.clients.claim())); });
self.addEventListener('fetch', event => {
  if (event.request.method !== 'GET' || new URL(event.request.url).origin !== self.location.origin) return;
  event.respondWith(caches.match(event.request).then(saved => saved || fetch(event.request).catch(error => { if (event.request.mode === 'navigate') return caches.match('/index.html'); throw error; })));
});
`);
