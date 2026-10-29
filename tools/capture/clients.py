#!/usr/bin/env python3
"""Captures the multipart bodies HTTP clients send, for tests/corpus/clients.

    python3 tools/capture/clients.py curl       (curl -F)
    python3 tools/capture/clients.py requests   (Python requests)
    python3 tools/capture/clients.py go         (Go mime/multipart)

Each client sends two bodies to a local capture server: `multipart` (text,
non-ASCII, CR LF, plain and non-ASCII file names, two files under one name)
and `quote` (a file name with a `"`, which clients escape differently).
Captures land in tests/corpus/clients/<client>-<version>/.
"""

import os
import pathlib
import re
import subprocess
import sys
import tempfile
import threading

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))
import capture  # noqa: E402

HERE = pathlib.Path(__file__).resolve().parent
OUT = capture.ROOT / "tests" / "corpus" / "clients"

TEXT = [("ascii", "hello world"), ("unicode", "héllo wörld ✓ 日本"), ("newline", "line1\r\nline2")]
FILES = [
    ("ascii_file", "hello.txt", "text/plain", b"plain text\n"),
    ("unicode_file", "résumé ✓.txt", "text/plain", b"unicode name\n"),
    ("multi", "a.txt", "text/plain", b"first\r\n--not a boundary\r\n"),
    ("multi", "b.bin", "application/octet-stream", bytes(range(256))),
]
QUOTE = ("quote_file", 'na"me.txt', "text/plain", b"quote\n")


def version(client):
    if client == "curl":
        out = subprocess.run(["curl", "--version"], capture_output=True, text=True).stdout
        return re.search(r"curl (\d+\.\d+)", out).group(1)
    if client == "requests":
        import requests
        return ".".join(requests.__version__.split(".")[:2])
    out = subprocess.run(["go", "version"], capture_output=True, text=True).stdout
    return re.search(r"go(\d+\.\d+)", out).group(1)


def send_curl(base, tmp):
    def files_args(files):
        args = []
        for i, (name, fname, ctype, data) in enumerate(files):
            path = pathlib.Path(tmp) / f"f{i}"
            path.write_bytes(data)
            # -F quotes the file name; a `"` or `\` inside it is escaped.
            quoted = fname.replace("\\", "\\\\").replace('"', '\\"')
            args += ["-F", f'{name}=@{path};filename="{quoted}";type={ctype}']
        return args
    text = []
    for name, value in TEXT:
        text += ["--form-string", f"{name}={value}"]
    subprocess.run(["curl", "-s", "-o", os.devnull] + text + files_args(FILES) + [f"{base}/capture/multipart"], check=True)
    subprocess.run(["curl", "-s", "-o", os.devnull] + files_args([QUOTE]) + [f"{base}/capture/quote"], check=True)


def send_requests(base, _tmp):
    import requests
    files = [(name, (fname, data, ctype)) for name, fname, ctype, data in FILES]
    requests.post(f"{base}/capture/multipart", data=TEXT, files=files, allow_redirects=False)
    name, fname, ctype, data = QUOTE
    requests.post(f"{base}/capture/quote", files=[(name, (fname, data, ctype))], allow_redirects=False)


def send_go(base, _tmp):
    subprocess.run(["go", "run", str(HERE / "go_client.go"), base], check=True)


def main():
    client = sys.argv[1]
    send = {"curl": send_curl, "requests": send_requests, "go": send_go}[client]
    capture.Handler.out_dir = OUT / f"{client}-{version(client)}"
    capture.Handler.expect = {"multipart", "quote"}
    capture.KINDS.append("quote")
    server = capture.Server(("127.0.0.1", 0), capture.Handler)
    threading.Thread(target=server.serve_forever, daemon=True).start()
    base = f"http://127.0.0.1:{server.server_address[1]}"
    with tempfile.TemporaryDirectory(prefix="form-net-clients-") as tmp:
        send(base, tmp)
    ok = capture.Handler.done.wait(timeout=30)
    server.shutdown()
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
