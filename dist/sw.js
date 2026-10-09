/* N3 Loop 8.0: remove legacy PWA caches. */
self.addEventListener('install',()=>self.skipWaiting());
self.addEventListener('activate',event=>event.waitUntil((async()=>{for(const k of await caches.keys())if(k.includes('n3-loop'))await caches.delete(k);await self.registration.unregister();await self.clients.claim();})()));
