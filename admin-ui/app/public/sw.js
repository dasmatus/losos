// LosOS admin UI service worker: caches the app shell and the fingerprinted
// static assets so the UI keeps working when the box is unreachable.
//
// It stays out of everything else on purpose. Answers from lososd, LosOS
// Login and the apps on the box must come from the box, not from a copy, and
// pages the apps serve in subframes are theirs to hand out. When the page is
// fetched as data or inside a frame, the browser does the whole request, so
// anything the browser or its extensions already intercept keeps working:
// this worker only takes over what it built itself.
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

/* Hashed by the build, so a new deployment changes the URL and no served copy
 * can go stale. theme-boot.js is deliberately unhashed: it colors the page
 * before the bundles arrive, and is not depended on for anything after that,
 * so a stale copy is harmless. */
const isBundle = (path) => path.startsWith("/assets/") || path === "/theme-boot.js";

self.addEventListener("fetch", (event) => {
  const req = event.request;
  if (req.method !== "GET") return;
  const url = new URL(req.url);
  if (url.origin !== self.location.origin) return;
  /* Only the top-level document and the bundles are this worker's business.
   * Everything the page fetches as data (`/api/*`, `/setup/state.json`, the
   * apps' endpoints) and every subframe stays with the browser. */
  const isTopFrame = req.destination === "document";
  if (!isTopFrame && !isBundle(url.pathname)) return;

  if (isTopFrame) {
    // Network-first for the document, falling back to the saved shell: the
    // page always boots from what the box currently serves while it is
    // reachable, and from the last good shell when it is not.
    event.respondWith(
      fetch(req)
        .then((res) => {
          if (res.ok) {
            const copy = res.clone();
            // A redirected answer never enters a cache, so refuse-inside-put
            // stays an everyday thing rather than an error.
            caches.open(CACHE).then((cache) => cache.put(req, copy)).catch(() => {});
          }
          return res;
        })
        .catch(() =>
          caches.match(req).then((cached) => cached || caches.match("/index.html")),
        ),
    );
    return;
  }

  // Cache-first for the bundles: the URL carries the version, so serving a
  // past copy is never wrong.
  event.respondWith(
    caches.match(req).then((cached) =>
      cached ||
      fetch(req).then((res) => {
        if (res.status === 200 && res.type === "basic") {
          const copy = res.clone();
          caches.open(CACHE).then((cache) => cache.put(req, copy)).catch(() => {});
        }
        return res;
      }),
    ),
  );
});
