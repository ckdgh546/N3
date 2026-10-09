/* N3 Loop 7.4 cache cleanup worker.
   If an older service worker exists, this update activates once, removes old N3 caches,
   unregisters itself, and leaves the site as a normal GitHub Pages web app. */
self.addEventListener('install', event => { self.skipWaiting(); });
self.addEventListener('activate', event => {
  event.waitUntil((async()=>{
    const keys = await caches.keys();
    await Promise.all(keys.filter(k=>k.includes('n3-loop')).map(k=>caches.delete(k)));
    await self.registration.unregister();
    await self.clients.claim();
  })());
});
