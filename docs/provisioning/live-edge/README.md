# Trying a box against the live edge

For the person who runs the edge. A session cannot do this: it reaches the
internet only through an HTTPS proxy, so the tunnel's ports (UDP and TCP
2333) are out of reach. Run it from a machine with a plain connection.

## 1. Does this network reach the edge?

```sh
bun nix/demo/live-edge/check.ts https://register.dasmat.us
```

It asks the control plane for `/health`, the tunnel pin, the gateway key and
the manage page, makes one `POST /register` with a made-up id (the edge
answers 401 and keeps nothing), then knocks on the tunnel: TCP 2333, and a
QUIC packet on UDP 2333 that a listening tunnel answers with a Version
Negotiation. Any `FAIL` names what is closed.

## 2. Let a test box in

The edge registers only boxes in `losos.edge.tenants`. Add one to the edge's
configuration (an id, a hostname under the zone, a token file) and deploy it.
That is a change to the live edge, so it is the operator's to make. On the
box, `losos.proxy.registrarUrl` is `https://register.dasmat.us` by default, and
the box's token is the one in the tenant's token file (`losos.proxy.tokenFile`).

## 3. A file through the edge

`nix build .#checks.x86_64-linux.losos-live-edge-proof` boots a box, signs in
with a browser, uploads a text file in LosOS Cloud and reads it back from the
box's WebDAV address with an app password, from loopback, as the tunnel
delivers it. Once the box is connected, the same request from outside is:

```sh
curl -u 'notshared:<app password>' https://<box>.<zone>/remote.php/webdav/proof.txt
```

Only an app password works through the tunnel; a LosOS Login password is
refused off the LAN. The app password comes from the Devices pane or the
desktop client's login flow.
