// SPDX-FileCopyrightText: 2026 Matus Mastena
// SPDX-License-Identifier: AGPL-3.0-or-later
//
// The page's whole view of whisper.cpp: load a model from the module's
// in-memory file system, hand over 16 kHz mono floats, read the segments
// back. Plain C functions and ccall, no Embind: the audio crosses once, as
// a copy into a buffer this side owns.

#include "whisper.h"

#include <emscripten.h>

#include <algorithm>
#include <cstdint>
#include <string>
#include <vector>

namespace {
whisper_context *g_ctx = nullptr;
std::vector<float> g_pcm;
std::string g_prompt;
std::string g_lang = "en";
std::string g_text;
}  // namespace

extern "C" {

// Load the model at `path` (written to the file system by the page).
EMSCRIPTEN_KEEPALIVE int losos_whisper_init(const char *path) {
    if (g_ctx != nullptr) return 1;
    whisper_context_params cp = whisper_context_default_params();
    cp.use_gpu = false;
    g_ctx = whisper_init_from_file_with_params(path, cp);
    return g_ctx != nullptr ? 1 : 0;
}

EMSCRIPTEN_KEEPALIVE int losos_whisper_multilingual() {
    return g_ctx != nullptr && whisper_is_multilingual(g_ctx) ? 1 : 0;
}

// Words the model should expect (names, product words), as a prompt.
EMSCRIPTEN_KEEPALIVE void losos_whisper_prompt(const char *text) { g_prompt = text; }

EMSCRIPTEN_KEEPALIVE void losos_whisper_language(const char *code) { g_lang = code; }

// A buffer for `n` samples that the page fills in place.
EMSCRIPTEN_KEEPALIVE float *losos_whisper_input(int n) {
    g_pcm.assign(static_cast<size_t>(std::max(n, 0)), 0.0f);
    return g_pcm.data();
}

// Transcribe what the page put in the buffer. Returns the text length in
// bytes, or -1 when nothing could run. Greedy decoding, no context between
// calls: each utterance stands alone.
EMSCRIPTEN_KEEPALIVE int losos_whisper_run(int threads) {
    if (g_ctx == nullptr || g_pcm.empty()) return -1;
    whisper_full_params p = whisper_full_default_params(WHISPER_SAMPLING_GREEDY);
    p.print_realtime = false;
    p.print_progress = false;
    p.print_timestamps = false;
    p.print_special = false;
    p.translate = false;
    p.no_context = true;
    p.single_segment = false;
    p.suppress_blank = true;
    p.suppress_nst = true;
    p.no_speech_thold = 0.6f;
    p.temperature = 0.0f;
    p.n_threads = std::max(1, threads);
    // The encoder reads a 30 s window however short the speech is; ask it
    // for only the frames the utterance fills (50 a second) and a margin.
    // Utterances are seconds long, so this is most of the speed.
    p.audio_ctx = std::min(1500, std::max(256, static_cast<int>(g_pcm.size() / 320) + 128));
    p.language = whisper_is_multilingual(g_ctx) ? g_lang.c_str() : "en";
    if (!g_prompt.empty()) p.initial_prompt = g_prompt.c_str();
    if (whisper_full(g_ctx, p, g_pcm.data(), static_cast<int>(g_pcm.size())) != 0) return -1;
    g_text.clear();
    const int n = whisper_full_n_segments(g_ctx);
    for (int i = 0; i < n; i++) {
        const char *t = whisper_full_get_segment_text(g_ctx, i);
        if (t != nullptr) g_text += t;
    }
    return static_cast<int>(g_text.size());
}

EMSCRIPTEN_KEEPALIVE const char *losos_whisper_text() { return g_text.c_str(); }

EMSCRIPTEN_KEEPALIVE void losos_whisper_free() {
    if (g_ctx != nullptr) whisper_free(g_ctx);
    g_ctx = nullptr;
    g_pcm.clear();
    g_pcm.shrink_to_fit();
}

}  // extern "C"
