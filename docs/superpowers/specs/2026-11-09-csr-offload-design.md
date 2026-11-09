# CSR offload + onion admin server — design

Date: 2026-11-09. Status: draft, part A builder-checked; part B added from
the owner's direction afterwards.

## Intent

Two moves with one theme — the browser does more, the box does less:

- **Part A — CSR offload.** Move per-view rendering and per-edit
  compilation out of `lososd` and into the owner's browser
  (client-side rendering). The box is a repurposed mini-PC; every byte of
  CPU and RAM it spends drawing pages for a browser that could draw them
  itself is a byte taken from file shares, mesh compute and the
  assistant.
- **Part B — the onion admin server.** The box's `.onion` address reaches
  LosOS Cloud, LosOS Git and LosOS Login, but never the admin pages: the
  front vhost's admin guard denies loopback (`modules/lan-only.nix`),
  which is where Tor visitors arrive from — the same rule that keeps the
  edge's tunnel the only loopback caller, and it must not be loosened.
  The owner wants the admin pages reachable over Tor through a smaller
  HTTP server, with **Traefik in front of it**.

## What was examined and why it is in or out

- **MDX pages** — in (part A). `GET /api/pages`
  (`backend/plugins/pages/src/model.rs`, `cmd_list`) renders every
  customised page to HTML on the box, once per sign-in per person, and
  the SPA caches the HTML. The same `losos-mdx` crate already ships to
  the browser as WebAssembly and the pages editor already renders pages
  with it (`admin-ui/app/tests/pages.browser.ts`). The renderer is paid
  for either way; only the placement is wrong.
- **User apps** — in (part A). The box parses, checks and compiles each
  app's JSX with oxc (`backend/src/jsx.rs`) when the editor checks or
  saves one. Interactive, browser-present work; moves to the browser.
  The box keeps oxc for the paths with no browser around (see Decided).
- **Log reader** — dropped, nothing to move. The no-JS page stays: it is
  the only place a browser without JavaScript can read the journal, and
  it is what answers while `lososd` is down. With it staying, the
  reader's JSON API is already the SPA's surface and there is nothing to
  offload.
- **Notes** — out, nothing to move. Notes are already client-rendered:
  "The web app renders the note and decides which note a link means"
  (`backend/src/cloud/notes/scan.rs`). The box only scans front matter,
  tags and links so the graph and backlinks draw without every note's
  text crossing the wire; moving that scan to the browser would put
  every note's full text on the wire to save a trivial amount of box
  CPU. Knowledge bases likewise render in the SPA.

# Part A — CSR offload (pages, user apps)

## Goals

1. **Pages render in the browser.** `GET /api/pages` answers with data,
   not HTML: for each page the custom source (or null), plus the blocks
   registry the build wrote. The SPA renders each page with the existing
   `losos-mdx` WASM at the moment it draws the page, and falls back to
   the default source when a custom source no longer renders (today the
   box does that fallback in `cmd_list`). A page `blocks.json` no longer
   lists stays left out, as today.
2. **Page saves validate in the browser, then commit on the box.** The
   editor checks a save with the WASM renderer and shows problems before
   the `PUT`. The box keeps its own `render_page` check inside
   `cmd_put` (see Decided for why).
3. **User apps compile in the browser.** The editor's check and save no
   longer ask the box to compile: the browser parses, checks and
   transforms JSX, and the sandboxed app frame runs the compiled script
   exactly as it does today (same frame contract, same CSP arms, same
   bindings block). `POST /api/user-apps/check` stays for clients
   without a browser.

## Non-goals and anti-goals (part A)

- **Notes and knowledge bases do not change** (see above).
- **No auth changes anywhere.** Who may read pages and apps stays
  exactly as it is.
- **The `/basic/` no-JS admin suite stays**, including the log reader's
  own no-JS page. Only the log reader's drawing of it belongs to this
  conversation, and it stays.
- **The builder's loop and app pushes keep server-side oxc.** The
  builder drives its own fix loop on the box and scores candidates by
  whether they compile, on purpose (`backend/src/builder.rs`); a push to
  an app's repository is checked and compiled with no browser in sight
  (`backend/src/app_repo.rs`). Both keep using `crate::jsx` unchanged.
- **No performance work in the SPA for its own sake.** The MDX WASM is a
  lazy chunk the editor already ships; pages rendering loads it for
  viewers too, which is accepted, not optimised around.

## Decided (part A)

Calls made in this conversation; correct any while reading.

