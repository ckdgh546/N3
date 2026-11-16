/* 필기 노트는 https://ckdgh546.github.io/note/ 로 옮겼다. 예전 주소의 보관 파일을 지우고 물러난다. */
self.addEventListener('install', () => self.skipWaiting());
self.addEventListener('activate', e => e.waitUntil(caches.keys().then(ks => Promise.all(ks.filter(k => k.indexOf('note-app') === 0).map(k => caches.delete(k)))).then(() => self.registration.unregister())));
