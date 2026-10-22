/* BlurHash (https://blurha.sh): a picture as a few colour waves in a short
 * string. Chat puts one in every picture it sends (MSC2448), so a blurred
 * picture can be drawn as its colours before anyone fetches the bytes.
 * Written here rather than pulled in: the whole algorithm is these lines. */

const DIGITS = "0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz#$%*+,-.:;=?@[]^_{|}~";

function encode83(value: number, length: number): string {
  let out = "";
  for (let i = 1; i <= length; i += 1) {
    const digit = Math.floor(value / 83 ** (length - i)) % 83;
    out += DIGITS[digit];
  }
  return out;
}

function decode83(text: string): number {
  let value = 0;
  for (const c of text) {
    const digit = DIGITS.indexOf(c);
    if (digit < 0) return Number.NaN;
    value = value * 83 + digit;
  }
  return value;
}

function toLinear(value: number): number {
  const v = value / 255;
  return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4;
}

function toSrgb(value: number): number {
  const v = Math.max(0, Math.min(1, value));
  return v <= 0.0031308 ? Math.round(v * 12.92 * 255 + 0.5) : Math.round((1.055 * v ** (1 / 2.4) - 0.055) * 255 + 0.5);
}

const signPow = (value: number, exp: number): number => Math.sign(value) * Math.abs(value) ** exp;

/** The hash of `width` × `height` RGBA pixels, `cx` × `cy` waves (1 to 9). */
export function encode(pixels: Uint8ClampedArray, width: number, height: number, cx = 4, cy = 3): string {
  const factors: [number, number, number][] = [];
  for (let j = 0; j < cy; j += 1) {
    for (let i = 0; i < cx; i += 1) {
      const norm = i === 0 && j === 0 ? 1 : 2;
      let r = 0;
      let g = 0;
      let b = 0;
      for (let y = 0; y < height; y += 1) {
        for (let x = 0; x < width; x += 1) {
          const basis = norm * Math.cos((Math.PI * i * x) / width) * Math.cos((Math.PI * j * y) / height);
          const at = 4 * (y * width + x);
          r += basis * toLinear(pixels[at] ?? 0);
          g += basis * toLinear(pixels[at + 1] ?? 0);
          b += basis * toLinear(pixels[at + 2] ?? 0);
        }
      }
      const scale = 1 / (width * height);
      factors.push([r * scale, g * scale, b * scale]);
    }
  }
  const [dc, ...ac] = factors as [[number, number, number], ...[number, number, number][]];
  let hash = encode83(cx - 1 + (cy - 1) * 9, 1);
  let max = 1;
  if (ac.length > 0) {
    const actual = Math.max(...ac.flat().map(Math.abs));
    const quantised = Math.floor(Math.max(0, Math.min(82, Math.floor(actual * 166 - 0.5))));
    max = (quantised + 1) / 166;
    hash += encode83(quantised, 1);
  } else {
    hash += encode83(0, 1);
  }
  hash += encode83((toSrgb(dc[0]) << 16) + (toSrgb(dc[1]) << 8) + toSrgb(dc[2]), 4);
  for (const [r, g, b] of ac) {
    const q = (v: number) => Math.floor(Math.max(0, Math.min(18, Math.floor(signPow(v / max, 0.5) * 9 + 9.5))));
    hash += encode83(q(r) * 19 * 19 + q(g) * 19 + q(b), 2);
  }
  return hash;
}

/** Whether `hash` is one [`decode`] can draw. */
export function valid(hash: unknown): hash is string {
  if (typeof hash !== "string" || hash.length < 6) return false;
  const size = decode83(hash[0] ?? "");
  const waves = (Math.floor(size / 9) + 1) * ((size % 9) + 1);
  return Number.isFinite(size) && hash.length === 4 + 2 * waves && Number.isFinite(decode83(hash));
}

/** RGBA pixels of `hash` at `width` × `height`; null for a hash that is not one. */
export function decode(hash: string, width: number, height: number): Uint8ClampedArray<ArrayBuffer> | null {
  if (!valid(hash)) return null;
  const size = decode83(hash[0] ?? "");
  const cy = Math.floor(size / 9) + 1;
  const cx = (size % 9) + 1;
  const max = (decode83(hash[1] ?? "") + 1) / 166;
  const colours: [number, number, number][] = [];
  for (let i = 0; i < cx * cy; i += 1) {
    if (i === 0) {
      const v = decode83(hash.substring(2, 6));
      colours.push([toLinear(v >> 16), toLinear((v >> 8) & 255), toLinear(v & 255)]);
    } else {
      const v = decode83(hash.substring(4 + i * 2, 6 + i * 2));
      const d = (q: number) => signPow((q - 9) / 9, 2) * max;
      colours.push([d(Math.floor(v / 361)), d(Math.floor(v / 19) % 19), d(v % 19)]);
    }
  }
  const out = new Uint8ClampedArray(new ArrayBuffer(width * height * 4));
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      let r = 0;
      let g = 0;
      let b = 0;
      for (let j = 0; j < cy; j += 1) {
        for (let i = 0; i < cx; i += 1) {
          const basis = Math.cos((Math.PI * x * i) / width) * Math.cos((Math.PI * y * j) / height);
          const c = colours[i + j * cx] ?? [0, 0, 0];
          r += c[0] * basis;
          g += c[1] * basis;
          b += c[2] * basis;
        }
      }
      const at = 4 * (y * width + x);
      out[at] = toSrgb(r);
      out[at + 1] = toSrgb(g);
      out[at + 2] = toSrgb(b);
      out[at + 3] = 255;
    }
  }
  return out;
}

/** The hash of a picture file, drawn small first; null when the browser
 *  cannot read it. */
export async function ofFile(file: Blob): Promise<string | null> {
  try {
    const bitmap = await createImageBitmap(file, { resizeWidth: 32, resizeHeight: 32, resizeQuality: "low" });
    const canvas = new OffscreenCanvas(32, 32);
    const context = canvas.getContext("2d");
    if (context === null) return null;
    context.drawImage(bitmap, 0, 0);
    bitmap.close();
    return encode(context.getImageData(0, 0, 32, 32).data, 32, 32);
  } catch {
    return null;
  }
}
