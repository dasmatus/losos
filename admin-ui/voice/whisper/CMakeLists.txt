# SPDX-FileCopyrightText: 2026 Matus Mastena
# SPDX-License-Identifier: AGPL-3.0-or-later
#
# whisper.cpp built for the browser (nix/whisper-wasm.nix): ggml's CPU
# backend with WebAssembly SIMD, and the small C interface in
# whisper-wasm.cpp. WHISPER_SRC is the unpacked whisper.cpp release.
cmake_minimum_required(VERSION 3.20)
project(losos-whisper-wasm C CXX)

set(WHISPER_SRC "" CACHE PATH "whisper.cpp source tree")
option(LOSOS_THREADS "build for SharedArrayBuffer threads" ON)

set(BUILD_SHARED_LIBS OFF)
set(WHISPER_BUILD_EXAMPLES OFF)
set(WHISPER_BUILD_TESTS OFF)
set(WHISPER_BUILD_SERVER OFF)
set(GGML_NATIVE OFF)
set(GGML_OPENMP OFF)
set(GGML_CCACHE OFF)
set(GGML_LLAMAFILE OFF)
add_subdirectory(${WHISPER_SRC} whisper EXCLUDE_FROM_ALL)

add_executable(losos-whisper whisper-wasm.cpp)
target_link_libraries(losos-whisper PRIVATE whisper)

set(LINK
  -sMODULARIZE=1 -sEXPORT_ES6=1 -sEXPORT_NAME=whisperFactory
  -sENVIRONMENT=web,worker
  -sALLOW_MEMORY_GROWTH=1 -sMAXIMUM_MEMORY=2gb -sINITIAL_MEMORY=256mb
  -sFORCE_FILESYSTEM=1 -sFILESYSTEM=1
  -sEXPORTED_FUNCTIONS=_malloc,_free
  -sEXPORTED_RUNTIME_METHODS=FS,HEAPF32,HEAPU8,UTF8ToString,ccall
  --no-entry -O3)
if(LOSOS_THREADS)
  list(APPEND LINK -flto=thin -pthread -sPTHREAD_POOL_SIZE=4)
endif()
target_link_options(losos-whisper PRIVATE ${LINK})
