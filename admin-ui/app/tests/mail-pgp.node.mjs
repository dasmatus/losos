/* OpenPGP in LosOS Mail (src/mail/pgp.ts), checked without a browser: keys
 * made and imported the way the Encryption dialog does it, mail sealed the
 * way the composer does, wrapped in the multipart lososd writes around it
 * (backend/src/mail.rs raw_message), and opened again the way the reader
 * does. Where GnuPG is on the PATH, it reads what the page writes and the
 * page reads what GnuPG writes. nix/tests/mail-pgp.nix runs the same thing
 * through a real Stalwart and a real browser.
 *
 *   node tests/mail-pgp.node.mjs
 */

import assert from 'node:assert/strict';
import { execFileSync, spawnSync } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import * as esbuild from 'esbuild';

// Bundled next to node_modules, with OpenPGP.js and postal-mime left as
// imports, so Node loads its own build of each.
const outfile = fileURLToPath(new URL('../node_modules/.cache/losos-tests/mail-pgp.mjs', import.meta.url));
await esbuild.build({
  entryPoints: [fileURLToPath(new URL('../src/mail/pgp.ts', import.meta.url))],
  bundle: true,
  format: 'esm',
  platform: 'node',
  outfile,
  external: ['openpgp', 'postal-mime'],
  alias: { '@': fileURLToPath(new URL('../src', import.meta.url)) },
});
const pgpTs = await import(pathToFileURL(outfile).href);
const { makeKey, importKey, unlockKey, unlockedKey, lockKey, seal, openMail, keyFacts, quotedPrintable, ImportProblem, CannotOpen, WrongPassphrase } = pgpTs;
const openpgp = await import('openpgp');

let passed = 0;
async function check(name, fn) {
  try {
    await fn();
    passed += 1;
    console.log(`ok   ${name}`);
  } catch (error) {
    console.error(`FAIL ${name}`);
    throw error;
  }
}

const enc = new TextEncoder();
const crlf = (s) => s.replace(/\r?\n/g, '\r\n');

/* The message lososd sends for a PGP body, as backend/src/mail.rs writes it. */
function raw({ from, to, subject, pgp }) {
  const b = 'losos-test-boundary';
  let out = `From: ${from}\r\nTo: ${to}\r\nSubject: ${subject}\r\nMIME-Version: 1.0\r\n`;
  if (pgp.mime === 'encrypted') {
    out += `Content-Type: multipart/encrypted;\r\n protocol="application/pgp-encrypted";\r\n boundary="${b}"\r\n\r\n`;
    out += 'This is an OpenPGP/MIME encrypted message (RFC 4880 and 3156)\r\n';
    out += `--${b}\r\nContent-Type: application/pgp-encrypted\r\nContent-Description: PGP/MIME version identification\r\n\r\nVersion: 1\r\n\r\n`;
    out += `--${b}\r\nContent-Type: application/octet-stream; name="encrypted.asc"\r\nContent-Description: OpenPGP encrypted message\r\nContent-Disposition: inline; filename="encrypted.asc"\r\n\r\n`;
    out += crlf(pgp.armored) + `\r\n\r\n--${b}--\r\n`;
  } else {
    out += `Content-Type: multipart/signed; micalg=${pgp.micalg};\r\n protocol="application/pgp-signature";\r\n boundary="${b}"\r\n\r\n`;
    out += 'This is an OpenPGP/MIME signed message (RFC 4880 and 3156)\r\n';
    out += `--${b}\r\n` + pgp.part + `\r\n--${b}\r\n`;
    out += 'Content-Type: application/pgp-signature; name="signature.asc"\r\nContent-Disposition: attachment; filename="signature.asc"\r\n\r\n';
    out += crlf(pgp.signature) + `\r\n\r\n--${b}--\r\n`;
  }
  return enc.encode(out);
}

function plainRaw({ from, to, subject, text }) {
  return enc.encode(
    `From: ${from}\r\nTo: ${to}\r\nSubject: ${subject}\r\nMIME-Version: 1.0\r\nContent-Type: text/plain; charset=utf-8\r\n\r\n${crlf(text)}\r\n`,
  );
}

const PASS = 'correct horse battery';
const BODY = 'Meet at the lake at six.\nÚdolie, Größe, 🐟\nFrom the shore.\n.\nLast line with a space ';

const anna = await makeKey('Anna', 'anna@box.home.arpa', PASS);
const annaKey = unlockedKey(anna.fingerprint);
const bob = await makeKey('Bob', 'bob@box.home.arpa', PASS);
const bobKey = unlockedKey(bob.fingerprint);
const carol = await makeKey('', 'carol@box.home.arpa', PASS);