1. **`cmd_put` keeps its server-side `render_page` validation.** A save
   is rare, and it keeps every writer honest — the TUI, a future client,
   a compromised tab — so the pages repository stays clean for all of
   them. The hot path (`cmd_list`) is what loses its renderer.
2. **`GET /api/pages` answers with sources and registry in one body.**
   Custom source, saved-by/at, and the registry JSON, the shape the SPA
   caches per sign-in today. The SPA bundle keeps carrying every page's
   default source, as it does now, so unchanged pages keep their instant
   first paint and the browser always holds the default it needs for the
   broken-custom-source fallback; only customised pages ride the wire.
3. **The browser-side app check is the same Rust, built for wasm32.**
   The check extracts into its own crate (like `losos-mdx`) and builds
   for `wasm32-unknown-unknown`, so there is one implementation of the
   policy — the same trick the MDX renderer already pulls. If oxc does
   not build for wasm32, the fallback is a TypeScript transform and
   checker in the browser with drift guarded by fixture tests shared
   with the Rust crate's tests. Spike this first; it is the plan's
   first risk.
4. **The browser PUTs source and compiled script together, and the box
   stores them without compiling.** The stored shape (source plus script
   in `user-apps.json`) and what the frame loads do not change. A
   browser-supplied script gains nothing: it only ever runs in that
   browser's own sandboxed frame. The save's history entry records the
   problems the client reported with the save; a save from a client
   without a browser (the TUI) carries no script, and the box compiles
   that one itself with oxc — the same code the push path runs. So the
   PUT accepts both shapes: source plus script, or source alone.

## Constraints (part A)

- The app frame's contract is unchanged: a compiled script mounted
  inside the promised bindings, in an `<iframe sandbox>` under the
  `/app-frame/` CSP arm. Only who compiles changes.
- House rules: clippy `-D warnings` and rustfmt across all crates, Bun
  workspaces with no Node APIs, Nix-installed `node_modules`, Playwright
  handed files not Buffers. `nix/tests/plugins.nix` (pages screenshots)
  and the admin UI's browser tests cover the touched surfaces and must
  keep passing in their new shape.

# Part B — the onion admin server

## What is wrong today

The hidden service maps port 80 to `127.0.0.1:8480` and port 443 to
`127.0.0.1:8443`, nginx's stream TLS, which passes to `8481`
(`modules/hidden-services.nix`). Both listeners sit on the front vhost,
whose admin routes carry the lan-only guard: a Tor visitor arrives from
`127.0.0.1`, which the guard turns away, exactly as it turns away the
edge's tunnel. So the admin pages answer 403 over Tor by design. The
guard is load-bearing (rule 9) and stays.

## Goals

5. **The admin pages work over Tor and I2P.** The `.onion` /
   `.b32.i2p` address serves the admin SPA and its API end to end:
   sign in with LosOS Login (or a spare key), read and use every pane a
   LAN visitor can.
6. **Traefik fronts it.** `losos-traefik` holds the onion listeners as
   its own entrypoints and routes to lososd; nginx leaves the onion
   path entirely and keeps the main vhost exactly as it is.
7. **The serving side is a lososd plugin.** A new plugin serves the
   onion surface's own routes — the SPA's static files. The API routes
   on the second listener are lososd's existing handlers dispatched
   directly (full parity needs them all); the listener's route list,
   not the plugin's manifest, is what narrows the surface, and the
   manifest documents what rides it. The plugin stays weak: it never
   names a caller, so it can never reach a route as someone else.

## The shape

