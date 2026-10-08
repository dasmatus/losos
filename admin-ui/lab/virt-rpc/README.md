# losos-lab-virt

A libvirt client for LosOS Lab that runs in the browser. It speaks
libvirt's own remote protocol (XDR RPC, program `0x20008086`, version 1)
from WebAssembly, so the host needs no `virsh` and no libvirt client
library. The host still has to carry the bytes, because a web page cannot
open libvirt's socket by itself.

Procedure numbers and struct layouts come from libvirt 12.7.0 (the version
in the flake's pinned nixpkgs): `src/remote/remote_protocol.x`,
`src/rpc/virnetprotocol.x`, `src/rpc/virkeepaliveprotocol.x`.

## Why a page cannot reach libvirt directly

- libvirtd, virtqemud and virtproxyd listen on a unix socket
  (`/run/libvirt/virtqemud-sock`, or `libvirt-sock` for virtproxyd), and
  optionally on TCP 16509 or TLS 16514. A page can use `fetch`,
  WebSocket, WebTransport and WebRTC, and none of them can open a raw TCP
  connection or a unix socket.
- libvirt has no WebSocket listener. In the 12.7.0 tree the only WebSocket
  code is QEMU's VNC websocket (`src/qemu/qemu_vnc.c`, `qemu.conf`), and
  that carries graphics only. `libvirt-console-proxy` puts VNC, SPICE and
  serial consoles behind WebSockets, but not the management API.
- Chrome's Direct Sockets API (`TCPSocket`) works only in Isolated Web
  Apps. The Chrome intent to ship (M130) says it is "available only in
  Isolated Web Apps on desktop platforms", and that for end users this
  means ChromeOS only. It needs cross-origin isolation and the
  `direct-sockets` permissions policy in the app's manifest. Loopback and
  LAN addresses also need `direct-sockets-private`, which Chrome 151 splits
  into `local-network` and `loopback-network`. `new TCPSocket(address,
  port)` takes a host and a port, so it cannot open a unix socket. A Lab
  page on Vercel or on the box's `/lab/` is a normal web page, so it gets
  no `TCPSocket`.

## What the host needs

A **byte relay** from a WebSocket to the daemon's socket. The relay knows
nothing about libvirt. Every binary message is written to the socket as
is, and every read from the socket goes back to the page as one binary
message. The Lab's own relay is `GET /lab/v1/virt` in `losos-registrar
lab` (`backend-registrar/src/lab/virt.rs`; on a box, `/api/lab/virt`): it
finds the socket for its `--connect` URI, admits a WebSocket only with a
single-use ticket from `POST /lab/v1/virt-ticket` sent as the
`ticket.<hex>` subprotocol beside `losos-lab`, and applies the helper's
Origin and Host checks. `relay/ws-relay.mjs` is a standalone test relay:
about 100 lines of Node with no dependencies. `websockify` would also work.
None of them parses the protocol; all the libvirt logic is in the page.

In an Isolated Web App, the alternative is a daemon listening on TCP
(virtproxyd or libvirtd with `listen_tcp = 1`). The client then connects
with `tcp://host:16509` and needs no relay. The Security section below
covers what this costs.

## What works

These were tested against virtqemud 12.7.0 and QEMU 11.1.0 (TCG):

| Procedure | Number |
|---|---|
| `AUTH_LIST` (auth none) | 66 |
| `AUTH_POLKIT` (sent when the socket asks for polkit; not exercised, the test host has no polkit) | 70 |
| `CONNECT_OPEN` (`qemu:///system`, `qemu:///session`) | 1 |
| `CONNECT_GET_VERSION`, `CONNECT_GET_LIB_VERSION` | 4, 157 |
| `DOMAIN_CREATE_XML` (transient; flags `PAUSED`, `AUTODESTROY`) | 10 |
| `DOMAIN_LOOKUP_BY_NAME`, `DOMAIN_RESUME` | 23, 28 |
| `DOMAIN_GET_STATE` (implemented; no test calls it yet) | 212 |
| `DOMAIN_DESTROY` | 12 |
| `DOMAIN_OPEN_CONSOLE` with stream data in both directions, finish and abort | 201 |
| `CONNECT_CLOSE` | 2 |
| keepalive `PING` answered with `PONG` | `0x6b656570` / 1, 2 |

Reply errors decode as `virNetMessageError`, so the page sees libvirt's
own message, for example "Domain not found: no domain with matching name
...".

Not implemented:

- SASL. libvirt's TCP default is `auth_tcp = "sasl"`. Supporting it would
  mean a SCRAM-SHA-256 (or GSSAPI) client in wasm, driving
  `AUTH_SASL_INIT/START/STEP` (67-69). SCRAM gives no encryption layer, so
  it would still need TLS.
- TLS on 16514. This would need rustls in wasm and a client certificate
  signed by the host's libvirt CA, kept in the browser.
- Events (`VIR_NET_MESSAGE` callbacks). They are ignored.
- File-descriptor passing. A relay cannot carry descriptors anyway.

## Using it

Rust, sans-IO. The same `Connection` drives the unit tests, the blocking
driver and the wasm wrapper.

```rust
let mut c = losos_lab_virt::Connection::new();
let serial = c.connect_open(Some("qemu:///system"), 0);
transport.write_all(&c.take_output())?;
for ev in c.feed(&bytes_from_daemon)? { /* Event::Reply / StreamData / StreamEnd / StreamError */ }
```

`blocking::Blocking` wraps any `Read + Write`, such as a `UnixStream` or
a `TcpStream`, and is what `tests/live.rs` uses.

JavaScript, through the wasm-bindgen wrapper (`src/web.rs`):

```js
import init, { VirtClient } from './pkg/losos_lab_virt.js';
await init();
const { ticket } = await (await fetch('http://127.0.0.1:8095/lab/v1/virt-ticket', { method: 'POST' })).json();
const virt = await VirtClient.connect('ws://127.0.0.1:8095/lab/v1/virt', ['losos-lab', `ticket.${ticket}`]);
// or VirtClient.connect('ws://127.0.0.1:8771/virt?token=…') through ws-relay.mjs,
// or VirtClient.connect('tcp://127.0.0.1:16509') in an IWA
await virt.open('qemu:///system');
await virt.version();                       // { hypervisor: '11.1.0', library: '12.7.0', … }
await virt.createDomain(xml, VirtClient.PAUSED | VirtClient.AUTODESTROY);
const con = await virt.console('pc1');      // con.readable: ReadableStream<Uint8Array>
await virt.resume('pc1');                   // paused first, so the console sees the first byte
con.write('uname -a\r');
await virt.destroy('pc1');
await virt.close();
```

`createDomain` defaults to `AUTODESTROY`, so a guest dies with its
connection: closing the tab ends it. A test confirmed this: the page went
away and the domain was gone. `VirtClient.directSocketsAvailable()` tells
whether `TCPSocket` exists in this page.

## Building

```sh
cargo build --release --target wasm32-unknown-unknown
wasm-bindgen --target web --out-dir demo/pkg \
  target/wasm32-unknown-unknown/release/losos_lab_virt.wasm
```

`wasm-bindgen` in `Cargo.toml` is pinned to exactly the version of
nixpkgs' `wasm-bindgen-cli` (0.2.127). The CLI refuses a module built
against any other version. The release `.wasm` is about 220 KB after
wasm-bindgen.

## Testing

```sh
cargo test                                   # protocol unit tests, no daemon
cargo clippy --all-targets -- -D warnings
cargo clippy --target wasm32-unknown-unknown --all-targets -- -D warnings

# Against a running daemon (each test is skipped without its variables):
LOSOS_VIRT_GUEST=/path/with/bzImage+rootfs.bin \
LOSOS_VIRT_SOCK=/run/libvirt/virtqemud-sock \
LOSOS_VIRT_TCP=127.0.0.1:16509 \
LOSOS_VIRT_EMULATOR=$(command -v qemu-system-x86_64) \
cargo test --test live -- --nocapture

# In a browser: the demo page through the relay
LOSOS_VIRT_SOCKET=/run/libvirt/virtqemud-sock LOSOS_VIRT_TOKEN=secret \
  node relay/ws-relay.mjs --port 8771 --static demo
# open http://127.0.0.1:8771/index.html?token=secret&guest=/path/with/images

# ... or through losos-registrar lab's relay (ws-relay.mjs then only serves
# the page; its own /virt goes unused)
losos-registrar lab --connect 'qemu:///system?socket=/run/libvirt/virtqemud-sock' \
  --origin http://127.0.0.1:8771
# open http://127.0.0.1:8771/index.html?helper=http://127.0.0.1:8095/lab/v1/&guest=/path/with/images
```

The live tests are skipped unless their variables are set, so `cargo
test` in CI and in `devenv test` runs only the protocol tests.

## Security

- **Whoever can reach the relay has libvirt at the relay's privilege.**
  libvirt identifies a unix-socket peer by `SO_PEERCRED`, so it sees the
  relay's uid and never the page's. `qemu:///system` lets a client define
  a domain that mounts any host file, which makes it equivalent to root
  on the host. `AUTH_POLKIT` does not help, because it checks the relay's
  uid too.
- **A relay on loopback is not private.** Any site open in the owner's
  browser can try `ws://127.0.0.1:<port>`. WebSockets are not bound by
  CORS: the browser opens the connection, sends the other site's Origin
  header, and leaves the decision to the server. `ws-relay.mjs` therefore checks `Origin` against
  an allowlist (`LOSOS_VIRT_ORIGINS`), requires a token
  (`LOSOS_VIRT_TOKEN`), binds 127.0.0.1, and refuses unmasked or oversized
  frames. Tested: a foreign Origin, or the right Origin with a wrong
  token, gets 403. On the box, the route belongs behind the admin token
  and the `lanOnly` guard, like `/api/`.
- **`listen_tcp` with `auth_tcp = "none"`** (the Direct Sockets path) gives
  every local user, and every container sharing the network namespace,
  root-equivalent libvirt. Only bind it to 127.0.0.1 on a single-user
  machine. Better, keep the relay with its token, or use SASL over TLS
  (not implemented here).
- **Prefer `qemu:///session`** when the Lab only needs its own throwaway
  guests. A per-user virtqemud runs as that user, so the worst case is
  that user's files, not root. The Lab's guests (TCG, read-only rootfs, no
  host networking) need nothing from the system instance.
- **The page builds the domain XML.** A page that can call
  `DOMAIN_CREATE_XML` can also ask for host block devices, `<qemu:commandline>`
  or a different emulator path. A relay that wants to restrict this has to
  parse libvirt's protocol, which is what this design avoids. Restrict the
  daemon instead: session mode, or a dedicated user.
