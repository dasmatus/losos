/* The messages between a call page and its e2ee worker. */

import type { Kind } from './e2ee';

export interface TransformOptions {
  op: 'seal' | 'open';
  kind: Kind;
  /* The SFU stream id (`p<pid>`) of the member whose frames these are. */
  sender?: string;
}

export type WorkerIn =
  | { type: 'port'; port: MessagePort }
  | { type: 'sendKey'; key: Uint8Array; index: number }
  | { type: 'recvKey'; sender: string; key: Uint8Array; index: number }
  | { type: 'dropSender'; sender: string }
  | { type: 'stream'; readable: ReadableStream; writable: WritableStream; options: TransformOptions }
  | { type: 'stop' };

export interface WorkerStats {
  /* Frames this browser encrypted and sent. */
  sealed: number;
  /* Frames received and decrypted. */
  opened: number;
  /* Frames that failed authentication: wrong key, or changed on the way. */
  refused: number;
  /* Frames dropped because no key was there yet, either way. */
  withoutKey: number;
}

export type WorkerOut = { type: 'stats'; stats: WorkerStats };
