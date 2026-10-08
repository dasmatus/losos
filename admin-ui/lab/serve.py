#!/usr/bin/env python3
"""Serve LosOS Lab with the two headers qemu-wasm needs.

qemu-wasm runs QEMU's threads as Web Workers sharing one SharedArrayBuffer,
and browsers only hand those to a cross-origin isolated page. This server
adds Cross-Origin-Opener-Policy and Cross-Origin-Embedder-Policy to every
response. Run it from this folder and open http://localhost:8080/.
"""
import http.server, os, sys
class H(http.server.SimpleHTTPRequestHandler):
    extensions_map = {**http.server.SimpleHTTPRequestHandler.extensions_map, '.wasm': 'application/wasm', '.js': 'text/javascript'}
    def end_headers(self):
        self.send_header('Cross-Origin-Opener-Policy', 'same-origin')
        self.send_header('Cross-Origin-Embedder-Policy', 'require-corp')
        self.send_header('Cross-Origin-Resource-Policy', 'same-origin')
        self.send_header('Cache-Control', 'no-cache')
        super().end_headers()
os.chdir(os.path.dirname(os.path.abspath(__file__)))
port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
print(f'LosOS Lab on http://localhost:{port}/')
http.server.ThreadingHTTPServer(('127.0.0.1', port), H).serve_forever()
