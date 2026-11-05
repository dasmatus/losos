/* A live-editing room in memory, as backend/src/cloud/collab/room.rs keeps
 * one: a Yjs document per file with the text in `content`, y-sync and
 * awareness over the y-websocket framing, every update passed to the other
 * connections. Pages reach it through routeWebSocket in harness.ts. `hold` keeps a
 * connection's messages back in both directions until it is let go, so a
 * test can make two people change the same file at the same moment. */

import * as Y from 'yjs';
import * as awarenessProtocol from 'y-protocols/awareness';
import * as syncProtocol from 'y-protocols/sync';
import * as decoding from 'lib0/decoding';
import * as encoding from 'lib0/encoding';
import type { FakeSocket } from './harness';

const SYNC = 0;
const AWARENESS = 1;

interface RoomConn {
  ws: FakeSocket;
  held: boolean;
  in: (string | Uint8Array)[];
  out: (string | Uint8Array)[];
  clients: Set<number>;
}

export function yroom(initial: string) {
  const doc = new Y.Doc();
  doc.getText('content').insert(0, initial);
  const awareness = new awarenessProtocol.Awareness(doc);
  awareness.setLocalState(null);
  const conns = new Map<string, RoomConn>();

  const sendTo = (conn: RoomConn, bytes: Uint8Array) => {
    if (conn.held) conn.out.push(bytes);
    else conn.ws.send(bytes);
  };
  const broadcast = (bytes: Uint8Array, except: unknown) => {
    for (const c of conns.values()) if (c !== except) sendTo(c, bytes);
  };
  doc.on('update', (update: Uint8Array, origin: unknown) => {
    const e = encoding.createEncoder();
    encoding.writeVarUint(e, SYNC);
    syncProtocol.writeUpdate(e, update);
    broadcast(encoding.toUint8Array(e), origin);
  });
  awareness.on('update', (payload: unknown, origin: unknown) => {
    const { added, updated, removed } = payload as { added: number[]; updated: number[]; removed: number[] };
    const changed = [...added, ...updated, ...removed];
    const from = origin as { clients?: Set<number> } | null | undefined;
    if (from && from.clients) {
      for (const id of added) from.clients.add(id);
      for (const id of removed) from.clients.delete(id);
    }
    const e = encoding.createEncoder();
    encoding.writeVarUint(e, AWARENESS);
    encoding.writeVarUint8Array(e, awarenessProtocol.encodeAwarenessUpdate(awareness, changed));
    broadcast(encoding.toUint8Array(e), null);
  });

  const receive = (conn: RoomConn, data: string | Uint8Array) => {
    const d = decoding.createDecoder(typeof data === 'string' ? new TextEncoder().encode(data) : new Uint8Array(data));
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
    /** For routeWebSocket (harness.ts): `name` names the connection for hold(). */
    connect(ws: FakeSocket, name: string) {
      const conn: RoomConn = { ws, held: false, in: [], out: [], clients: new Set() };
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
    hold(name: string) {
      const conn = conns.get(name);
      if (conn) conn.held = true;
    },
    /** Awareness keeps a timer; without this the test never exits. */
    close() {
      awareness.destroy();
      doc.destroy();
    },
    release(name: string) {
      const conn = conns.get(name);
      if (!conn) return;
      conn.held = false;
      for (const m of conn.in.splice(0)) receive(conn, m);
      for (const b of conn.out.splice(0)) conn.ws.send(b);
    },
  };
}
