/* Private details in a message about to go out (components/details-check.tsx):
 * a payment card number, an IBAN, a passport or ID card number, a Slovak or
 * Czech birth number, a US Social Security number. Each is found by its own
 * check digits or rules, not by its shape alone, so a phone number or an
 * order number rarely trips it. Runs in the page; the text is never sent
 * anywhere to be checked. What is found is named by its last four
 * characters only. */

export type DetailKind = "card" | "iban" | "document" | "birthNumber" | "ssn";

export interface Detail {
  kind: DetailKind;
  /** The last four characters, to name it by; empty for a birth number. */
  last: string;
}

/** Luhn, for card numbers. */
export function luhn(digits: string): boolean {
  let sum = 0;
  for (let i = 0; i < digits.length; i++) {
    let d = digits.charCodeAt(digits.length - 1 - i) - 48;
    if (i % 2 === 1) {
      d *= 2;
      if (d > 9) d -= 9;
    }
    sum += d;
  }
  return sum % 10 === 0;
}

/* The card networks' number ranges and lengths. A number that passes Luhn
 * but starts where no network issues cards is left alone. */
function cardNetwork(n: string): boolean {
  const len = n.length;
  const p2 = Number(n.slice(0, 2));
  const p3 = Number(n.slice(0, 3));
  const p4 = Number(n.slice(0, 4));
  const p6 = Number(n.slice(0, 6));
  if (n.startsWith("4")) return len === 13 || len === 16 || len === 19; // Visa
  if ((p2 >= 51 && p2 <= 55) || (p4 >= 2221 && p4 <= 2720)) return len === 16; // Mastercard
  if (p2 === 34 || p2 === 37) return len === 15; // American Express
  if (p4 === 6011 || p2 === 65 || (p3 >= 644 && p3 <= 649) || (p6 >= 622126 && p6 <= 622925)) return len >= 16; // Discover
  if (p2 === 62) return len >= 16; // UnionPay
  if (p4 >= 3528 && p4 <= 3589) return len >= 16; // JCB
  if ((p3 >= 300 && p3 <= 305) || p2 === 36 || p2 === 38 || p2 === 39) return len >= 14; // Diners Club
  if (p2 === 50 || (p2 >= 56 && p2 <= 69)) return len >= 13; // Maestro
  return false;
}

/* IBAN lengths by country, from the SWIFT registry. */
const IBAN_LENGTH: Record<string, number> = {
  AD: 24, AE: 23, AL: 28, AT: 20, AZ: 28, BA: 20, BE: 16, BG: 22, BH: 22, BR: 29, BY: 28, CH: 21, CR: 22,
  CY: 28, CZ: 24, DE: 22, DK: 18, DO: 28, EE: 20, EG: 29, ES: 24, FI: 18, FO: 18, FR: 27, GB: 22, GE: 22,
  GI: 23, GL: 18, GR: 27, GT: 28, HR: 21, HU: 28, IE: 22, IL: 23, IQ: 23, IS: 26, IT: 27, JO: 30, KW: 30,
  KZ: 20, LB: 28, LC: 32, LI: 21, LT: 20, LU: 20, LV: 21, MC: 27, MD: 24, ME: 22, MK: 19, MR: 27, MT: 31,
  MU: 30, NL: 18, NO: 15, PK: 24, PL: 28, PS: 29, PT: 25, QA: 29, RO: 24, RS: 22, SA: 24, SC: 31, SE: 24,
  SI: 19, SK: 24, SM: 27, ST: 25, SV: 28, TL: 23, TN: 24, TR: 26, UA: 29, VA: 22, VG: 24, XK: 20,
};

/** ISO 7064 mod 97-10, as an IBAN is checked. */
export function ibanValid(iban: string): boolean {
  const moved = iban.slice(4) + iban.slice(0, 4);
  let rest = 0;
  for (const ch of moved) {
    const value = ch >= "A" ? ch.charCodeAt(0) - 55 : ch.charCodeAt(0) - 48;
    rest = value > 9 ? (rest * 100 + value) % 97 : (rest * 10 + value) % 97;
  }
  return rest === 1;
}

/** ICAO 9303's check digit: weights 7, 3, 1; A is 10, < is 0. */
export function mrzCheck(field: string): number {
  const weights = [7, 3, 1];
  let sum = 0;
  for (let i = 0; i < field.length; i++) {
    const ch = field[i] as string;
    const value = ch === "<" ? 0 : ch >= "A" ? ch.charCodeAt(0) - 55 : ch.charCodeAt(0) - 48;
    sum += value * (weights[i % 3] as number);
  }
  return sum % 10;
}