await check('a new key is v4 Curve25519, locked, with the address', async () => {
  const locked = await openpgp.readPrivateKey({ armoredKey: anna.lockedKey });
  assert.equal(locked.isDecrypted(), false);
  assert.equal(locked.keyPacket.version, 4);
  assert.equal(locked.getAlgorithmInfo().algorithm, 'eddsaLegacy');
  const facts = await keyFacts(anna.publicKey);
  assert.deepEqual(facts.emails, ['anna@box.home.arpa']);
  assert.equal(facts.canEncrypt, true);
  assert.equal(facts.fingerprint, anna.fingerprint);
  assert.ok(!anna.publicKey.includes('PRIVATE'));
});

await check('the passphrase unlocks the key, a wrong one does not', async () => {
  lockKey();
  assert.equal(unlockedKey(anna.fingerprint), null);
  await assert.rejects(unlockKey(anna.lockedKey, 'not it at all'), WrongPassphrase);
  await unlockKey(anna.lockedKey, PASS);
  assert.notEqual(unlockedKey(anna.fingerprint), null);
  assert.equal(unlockedKey(bob.fingerprint), null);
});

await check('PGP/MIME encrypted and signed: Bob reads it, signed by Anna', async () => {
  const made = await seal({ text: BODY, encryptTo: [bob.publicKey, anna.publicKey], signWith: annaKey, inline: false });
  assert.equal(made.text, '');
  assert.equal(made.pgp.mime, 'encrypted');
  assert.ok(!made.pgp.armored.includes('lake'));
  const message = raw({ from: 'anna@box.home.arpa', to: 'bob@box.home.arpa', subject: 'Lake', pgp: made.pgp });
  const opened = await openMail(message, bobKey, [anna.publicKey]);
  assert.equal(opened.kind, 'encrypted');
  assert.equal(opened.inline, false);
  assert.equal(opened.text.replace(/\r\n/g, '\n').trimEnd(), BODY.trimEnd());
  assert.equal(opened.signature.state, 'good');
  assert.equal(opened.signature.fingerprint, anna.fingerprint);
  assert.deepEqual(opened.signature.emails, ['anna@box.home.arpa']);
  // Anna reads her own copy in Sent.
  const own = await openMail(message, annaKey, [anna.publicKey]);
  assert.equal(own.signature.state, 'good');
});

await check('encrypted without the sender key: signed by a key the box does not know', async () => {
  const made = await seal({ text: BODY, encryptTo: [bob.publicKey], signWith: annaKey, inline: false });
  const opened = await openMail(raw({ from: 'a@x', to: 'b@x', subject: 's', pgp: made.pgp }), bobKey, []);
  assert.equal(opened.signature.state, 'unknown');
  assert.match(opened.signature.keyId, /^[0-9A-F]{16}$/);
});

await check('encrypted only: no signature', async () => {
  const made = await seal({ text: 'hi', encryptTo: [bob.publicKey], signWith: null, inline: false });
  const opened = await openMail(raw({ from: 'a@x', to: 'b@x', subject: 's', pgp: made.pgp }), bobKey, [anna.publicKey]);
  assert.equal(opened.signature.state, 'none');
  assert.equal(opened.text.trim(), 'hi');
});

await check('not for me, and locked, are told apart', async () => {
  const made = await seal({ text: 'secret', encryptTo: [bob.publicKey], signWith: null, inline: false });
  const message = raw({ from: 'a@x', to: 'b@x', subject: 's', pgp: made.pgp });
  const carolKey = await openpgp.decryptKey({
    privateKey: await openpgp.readPrivateKey({ armoredKey: carol.lockedKey }),
    passphrase: PASS,
  });
  await assert.rejects(openMail(message, carolKey, []), (e) => e instanceof CannotOpen && e.kind === 'notForMe');
  await assert.rejects(openMail(message, null, []), (e) => e instanceof CannotOpen && e.kind === 'locked');
});

await check('PGP/MIME signed: good, then tampered', async () => {
  const made = await seal({ text: BODY, encryptTo: [], signWith: annaKey, inline: false });
  assert.equal(made.pgp.mime, 'signed');
  assert.match(made.pgp.micalg, /^pgp-sha(224|256|384|512)$/);
  // What lososd checks before it sends: CRLF, 7-bit, no trailing space.
  assert.ok(/^[\x00-\x7f]*$/.test(made.pgp.part));
  assert.ok(!/[^\r]\n/.test(made.pgp.part));
  assert.ok(!/[ \t]\r\n/.test(made.pgp.part));
  const good = await openMail(raw({ from: 'anna@box.home.arpa', to: 'b@x', subject: 's', pgp: made.pgp }), null, [anna.publicKey]);
  assert.equal(good.kind, 'signed');
  assert.equal(good.signature.state, 'good');
  assert.equal(good.text.replace(/\r\n/g, '\n').trimEnd(), BODY.trimEnd());
  const tampered = { ...made.pgp, part: made.pgp.part.replace('six', 'ten') };
  const bad = await openMail(raw({ from: 'anna@box.home.arpa', to: 'b@x', subject: 's', pgp: tampered }), null, [anna.publicKey]);
  assert.equal(bad.signature.state, 'bad');
});

