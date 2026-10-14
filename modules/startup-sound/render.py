"""The LosOS startup sound: the last seven seconds of the launch film's score,
rendered from code at build time, so no recording sits in the repository.

    python3 render.py out.wav

G major at 120 BPM. On the first downbeat a Gadd9 rings out on a
supersaw pad and a strum of FM bells over a sine bass. A bar later the pad
moves to Cmaj9 and the bells play the logo's motif, G4 D5 A5, on beats 1, 1.5
and 2.5. The rest is the hall reverb ringing out under a fade.

The instruments, the mix and the master are a port of the parts of cinetic's
synth.py, score.py and master.py that this passage uses, under the licence
below. Every voice is deterministic: noise comes from fixed seeds, so the same
code always renders the same samples.

Instruments, mix and master from cinetic (https://github.com/Leonxlnx/cinetic):

MIT License

Copyright (c) 2026 Leonxlnx

Permission is hereby granted, free of charge, to any person obtaining a copy
of this software and associated documentation files (the "Software"), to deal
in the Software without restriction, including without limitation the rights
to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
copies of the Software, and to permit persons to whom the Software is
furnished to do so, subject to the following conditions:

The above copyright notice and this permission notice shall be included in all
copies or substantial portions of the Software.

THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
SOFTWARE.
"""

import sys
import wave

import numpy as np
from scipy import signal

SR = 48000
DUR = 7.0  # seconds, the film's 48.0 s to 55.0 s
BEAT = 0.5


# ---- primitives -------------------------------------------------------------------------
def ns(d):
    return max(1, int(round(d * SR)))


def tt(d):
    return np.arange(ns(d)) / SR


def lp(f, order=2):
    return signal.butter(order, float(np.clip(f, 10, SR * 0.45)), "low", fs=SR, output="sos")


def hp(f, order=2):
    return signal.butter(order, float(np.clip(f, 10, SR * 0.45)), "high", fs=SR, output="sos")


def filt(x, sos):
    return signal.sosfilt(sos, x, axis=0)


def mtof(m):
    return 440.0 * 2 ** ((m - 69) / 12)


def _blep(ph, dt):
    y = np.zeros_like(ph)
    m1 = ph < dt
    t1 = ph[m1] / dt[m1]
    y[m1] = t1 + t1 - t1 * t1 - 1
    m2 = ph > 1 - dt
    t2 = (ph[m2] - 1) / dt[m2]
    y[m2] = t2 * t2 + t2 + t2 + 1
    return y


def sine(freq, d):
    return np.sin(2 * np.pi * np.cumsum(np.full(ns(d), float(freq))) / SR)


def saw(freq, d, phase0=0.0):
    """PolyBLEP band-limited saw."""
    f = np.full(ns(d), float(freq))
    ph, dt = (phase0 + np.cumsum(f) / SR) % 1.0, f / SR
    return 2 * ph - 1 - _blep(ph, dt)


def adsr(d, a=0.005, dcy=0.1, s=0.7, r=0.2):
    n = ns(d)
    env = np.full(n, float(s))
    na, nd, nr = min(n, max(1, int(a * SR))), int(dcy * SR), int(r * SR)
    env[:na] = np.linspace(0, 1, na)
    if na + nd < n:
        env[na:na + nd] = np.linspace(1, s, nd)
    if 0 < nr < n:
        env[-nr:] *= np.linspace(1, 0, nr) ** 1.5
    return env


def sat(x, drive=1.5):
    return np.tanh(x * drive) / np.tanh(drive)


def pan2(x, pan=0.0):
    a = (float(np.clip(pan, -1, 1)) + 1) * np.pi / 4
    return np.stack([x * np.cos(a), x * np.sin(a)], axis=1)


