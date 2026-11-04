# The call agent's models and engine, served at /voice/ (modules/calls.nix):
#
#   whisper/{threads,single}/losos-whisper.{js,wasm}   the ears' engine
#   whisper.bin                                        the ears' model
#   kokoro.onnx, voices/<name>.bin                     the mouth
#
# Kept out of the admin UI package on purpose: a hundred and fifty megabytes
# of weights would change that package's hash and ride along in every copy of
# it. Each file is a fixed version, fetched and hashed here; nothing is
# downloaded on a box.
{
  pkgs,
  lib ? pkgs.lib,
}:
let
  whisper = import ./whisper-wasm.nix { inherit pkgs; };

  # Whisper base.en, 5-bit quantised (ggml format, MIT): 57 MB, reads English
  # about twice as fast as real time on a laptop core pair and gets the
  # product's name right with the prompt in whisper-wasm.cpp's caller.
  whisperModel = pkgs.fetchurl {
    url = "https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/ggml-base.en-q5_1.bin";
    hash = "sha256-S69w3Q18Qke6K4H6/ZwBAFrHfC+e8GTgDc8ZXQ4v3S8=";
  };

  # Kokoro-82M v1.0 (Apache-2.0) as ONNX, at a fixed revision of the
  # onnx-community export: the 8-bit model is 92 MB and runs on ONNX
  # Runtime's WebAssembly backend, which has no half-precision kernels.
  kokoroRev = "1939ad2a8e416c0acfeecc08a694d14ef25f2231";
  kokoro = file: hash: pkgs.fetchurl {
    url = "https://huggingface.co/onnx-community/Kokoro-82M-v1.0-ONNX/resolve/${kokoroRev}/${file}";
    inherit hash;
  };
  kokoroModel = kokoro "onnx/model_quantized.onnx" "sha256-+66SV+HgX/xyfpUe+bnJhBjm158cm2sTvVn1yQKKFHg=";

  # The voices on offer: American and British, women and men. The default is
  # af_heart. src/voice/mouth.ts names them.
  voices = {
    af_heart = "sha256-1YPM/zzcovf65TXLmYrAfp/LkPCXN7mkH6JzTsRKjws=";
    af_bella = "sha256-9p2DYgm3jrjGbnXjzaSR4m6oOKNnQlfp1OVwPLr1XIs=";
    am_michael = "sha256-HR8h3Y2jnDBwXNTHXQOdJl6bxKKpPtCbyeGxIl65W6E=";
    am_adam = "sha256-FisDXtkc/Ei2BGmCGExkX3LtzdG4KEM0f2Bde/exVxY=";
    bf_emma = "sha256-Zp/gZH+d0E/KuS8UOaQO60yLSrH4LkmW/j2RjOSmO3M=";
    bm_george = "sha256-xLI1pMHyzTuTn+0IuJnOk4Vji3Y/e3OllhbE/JvWybw=";
  };
in
pkgs.linkFarm "losos-voice" (
  [
    {
      name = "whisper";
      path = whisper;
    }
    {
      name = "whisper.bin";
      path = whisperModel;
    }
    {
      name = "kokoro.onnx";
      path = kokoroModel;
    }
  ]
  ++ lib.mapAttrsToList (name: hash: {
    name = "voices/${name}.bin";
    path = kokoro "voices/${name}.bin" hash;
  }) voices
)