await check('inline encrypted and inline cleartext signed', async () => {
  const made = await seal({ text: 'inline words', encryptTo: [bob.publicKey], signWith: annaKey, inline: true });
  assert.equal(made.pgp, undefined);
  assert.match(made.text, /^-----BEGIN PGP MESSAGE-----/);
  const opened = await openMail(plainRaw({ from: 'a@x', to: 'b@x', subject: 's', text: made.text }), bobKey, [anna.publicKey]);
  assert.equal(opened.kind, 'encrypted');
  assert.equal(opened.inline, true);
  assert.equal(opened.text.trim(), 'inline words');
  assert.equal(opened.signature.state, 'good');

  const signed = await seal({ text: 'signed words', encryptTo: [], signWith: annaKey, inline: true });
  assert.match(signed.text, /^-----BEGIN PGP SIGNED MESSAGE-----/);
  const read = await openMail(plainRaw({ from: 'a@x', to: 'b@x', subject: 's', text: signed.text }), null, [anna.publicKey]);
  assert.equal(read.kind, 'signed');
  assert.equal(read.text.trim(), 'signed words');
  assert.equal(read.signature.state, 'good');
});

await check('plain mail stays plain', async () => {
  const opened = await openMail(plainRaw({ from: 'a@x', to: 'b@x', subject: 's', text: 'nothing to see' }), null, []);
  assert.equal(opened.kind, 'plain');
  assert.equal(opened.signature.state, 'none');
});

await check('import: wrong passphrase, another address, then a good one relocked', async () => {
  await assert.rejects(importKey(anna.lockedKey, 'wrong wrong wrong', '', 'anna@box.home.arpa'), (e) => e instanceof ImportProblem && e.kind === 'wrongPassphrase');
  await assert.rejects(importKey(anna.lockedKey, PASS, '', 'eva@box.home.arpa'), (e) => e instanceof ImportProblem && e.kind === 'otherAddress' && e.detail === 'anna@box.home.arpa');
  await assert.rejects(importKey(anna.publicKey, '', '', 'anna@box.home.arpa'), (e) => e instanceof ImportProblem && e.kind === 'notPrivate');
  const again = await importKey(anna.lockedKey, PASS, 'a brand new passphrase', 'anna@box.home.arpa');
  assert.equal(again.fingerprint, anna.fingerprint);
  const locked = await openpgp.readPrivateKey({ armoredKey: again.lockedKey });
  assert.equal(locked.isDecrypted(), false);
  await openpgp.decryptKey({ privateKey: locked, passphrase: 'a brand new passphrase' });
});

await check('quoted-printable keeps lines short and ends clean', () => {
  const qp = quotedPrintable(`${'x'.repeat(200)} \nFrom me\n.`);
  for (const line of qp.split('\r\n')) assert.ok(line.length <= 76, line);
  assert.ok(!qp.includes(' \r\n'));
  assert.ok(qp.includes('=46rom'));
  assert.ok(qp.endsWith('=2E'));
});

/* GnuPG, when it is here: it reads the page's mail, and the page reads its. */
const hasGpg = spawnSync('gpg', ['--version']).status === 0;
if (hasGpg) {
  const home = mkdtempSync(join(tmpdir(), 'losos-gpg-'));
  const gpg = (args, input) =>
    execFileSync('gpg', ['--homedir', home, '--batch', '--yes', '--pinentry-mode', 'loopback', '--passphrase', PASS, ...args], {
      input,
      stdio: ['pipe', 'pipe', 'pipe'],
    });
  try {
    gpg(['--import'], bob.lockedKey);
    gpg(['--import'], anna.publicKey);
    await check('GnuPG decrypts and verifies what the page sends', async () => {
      const made = await seal({ text: 'to gpg', encryptTo: [bob.publicKey], signWith: annaKey, inline: false });
      const result = spawnSync('gpg', ['--homedir', home, '--batch', '--pinentry-mode', 'loopback', '--passphrase', PASS, '--status-fd', '2', '--decrypt'], { input: made.pgp.armored });
      assert.equal(result.status, 0, result.stderr.toString());
      assert.match(result.stdout.toString(), /to gpg/);
      assert.match(result.stderr.toString(), new RegExp(`VALIDSIG ${anna.fingerprint}`));
    });
    await check('the page decrypts and verifies what GnuPG sends', async () => {
      gpg(['--import'], anna.lockedKey);
      const armored = gpg(
        ['--armor', '--trust-model', 'always', '--local-user', 'anna@box.home.arpa', '--recipient', 'bob@box.home.arpa', '--sign', '--encrypt'],
        'from gpg\n',
      ).toString();
      const opened = await openMail(plainRaw({ from: 'anna@box.home.arpa', to: 'bob@box.home.arpa', subject: 's', text: armored }), bobKey, [anna.publicKey]);
      assert.equal(opened.text.trim(), 'from gpg');
      assert.equal(opened.signature.state, 'good');
    });
  } finally {
    rmSync(home, { recursive: true, force: true });
  }
} else {
  console.log('skip GnuPG: not on the PATH');
}

console.log(`${passed} passed`);