function checked(field: string, digit: string | undefined): boolean {
  return digit !== undefined && /^\d$/.test(digit) && mrzCheck(field) === Number(digit);
}

const DATE = /^\d\d(0[1-9]|1[0-2])(0[1-9]|[12]\d|3[01])$/;

/* A machine-readable zone line of a passport (44 characters), a visa or an
 * older ID card (36) or an ID card (30). Lines without a number and its
 * check digit, or without two dates and theirs, are not taken. */
function mrzLine(line: string): string | null {
  if (line.length === 44 || line.length === 36) {
    // Number, check, nationality, birth date, check, sex, expiry, check.
    const number = line.slice(0, 9);
    const birth = line.slice(13, 19);
    const expiry = line.slice(21, 27);
    if (checked(number, line[9]) && DATE.test(birth) && checked(birth, line[19]) && DATE.test(expiry) && checked(expiry, line[27])) {
      return number.replace(/<+$/, "");
    }
    return null;
  }
  if (line.length === 30) {
    // An ID card's first line: type, issuer, number, check.
    if (/^[ACI][A-Z<][A-Z<]{3}/.test(line) && /\d/.test(line.slice(5, 14)) && checked(line.slice(5, 14), line[14])) {
      return line.slice(5, 14).replace(/<+$/, "");
    }
    // Its second: birth date, check, sex, expiry, check.
    const birth = line.slice(0, 6);
    const expiry = line.slice(8, 14);
    if (DATE.test(birth) && checked(birth, line[6]) && DATE.test(expiry) && checked(expiry, line[14])) return "";
  }
  return null;
}

/* "passport number EA1234567", "číslo OP: EA123456", "Ausweisnummer
 * L01X00T47": a word for a travel or identity document, then within a few
 * words a number of six to ten capitals and digits, four or more of them
 * digits. */
