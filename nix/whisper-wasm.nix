# whisper.cpp for the browser: ggml's CPU backend compiled to WebAssembly
# (SIMD, and threads where the page is cross-origin isolated) with the small
# C interface in admin-ui/voice/whisper/whisper-wasm.cpp. The source is
# nixpkgs' own whisper.cpp release, so the engine is the one a box's other
# whisper users build, and nothing is fetched at build time.
#
# Two builds come out, `threads/` and `single/`: a page that is not
# cross-origin isolated has no SharedArrayBuffer and takes the second.
{
  pkgs,
  lib ? pkgs.lib,
}:
let
  build =
    threads:
    pkgs.stdenv.mkDerivation {
      pname = "losos-whisper-wasm${lib.optionalString (!threads) "-single"}";
      version = pkgs.whisper-cpp.version;

      src = lib.fileset.toSource {
        root = ./../admin-ui/voice/whisper;
        fileset = lib.fileset.unions [
          ./../admin-ui/voice/whisper/CMakeLists.txt
          ./../admin-ui/voice/whisper/whisper-wasm.cpp
        ];
      };

      nativeBuildInputs = [
        pkgs.emscripten
        pkgs.cmake
        pkgs.ninja
        pkgs.python3
      ];
      dontConfigure = true;
      dontFixup = true;

      buildPhase = ''
        runHook preBuild
        # Emscripten keeps its compiled system libraries in a cache it must be
        # able to write.
        export HOME=$TMPDIR
        export EM_CACHE=$TMPDIR/emscripten-cache
        mkdir -p build
        emcmake cmake -S . -B build -G Ninja \
          -DCMAKE_BUILD_TYPE=Release \
          -DWHISPER_SRC=${pkgs.whisper-cpp.src} \
          -DLOSOS_THREADS=${if threads then "ON" else "OFF"} \
          -DCMAKE_C_FLAGS="-O3 -msimd128 ${lib.optionalString threads "-pthread"}" \
          -DCMAKE_CXX_FLAGS="-O3 -msimd128 ${lib.optionalString threads "-pthread"}"
        cmake --build build --target losos-whisper
        runHook postBuild
      '';

      installPhase = ''
        runHook preInstall
        mkdir -p $out
        cp build/losos-whisper.js build/losos-whisper.wasm $out/
        runHook postInstall
      '';

      meta = {
        description = "whisper.cpp as WebAssembly for the call agent's ears";
        license = lib.licenses.mit;
      };
    };
in
pkgs.linkFarm "losos-whisper-wasm" [
  {
    name = "threads";
    path = build true;
  }
  {
    name = "single";
    path = build false;
  }
]
