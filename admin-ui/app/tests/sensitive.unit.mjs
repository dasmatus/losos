/* The private-details warning's finder (src/lib/sensitive.ts): card
 * numbers, IBANs, passport and ID card numbers, birth numbers and Social
 * Security numbers it must catch, and the look-alikes it must leave alone.
 * Every number here is a published test or specimen value.
 *
 *   bun tests/sensitive.unit.mjs
 */

import assert from './assert.mjs';

const { findDetails, detailSpans, blackOut, luhn, ibanValid, mrzCheck, birthNumberValid } = await import('../src/lib/sensitive.ts');

const kinds = (text) => findDetails(text).map((d) => `${d.kind}:${d.last}`);

// The check digits themselves.
assert.ok(luhn('4242424242424242'));
assert.ok(!luhn('4242424242424241'));
assert.ok(ibanValid('GB82WEST12345698765432'));
assert.ok(!ibanValid('GB82WEST12345698765433'));
assert.equal(mrzCheck('L898902C3'), 6);
assert.equal(mrzCheck('740812'), 2);
assert.equal(mrzCheck('120415'), 9);
assert.ok(birthNumberValid('9001010018'));
assert.ok(birthNumberValid('8551011238'), 'a woman: the month plus 50');
assert.ok(!birthNumberValid('9001010019'));
assert.ok(!birthNumberValid('9013010012'), 'no thirteenth month');

// Cards: the networks' test numbers, written the ways people write them.
assert.deepEqual(kinds('my card is 4242 4242 4242 4242, exp 12/29'), ['card:4242']);
assert.deepEqual(kinds('4111-1111-1111-1111'), ['card:1111']);
assert.deepEqual(kinds('Mastercard 5555555555554444'), ['card:4444']);
assert.deepEqual(kinds('amex 3782 822463 10005'), ['card:0005']);
assert.deepEqual(kinds('twice: 4242424242424242 and 4242 4242 4242 4242'), ['card:4242']);
// Not cards: a wrong last digit, no network, a phone, a long reference.
assert.deepEqual(kinds('4242 4242 4242 4241'), []);
assert.deepEqual(kinds('order 79927398713000'), []);
assert.deepEqual(kinds('call me on +421 905 123 456'), []);
assert.deepEqual(kinds('tracking 1Z999AA10123456784'), []);
assert.deepEqual(kinds('0000 0000 0000 0000'), []);

// IBANs: grouped, run together, followed by words; a wrong check is not.
assert.deepEqual(kinds('IBAN: SK31 1200 0000 1987 4263 7541 thanks'), ['iban:7541']);
assert.deepEqual(kinds('send it to DE89370400440532013000 please'), ['iban:3000']);
assert.deepEqual(kinds('gb82 west 1234 5698 7654 32'), ['iban:5432']);
assert.deepEqual(kinds('DE89 3704 0044 0532 0130 01'), []);
assert.deepEqual(kinds('ZZ12 3456 7890 1234 5678'), []);
// The IBAN's digits are not taken for a card on their own.
assert.ok(!kinds('SK31 1200 0000 1987 4263 7541').some((k) => k.startsWith('card')));

// Passports and ID cards: ICAO 9303's specimen zones, and a number named.
const passport = 'P<UTOERIKSSON<<ANNA<MARIA<<<<<<<<<<<<<<<<<<<\nL898902C36UTO7408122F1204159ZE184226B<<<<<10';
assert.deepEqual(kinds(passport), ['document:02C3']);
const idCard = 'I<UTOD231458907<<<<<<<<<<<<<<<\n7408122F1204159UTO<<<<<<<<<<<6\nERIKSSON<<ANNA<MARIA<<<<<<<<<<';
assert.deepEqual(kinds(idCard), ['document:5890']);
assert.deepEqual(kinds('7408122F1204159UTO<<<<<<<<<<<6'), ['document:']);
assert.deepEqual(kinds('My passport number is EA1234567.'), ['document:4567']);
assert.deepEqual(kinds('číslo OP: EA123456'), ['document:3456']);
assert.deepEqual(kinds('Meine Ausweisnummer ist L01X00T47'), ['document:0T47']);
// A document word with no number after it, or a number of another kind.
assert.deepEqual(kinds('my passport photo is ready'), []);
assert.deepEqual(kinds('passport renewal costs 70 EUR'), []);
assert.deepEqual(kinds('L898902C37UTO7408122F1204159ZE184226B<<<<<10'), [], 'a wrong check digit');

// Birth numbers: with the slash anywhere, without it only when named.
assert.deepEqual(kinds('narodený 900101/0018'), ['birthNumber:']);
assert.deepEqual(kinds('rodné číslo 8551011238'), ['birthNumber:']);
assert.deepEqual(kinds('call 0905121233'), [], 'ten digits alone could be a phone');
assert.deepEqual(kinds('900101/0019'), []);

// Social Security numbers, dashed, in the ranges the SSA issues.
assert.deepEqual(kinds('SSN 123-45-6789'), ['ssn:6789']);
assert.deepEqual(kinds('000-12-3456 and 666-12-3456 and 900-12-3456'), []);
assert.deepEqual(kinds('on 2024-10-22 at 10:30'), []);

// Where each one is, for drawing it black: the number itself, separators
// inside it, nothing around it.
const cut = (text) => detailSpans(text).map((d) => text.slice(d.start, d.end));
assert.deepEqual(cut('card 4242 4242 4242 4242, thanks'), ['4242 4242 4242 4242']);
assert.deepEqual(cut('IBAN: SK31 1200 0000 1987 4263 7541 thanks'), ['SK31 1200 0000 1987 4263 7541']);
assert.deepEqual(cut('rodné číslo 8551011238.'), ['8551011238']);
assert.deepEqual(cut('passport EA1234567 and SSN 123-45-6789'), ['EA1234567', '123-45-6789']);
assert.deepEqual(cut(`Here:\n  ${idCard}`), ['I<UTOD231458907<<<<<<<<<<<<<<<', '7408122F1204159UTO<<<<<<<<<<<6']);
assert.deepEqual(cut('twice: 4242424242424242 and 4242 4242 4242 4242').length, 2);

// Plain text, for a desktop notification: squares where the numbers were.
assert.equal(blackOut('Pay SK31 1200 0000 1987 4263 7541 or card 4242 4242 4242 4242.'), 'Pay \u2588\u2588\u2588\u2588 or card \u2588\u2588\u2588\u2588.');
assert.equal(blackOut('Hi Eva, chat is up on the box.'), 'Hi Eva, chat is up on the box.');

// Ordinary messages.
assert.deepEqual(kinds('Hi Eva, chat is up on the box.'), []);
assert.deepEqual(kinds('The meeting is in room 4242, floor 3, 2026-10-22.'), []);

console.log('sensitive: ok');