const DOCUMENT_WORD =
  /(?<![\p{L}\p{N}])(passport|id card|identity card|id number|id no|national id|driver'?s licen[cs]e|pas|pasu|cestovn\S* pas\S*|občiansk\S* preukaz\S*|op|rodn\S* list|reisepass\S*|pass-?nr|passnummer|personalausweis\S*|ausweis\S*|führerschein\S*|vodičsk\S* preukaz\S*)(?![\p{L}\p{N}])/giu;
const DOCUMENT_NUMBER = /(?<![\p{L}\p{N}])([A-Z0-9]{6,10})(?![\p{L}\p{N}])/gu;

/* A birth number with its slash (YYMMDD/NNNN), or without it after the
 * words that name one. Ten digits divisible by 11, or the older rule where
 * the remainder is 10 and the last digit 0. */
const BIRTH_SLASH = /(?<![\d/])(\d{6})\/(\d{4})(?![\d/])/g;
const BIRTH_WORD = /(?<![\p{L}\p{N}])(rodné číslo|rodného čísla|rč|r\.č\.|birth number|geburtsnummer)(?![\p{L}\p{N}])[^\n\d]{0,20}(\d{10})(?!\d)/giu;

export function birthNumberValid(digits: string): boolean {
  if (!/^\d{10}$/.test(digits)) return false;
  const month = Number(digits.slice(2, 4));
  const day = Number(digits.slice(4, 6));
  const m = month > 70 ? month - 70 : month > 50 ? month - 50 : month > 20 ? month - 20 : month;
  if (m < 1 || m > 12 || day < 1 || day > 31) return false;
  const whole = BigInt(digits);
  if (whole % 11n === 0n) return true;
  return Number(BigInt(digits.slice(0, 9)) % 11n) === 10 && digits.endsWith("0");
}

const SSN = /(?<![\d-])(\d{3})-(\d{2})-(\d{4})(?![\d-])/g;

/* Thirteen to nineteen digits in one run, single spaces or dashes between
 * groups, with no digit or letter right before and no digit right after. */
const CARD = /(?<!\d[ -]?)(?<![\p{L}])(?:\d[ -]?){12,18}\d(?![ -]?\d)/gu;
const IBAN = /(?<![\p{L}\p{N}])([A-Za-z]{2})(\d{2})((?: ?[A-Za-z0-9]){10,32})/gu;

/** Where one private detail sits in a text. */
export interface DetailSpan extends Detail {
  start: number;
  end: number;
}

interface Found extends DetailSpan {
  /** What makes two finds the same detail. */
  value: string;
}

/* Every place in `text` that holds a private detail, in order, none
 * overlapping. */
function scan(text: string): Found[] {
  const found: Found[] = [];
  const add = (kind: DetailKind, value: string, start: number, end: number, last = value.slice(-4)) => {
    found.push({ kind, value, last, start, end });
  };

  for (const m of text.matchAll(CARD)) {
    const digits = m[0].replace(/[ -]/g, "");
    if (luhn(digits) && cardNetwork(digits) && !/^(\d)\1+$/.test(digits)) add("card", digits, m.index, m.index + m[0].length);
  }

  for (const m of text.matchAll(IBAN)) {
    const country = (m[1] as string).toUpperCase();
    const length = IBAN_LENGTH[country];
    if (length === undefined) continue;
    const compact = `${country}${m[2]}${(m[3] as string).replace(/ /g, "")}`.toUpperCase();
    if (compact.length < length) continue;
    const iban = compact.slice(0, length);
    if (!/^[A-Z]{2}\d{2}[A-Z0-9]+$/.test(iban) || !ibanValid(iban)) continue;
    // Where the IBAN's last character is in the text, spaces counted.
    let end = m.index;
    for (let seen = 0; seen < length; end++) if (text[end] !== " ") seen++;
    add("iban", iban, m.index, end);
  }

  // A machine-readable zone, line by line. An ID card's zone has its number
  // on one line and the dates on the next: both are blacked out, and the
  // card is named once, by its number when the number is there.
  let at = 0;
  for (const raw of text.split("\n")) {
    const line = raw.trim().replace(/ /g, "").toUpperCase();
    const start = at + (raw.length - raw.trimStart().length);
    const end = at + raw.trimEnd().length;
    at += raw.length + 1;
    if (!/^[A-Z0-9<]+$/.test(line) || !line.includes("<")) continue;
    const number = mrzLine(line);
    if (number === "") add("document", "", start, end, "");
    else if (number !== null) add("document", number, start, end);
  }

  for (const word of text.matchAll(DOCUMENT_WORD)) {
    const from = word.index + word[0].length;
    const after = (text.slice(from).split("\n")[0] ?? "").slice(0, 32);
    for (const m of after.matchAll(DOCUMENT_NUMBER)) {
      const value = m[1] as string;
      if ((value.match(/\d/g) ?? []).length >= 4) {
        add("document", value, from + m.index, from + m.index + value.length);
        break;
      }
    }
  }

  for (const m of text.matchAll(BIRTH_SLASH)) {
    const digits = `${m[1]}${m[2]}`;
    if (birthNumberValid(digits)) add("birthNumber", digits, m.index, m.index + m[0].length, "");
  }
  for (const m of text.matchAll(BIRTH_WORD)) {
    const digits = m[2] as string;
    const end = m.index + m[0].length;
    if (birthNumberValid(digits)) add("birthNumber", digits, end - digits.length, end, "");
  }

  for (const m of text.matchAll(SSN)) {
    const [area, group, serial] = [Number(m[1]), Number(m[2]), Number(m[3])];
    if (area !== 0 && area !== 666 && area < 900 && group !== 0 && serial !== 0) {
      add("ssn", `${m[1]}${m[2]}${m[3]}`, m.index, m.index + m[0].length);
    }
  }

  // In order; where two overlap, the earlier and then the longer wins.
  found.sort((a, b) => a.start - b.start || b.end - a.end);
  const kept: Found[] = [];
  for (const f of found) if (kept.length === 0 || f.start >= (kept.at(-1) as Found).end) kept.push(f);
  return kept;
}

/** Every place in `text` that holds a private detail, for drawing it
 *  blacked out. */
export function detailSpans(text: string): DetailSpan[] {
  return scan(text).map(({ kind, last, start, end }) => ({ kind, last, start, end }));
}

/** Every private detail in `text`, each once, in the order found. */
export function findDetails(text: string): Detail[] {
  const all = scan(text);
  const named = all.some((f) => f.kind === "document" && f.value !== "");
  const found: Detail[] = [];
  const seen = new Set<string>();
  for (const f of all) {
    if (f.kind === "document" && f.value === "" && named) continue;
    const key = `${f.kind}:${f.value}`;
    if (seen.has(key)) continue;
    seen.add(key);
    found.push({ kind: f.kind, last: f.last });
  }
  return found;
}
