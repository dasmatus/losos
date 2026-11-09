# The admin app's Content-Security-Policy, as one string. Two servers hand
# the app's document out: the front vhost (modules/containers.nix, in its
# header map) and Traefik's onion document router (modules/apps.nix, whose
# `headers` middleware sets it on the second listener's answers) — one
# policy, imported twice, so they cannot drift.
#
# `add_header` at server level is inherited by every location that does not
# set one of its own, so containers.nix selects values with a `map` on $uri:
# the routes with a policy of their own get theirs; nginx omits a header
# whose value is empty. Fail-closed on purpose: the default arm is this
# strict policy, so a new admin route is covered without anyone remembering
# to add it.
#
# (Chosen over per-location `add_header` because that form *replaces* the
# inherited set, so any header added at server level later would silently
# vanish from exactly the locations that need it most.)
#
# img-src carries `data:` for a small picture a library or a stylesheet
# writes inline, and a CSS background-image is an img-src fetch. Bun's
# bundler (admin-ui/app/build.ts) emits every imported file as a file under
# assets/, so the grant is currently unused — it is kept because the first
# inlined SVG would otherwise fail in a console nobody is watching, on a box
# with no shell.
#
# connect-src is plain 'self': it used to also carry http(s)://$host:3456
# for the dashboard's cross-origin probe of the Tahoe web UI, and Tahoe-LAFS
# is gone — with it the :3456 vhost, the probe, and any reason for this page
# to talk to a second origin.
#
# script-src carries 'wasm-unsafe-eval' for LosOS Diagrams, a screen of
# this page at /diagrams whose core is Rust compiled to WebAssembly
# (admin-ui/lab/core). It lets the page compile WebAssembly and nothing
# more: eval() and new Function() stay refused, and compiling a module
# takes script that is already running, which this policy still allows
# only from 'self'. It was a policy of its own while Diagrams was a page
# of its own; as a screen it is reached by the router without a page load,
# so the admin document needs it. img-src's blob: is the pictures and
# setup files Diagrams hands out.
#
# The board embeds `/widget-frame/` for the owner's hand-written widgets. It
# has to be spelled out: under CSP3 frame-src falls back to child-src and
# then to default-src, which is 'none' above, so an absent frame-src is a
# *block*, not an inherit. 'self' and nothing wider.
#
# frame-ancestors 'none' governs who may frame the admin page. Nothing about
# embedding a frame needs this loosened, and nix/tests/front-vhost.nix
# asserts it stayed DENY precisely because reaching for it is the obvious
# wrong way to unblock an iframe.
{ lib }:

lib.concatStringsSep "; " [
  "default-src 'none'"
  "script-src 'self' 'wasm-unsafe-eval'"
  "style-src 'self'"
  "img-src 'self' data: blob:"
  "font-src 'self'"
  "connect-src 'self'"
  "frame-src 'self'"
  "form-action 'none'"
  "frame-ancestors 'none'"
  "base-uri 'none'"
]
