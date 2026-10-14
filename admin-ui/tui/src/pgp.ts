/* OpenPGP, in process (openpgp.js). The user's key signs every request and
 * decrypts every answer; the host key, pinned at login, is what requests are
 * encrypted to and what every answer must be signed by. */

import * as openpgp from "openpgp";
import { t } from "./lang";

export type PrivateKey = openpgp.PrivateKey;
export type PublicKey = openpgp.PublicKey;

/** A user key that fails one of the contract's requirements. */
export class KeyProblem extends Error {
  constructor(message: string) {
    super(message);
    this.name = "KeyProblem";
  }
}

export function fingerprintOf(key: openpgp.Key): string {
  return key.getFingerprint().toUpperCase();
}

/** "ABCD EF01 2345 ..." for reading aloud and comparing by eye. */
export function groupFingerprint(fingerprint: string): string {
  return fingerprint
    .replace(/\s+/g, "")
    .toUpperCase()
    .replace(/(.{4})(?=.)/g, "$1 ");
}

export function sameFingerprint(a: string, b: string): boolean {
  return a.replace(/\s+/g, "").toUpperCase() === b.replace(/\s+/g, "").toUpperCase();
}

export function userIdsOf(key: openpgp.Key): string[] {
  return key.getUserIDs();
}

/* Read an armoured secret key and check the contract's requirements: it is a
 * secret key, its primary can sign, and it has an encryption subkey. A key
 * under a passphrase is checked as it is; nothing here needs the passphrase. */
export async function readUserKey(armored: string): Promise<PrivateKey> {
  let key: openpgp.Key;
  try {
    key = await openpgp.readKey({ armoredKey: armored });
  } catch {
    throw new KeyProblem(t("tui.key.notArmoured"));
  }
  if (!key.isPrivate()) throw new KeyProblem(t("tui.key.notSecret"));
  const privateKey = key as PrivateKey;
  try {
    await privateKey.getSigningKey();
  } catch {
    throw new KeyProblem(t("tui.key.cannotSign"));
  }
  let encryption: Awaited<ReturnType<PrivateKey["getEncryptionKey"]>>;
  try {
    encryption = await privateKey.getEncryptionKey();
  } catch {
    throw new KeyProblem(t("tui.key.noEncryption"));
  }
  if (encryption.getKeyID().equals(privateKey.getKeyID())) {
    throw new KeyProblem(t("tui.key.noEncryption"));
  }
  return privateKey;
}

export async function readHostKey(armored: string): Promise<PublicKey> {
  const key = await openpgp.readKey({ armoredKey: armored });
  return key.toPublic();
}

export function needsPassphrase(key: PrivateKey): boolean {
  return !key.isDecrypted();
}

/** The key, unlocked. Throws KeyProblem on a wrong passphrase. */
export async function unlock(key: PrivateKey, passphrase: string): Promise<PrivateKey> {
  if (key.isDecrypted()) return key;
  try {
    return await openpgp.decryptKey({ privateKey: key, passphrase });
  } catch {
    throw new KeyProblem(t("tui.key.wrongPassphrase"));
  }
}
