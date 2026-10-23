"""Serve the self-contained report on localhost (WebGPU secure context)."""
from http.server import ThreadingHTTPServer, SimpleHTTPRequestHandler
from functools import partial
from pathlib import Path
import argparse

parser = argparse.ArgumentParser()
parser.add_argument("--port", type=int, default=0)
args = parser.parse_args()
root = Path(__file__).resolve().parent
server = ThreadingHTTPServer(("127.0.0.1", args.port), partial(SimpleHTTPRequestHandler, directory=str(root)))
print(f"http://localhost:{server.server_port}/{root.name}.html", flush=True)
try:
    server.serve_forever()
except KeyboardInterrupt:
    server.server_close()