def place(dst, x, t, gain=1.0, pan=0.0, tail=0.006):
    """Add x into the stereo buffer dst at t seconds, with a short fade at its end."""
    i = int(round(t * SR))
    if i >= len(dst):
        return
    x = pan2(x, pan) if x.ndim == 1 else x.copy()
    nf = min(len(x), int(tail * SR))
    if nf > 1:
        x[-nf:] *= (np.cos(np.linspace(0, np.pi / 2, nf)) ** 2)[:, None]
    n = min(len(x), len(dst) - i)
    dst[i:i + n] += x[:n] * gain


# ---- instruments ------------------------------------------------------------------------
def supersaw(freq, d, cutoff, a, r, seed, voices=5, detune=0.11):
    """Pad note: five saws spread over +-detune semitones and across the field."""
    rng = np.random.default_rng(seed)
    out = np.zeros((ns(d), 2))
    for v in range(voices):
        off = (v - (voices - 1) / 2) / ((voices - 1) / 2 + 1e-9) * detune
        s = saw(freq * 2 ** (off / 12), d, rng.random())
        ang = ((v / max(1, voices - 1) - 0.5) * 1.4 + 1) * np.pi / 4
        out[:, 0] += s * np.cos(ang)
        out[:, 1] += s * np.sin(ang)
    out = filt(out / voices, lp(cutoff, 2))
    return out * adsr(d, a, 0.3, 0.85, r)[:, None]


def bass(freq, d, drive=1.2, h2=0.22):
    """Sine bass with its 2nd harmonic and a little saw, low-passed and saturated."""
    x = sine(freq, d) + sine(freq * 2, d) * h2 + saw(freq, d) * 0.06
    x = sat(filt(x, lp(900, 2)), drive) * adsr(d, 0.006, 0.12, 0.8, 0.08)
    k = min(len(x), ns(0.4))
    x[-k:] *= np.linspace(1, 0, k)
    return x


def fm_bell(freq, d, index, ratio, tau):
    d = max(d, 5 * tau)
    t = tt(d)
    mod = np.sin(2 * np.pi * freq * ratio * t) * index * np.exp(-t / 0.35)
    x = np.sin(2 * np.pi * freq * t + mod) * np.exp(-t / tau)
    x += np.sin(2 * np.pi * freq * 2.0 * t) * np.exp(-t / (tau * 0.35)) * 0.15
    return x * np.minimum(1, t / 0.003)


def hall(rt60=2.8, pre=0.025, lpf=5500, seed=3):
    """Stereo noise-tail impulse response with air absorption, unit energy."""
    n = ns(rt60 * 1.2)
    t = np.arange(n) / SR
    rng = np.random.default_rng(seed)
    ir = np.stack([rng.standard_normal(n), rng.standard_normal(n)], axis=1) * np.exp(-6.9 * t / rt60)[:, None]
    ir = filt(ir, lp(lpf, 1))
    ir = np.concatenate([np.zeros((ns(pre), 2)), ir])
    return ir / np.sqrt(np.sum(ir ** 2))


def reverb(x):
    h = hall()
    return np.stack([signal.fftconvolve(x[:, c], h[:, c])[:len(x)] for c in (0, 1)], axis=1)


