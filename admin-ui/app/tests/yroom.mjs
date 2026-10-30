/* A live-editing room in memory, as backend/src/cloud/collab/room.rs keeps
 * one: a Yjs document per file with the text in `content`, y-sync and
 * awareness over the y-websocket framing, every update passed to the other
 * connections. Pages reach it through routeWebSocket in harness.mjs. `hold` keeps a
 * connection's messages back in both directions until it is let go, so a
 * test can make two people change the same file at the same moment. */

import * as Y from 'yjs';
import * as awarenessProtocol from 'y-protocols/awareness';
import * as syncProtocol from 'y-protocols/sync';
import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';

const SYNC = 0;
const AWARENESS = 1;

export function yroom(initial) {
  const doc = new Y.Doc();
  doc.getText('content').insert(0, initial);
  const awareness = new awarenessProtocol.Awareness(doc);
  awareness.setLocalState(null);
  const conns = new Map();

  const sendTo = (conn, bytes) => {
    if (conn.held) conn.out.push(bytes);
    else conn.ws.send(bytes);
  };
  const broadcast = (bytes, except) => {
    for (const c of conns.values()) if (c !== except) sendTo(c, bytes);
  };
  doc.on('update', (update, origin) => {
    const e = encoding.createEncoder();
    encoding.writeVarUint(e, SYNC);
    syncProtocol.writeUpdate(e, update);
    broadcast(encoding.toUint8Array(e), origin);
  });
  awareness.on('update', ({ added, updated, removed }, origin) => {
    const changed = [...added, ...updated, ...removed];
    if (origin && origin.clients) {
      for (const id of added) origin.clients.add(id);
      for (const id of removed) origin.clients.delete(id);
    }
    const e = encoding.createEncoder();
    encoding.writeVarUint(e, AWARENESS);
    encoding.writeVarUint8Array(e, awarenessProtocol.encodeAwarenessUpdate(awareness, changed));
    broadcast(encoding.toUint8Array(e), null);
  });

  const receive = (conn, data) => {
    const d = decoding.createDecoder(new Uint8Array(data));
    const kind = decoding.readVarUint(d);
    if (kind === SYNC) {
      const e = encoding.createEncoder();
      encoding.writeVarUint(e, SYNC);
      syncProtocol.readSyncMessage(d, e, doc, conn);
      if (encoding.length(e) > 1) sendTo(conn, encoding.toUint8Array(e));
    } else if (kind === AWARENESS) {
      awarenessProtocol.applyAwarenessUpdate(awareness, decoding.readVarUint8Array(d), conn);
    }
  };

  return {
    doc,
    text: () => doc.getText('content').toString(),
    /** For routeWebSocket (harness.mjs): `name` names the connection for hold(). */
    connect(ws, name) {
      const conn = { ws, held: false, in: [], out: [], clients: new Set() };
      conns.set(name, conn);
      ws.onMessage((m) => {
        if (conn.held) conn.in.push(m);
        else receive(conn, m);
      });
      ws.onClose(() => {
        conns.delete(name);
        awarenessProtocol.removeAwarenessStates(awareness, [...conn.clients], null);
      });
      const e = encoding.createEncoder();
      encoding.writeVarUint(e, SYNC);
      syncProtocol.writeSyncStep1(e, doc);
      sendTo(conn, encoding.toUint8Array(e));
      const states = [...awareness.getStates().keys()];
      if (states.length > 0) {
        const a = encoding.createEncoder();
        encoding.writeVarUint(a, AWARENESS);
        encoding.writeVarUint8Array(a, awarenessProtocol.encodeAwarenessUpdate(awareness, states));
        sendTo(conn, encoding.toUint8Array(a));
      }
    },
    hold(name) {
      conns.get(name).held = true;
    },
    /** Awareness keeps a timer; without this the test never exits. */
    close() {
      awareness.destroy();
      doc.destroy();
    },
    release(name) {
      const conn = conns.get(name);
      conn.held = false;
      for (const m of conn.in.splice(0)) receive(conn, m);
      for (const b of conn.out.splice(0)) conn.ws.send(b);
    },
  };
}