- **Entry points.** Traefik gains two loopback entrypoints:
  `onion-http` on 8480 and `onion-https` on 8443 ending TLS with the
  hidden leaf certificate the `losos-hidden-tls` unit already mints
  (`modules/hidden-services.nix`; the certificate's group becomes
  Traefik's so it can read it). The hidden services keep mapping
  80→8480 and 443→8443; I2P rides the same listeners. nginx loses the
  8480/8481 listeners and its stream block, nothing else.
- **Auth.** Routers on the onion entrypoints carry a `forwardAuth`
  middleware against LosOS Login's authz endpoint — the Traefik
  spelling of the `auth_request` dance nginx does today — which sets
  `Remote-User` and `Remote-Groups` for the backend. A second,
  higher-priority router matching a Bearer `Authorization` header skips
  the middleware, so the spare key opens the box over Tor as it does on
  the LAN. The cookie flow works as it does today: Authelia already
  takes the hidden addresses as cookie domains, and the 8443 half is
  https, which its Secure cookies need.
- **Backend.** lososd binds a second loopback listener, apart from the
  8082 admin API, so the onion surface stays a thing you can point at
  and reason about. On it, the new plugin serves the SPA's static files
  (the client renders; the box hands out bytes) and the API routes,
   which are lososd's own handlers with the caller the forwardAuth
  headers name — trusted the same way `Remote-User` from nginx is
  trusted today (loopback plus identity on).
- **Surface.** Full parity with the LAN for signed-in visitors: every
  pane a LAN visitor can reach, the onion visitor reaches. The one
  exception is deliberate: the unauthenticated family — the first-run
  claim and everything under `/api/setup` — does not ride the onion.
  The claim is a first-run, at-the-box act, and putting it on the
  onion would let anyone in the Tor network claim an unclaimed box.
  Health stays unauthenticated. What narrows later narrows in the
  second listener's route list.
- **Apps stay LAN-only.** Traefik's app routers keep their
  `lan-only@file` ipAllowList and do not attach to the onion
  entrypoints. (My call — the ask was the admin pages; say the word and
  app ports attach to the onion too.)
- **One Traefik, conditional config.** `losos-traefik` runs whenever
  apps or hidden services are on; its static config gains the onion
  entrypoints and forwardAuth only when hidden services are on, and
  carries the Podman provider and apps plumbing only when apps are. A
  box with Tor and no apps runs the same daemon with the apps parts
  absent. The 8480/8443 ports join the reserved list, and the
  daemon's firewall owner rule — which keeps the loopback API from
  hostNetwork pods — must cover the second listener and Traefik's uid,
  or any pod could name itself anyone on it.

## Decided (part B)

8. **Traefik in front.** The owner picked this over the onion server
   proxying to Traefik: the apps router stays the one thing holding
   listeners beyond nginx, and the onion joins it as entrypoints.
9. **The onion splits by path, not by vhost.** Traefik holds the onion
   listeners: admin paths and the API route to lososd's second
   listener, `/auth/` routes to Authelia, and everything the onion
   serves today that is not admin — `/cloud/`, `/git/`, `/wiki/`,
   `/kb/`, the handbook — proxies to a nginx loopback listener nginx
   keeps for exactly those paths. LosOS Cloud and LosOS Git keep their
   Unix sockets and their "only nginx opens them" trust story untouched
   (rule 10); no socket opener is added, no TCP port given to Cloud.
   The admin guard never applies to these paths, and admin paths never
   reach nginx.
10. **A separate lososd listener for the onion surface**, not a reuse
    of 8082, so guard reasoning stays clean.
11. **Parity of surface, minus the claim.** The signed-in surface
    matches the LAN's; the unauthenticated claim/setup family is absent
    and health stays. What narrows later narrows in the second
    listener's route list, which the manifest documents.
12. **The plugin stays weak.** It serves the static files and never
    names a caller; API dispatch is host-side against the existing
    handlers. A manifest-narrowed proxy plugin would have to reach
    routes as any principal — a full-admin-equivalent component, which
    is the opposite of the wall this feature is meant to be.
13. **One Traefik daemon for both roles**, its static config split so
    hidden services without apps do not drag the apps plumbing along.
14. **The no-JS suite stays LAN-only.** The onion surface is the SPA
    (the client renders) plus `/auth/` for sign-in; `/basic/` keeps
    needing nginx's cookie-to-Bearer translation, which only the LAN
    vhost does. A no-JS browser over Tor draws the box's plain 403, as
    an unauthenticated visitor does.

## Left to the builder (both parts)

- Exact JSON shapes and TypeScript types for the new `/api/pages`
  answer; how the pages store keeps drafts as sources instead of HTML.
- Where the extracted app-check crate lives, its WASM binding shape,
  and how the admin UI loads it (`src/mdx/pkg/` is the pattern).
- The onion plugin's crate layout, its manifest (files, endpoints), the
  second listener's wiring in `backend/src/http.rs`, and how static
  files cross the plugin boundary (host fs imports per request, or the
  host serving the dist directory on that listener with the plugin
  owning only routes — measure, then pick).
- Traefik's static and dynamic config for the two entrypoints and the
  forwardAuth routers; where the Bearer bypass router's rule lives.
- Test updates: `pages.browser.ts` becomes the norm rather than the
  double-check; the app editor's browser tests move from
  `/api/user-apps/check` to the in-browser check; a VM test that boots a
  box with Tor (or a stubbed listener) and reads the admin SPA over the
  onion entrypoints through Traefik, signed in and with the spare key.
- Handbook and developer-doc updates wherever the hidden services' 403
  and the box-compiled apps are described (`.claude/CLAUDE.md` among
  them); `nix/tests/hidden-services.nix` asserts the new shape.