# ---- the score --------------------------------------------------------------------------
def music(n):
    pad, low, bells = (np.zeros((n, 2)) for _ in range(3))

    # Pad: Gadd9 (G3 B3 D4 A4), then Cmaj9 (E3 B3 D4 C5), each held 4.2 s.
    for at, chord, seeds in ((0.0, (55, 59, 62, 69), (1028, 1045, 1062, 1079)),
                             (2.0, (52, 59, 62, 72), (1029, 1046, 1063, 1080))):
        for m, seed in zip(chord, seeds):
            place(pad, supersaw(mtof(m), 4.2, 1700, 0.02, 2.2, seed), at, 0.187)
    pad = filt(pad, hp(170, 2))

    # Bass: G1 under the first chord, just after the downbeat, then C2.
    place(low, bass(mtof(31), 2.0), 0.06, 0.27)
    place(low, bass(mtof(36), 3.2), 2.0, 0.27)

    # Bells: the Gadd9 strummed upwards (G3 A4 B4 D5), then the motif G4 D5 A5.
    for k, m in enumerate((55, 69, 71, 74)):
        place(bells, fm_bell(mtof(m), 4.5, 2.0, 3.5, 1.8), 0.02 * k, 0.10 - 0.015 * k)
    for k, (m, beat) in enumerate(((67, 0.0), (74, 0.5), (81, 1.5))):
        place(bells, fm_bell(mtof(m), 3.0, 1.4, 3.5, 1.3), 2.0 + beat * BEAT, (0.10 - 0.015 * k) * 0.8,
              0.12 * (k % 2 * 2 - 1))

    send = filt(pad * 0.25 + bells * 0.6, hp(250, 2))
    mix = low * 0.55 + pad * 1.6 + bells * 1.5 + reverb(send) * 0.6

    # Section levels (0.65 for the resolve, 1.0 for the held end) and the film's end fade.
    t = np.arange(n) / SR
    level = 0.65 + 0.35 / (1 + np.exp(-(t - 4.0) / 0.03))
    fade = np.clip((DUR - 0.03 - t) / 1.3, 0, 1) ** 2
    return mix * (level * fade)[:, None]


# ---- master -----------------------------------------------------------------------------
# The film masters its whole soundtrack to -14 LUFS; over this passage that came to 1.05
# (a least-squares fit of this render against the film's own audio), so the sound is as
# loud as it is in the film.
GAIN = 1.05
CEILING = 0.77


def bus_comp(x, thr=-14.0, ratio=2.0, attack=0.015, release=0.12, rms=0.005, hop=48):
    det = signal.sosfilt(signal.butter(1, 1 / (2 * np.pi * rms), "low", fs=SR, output="sos"), np.mean(x ** 2, axis=1))
    nb = int(np.ceil(len(x) / hop))
    d = np.pad(det, (0, nb * hop - len(det)), mode="edge").reshape(nb, hop).max(axis=1)
    target = -np.maximum(0, 10 * np.log10(np.maximum(d, 1e-12)) - thr) * (1 - 1 / ratio)
    att, rel = np.exp(-hop / (attack * SR)), np.exp(-hop / (release * SR))
    g = np.empty(nb)
    cur = 0.0
    for i, v in enumerate(target.tolist()):
        cur = v + (cur - v) * (att if v < cur else rel)
        g[i] = cur
    gr = np.interp(np.arange(len(x)), np.arange(nb) * hop + hop / 2, g)
    return x * (10 ** (gr / 20))[:, None]


def finish(x):
    x = filt(x, hp(28, 2))
    mid = x.mean(axis=1, keepdims=True)  # mono below 120 Hz
    side = filt((x[:, :1] - x[:, 1:]) / 2, hp(120, 2))
    x = np.concatenate([mid + side, mid - side], axis=1)
    x = bus_comp(x)
    x = np.tanh(x * 0.9 * 1.15) / np.tanh(1.15)  # glue: soft saturation and a little air
    x = x + filt(x, signal.butter(1, 2500, "high", fs=SR, output="sos")) * 0.25
    x = x * GAIN
    peak = np.max(np.abs(x))
    if peak > CEILING:
        x *= CEILING / peak
    k = ns(0.003)
    x[:k] *= (0.5 - 0.5 * np.cos(np.pi * np.arange(k) / (k - 1)))[:, None]
    x[-480:] = 0
    return x


def render():
    return finish(music(ns(DUR)))


def write(path, x):
    pcm = np.clip(np.round(x * 32767), -32768, 32767).astype("<i2")
    with wave.open(path, "wb") as w:
        w.setnchannels(2)
        w.setsampwidth(2)
        w.setframerate(SR)
        w.writeframes(pcm.tobytes())


if __name__ == "__main__":
    write(sys.argv[1], render())
