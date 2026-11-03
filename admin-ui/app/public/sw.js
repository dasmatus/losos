// LosOS admin UI service worker: caches the app shell and static assets so the
// UI keeps working when the box is unreachable.
const CACHE = "losos-ui-v1";
const SHELL = ["/", "/index.html", "/theme-boot.js"];

self.addEventListener("install", (event) => {
  event.waitUntil(
    caches.open(CACHE).then((cache) => cache.addAll(SHELL)).then(() => self.skipWaiting()),
  );
});

self.addEventListener("activate", (event) => {
  event.waitUntil(
    caches.keys().then((keys) =>
      Promise.all(keys.filter((k) => k !== CACHE).map((k) => caches.delete(k))),
    ).then(() => self.clients.claim()),
  );
});

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;
  // Network-first for API calls.
  if (url.pathname.startsWith("/api") || url.pathname.startsWith("/cloud/api") || url.pathname.startsWith("/kb/api")) {
    event.respondWith(
      fetch(req).catch(() => caches.match(req)),
    );
    return;
  }
  // Cache-first for everything else, falling back to network.
  event.respondWith(
    caches.match(req).then((cached) =>
      cached ||
      fetch(req).then((res) => {
        const copy = res.clone();
        caches.open(CACHE).then((cache) => cache.put(req, copy));
        return res;
      }),
    ),
  );
});
