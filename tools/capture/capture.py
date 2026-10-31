#!/usr/bin/env python3
"""Captures the form bodies a browser sends, for tests/corpus/browsers.

The page fills one multipart form (text, names, and files, set through
DataTransfer so every browser gets the same files), one urlencoded form, and
one multipart form form-net refuses by design (a backslash in a name, D-082),
and submits each once. The server saves each body exactly as received, with
its Content-Type and the browser's User-Agent.

    python3 tools/capture/capture.py --browser "/path/to/chrome" --headless
        launches the browser on a private profile and waits for every form;
    python3 tools/capture/capture.py
        prints a URL to open by hand (Safari, or any browser), then waits.

Captures land in tests/corpus/browsers/<browser>-<major version>/.
Standard library only.
"""

import argparse
import datetime
import http.server
import os
import pathlib
import re
import shutil
import subprocess
import sys
import tempfile
import threading

ROOT = pathlib.Path(__file__).resolve().parents[2]
OUT = ROOT / "tests" / "corpus" / "browsers"

# The cases; tests/multipart_browsers.rs holds the expected parse of each.
PAGE = r"""<!doctype html>
<html><head><meta charset="utf-8"><title>form-net capture</title></head>
<body>
<form id="mp" method="post" action="/capture/multipart" enctype="multipart/form-data" accept-charset="UTF-8">
  <input name="ascii" value="hello world">
  <input name="unicode" value="h&eacute;llo w&ouml;rld &#x2713; &#x65E5;&#x672C;">
  <textarea name="newline"></textarea>
  <input name="quotes" value="say &quot;hi&quot; and 'bye'">
  <input name='na"me' value="quote in the name">
  <input name="per%cent" value="percent in the name">
  <input name="&uuml;n&iuml;" value="non-ASCII name">
  <input type="hidden" name="_charset_">
  <input type="file" name="empty">
  <input type="file" name="ascii_file">
  <input type="file" name="unicode_file">
  <input type="file" name="quote_file">
  <input type="file" name="percent_file">
  <input type="file" name="backslash_file">
  <input type="file" name="newline_file">
  <input type="file" name="multi" multiple>
  <button id="go-mp" type="button">Fill and send the multipart form</button>
</form>
<form id="ue" method="post" action="/capture/urlencoded" accept-charset="UTF-8">
  <input name="ascii" value="hello world">
  <input name="unicode" value="h&eacute;llo w&ouml;rld &#x2713; &#x65E5;&#x672C;">
  <textarea name="newline"></textarea>
  <input name="symbols" value="a+b=c&amp;d%e;f/g?h#i">
  <input name="na&quot;me" value="quote in the name">
  <input type="hidden" name="_charset_">
  <button id="go-ue" type="button">Send the urlencoded form</button>
</form>
<form id="rf" method="post" action="/capture/refused" enctype="multipart/form-data" accept-charset="UTF-8">
  <input name="back\slash" value="backslash in the name, which form-net refuses (D-082)">
  <button id="go-rf" type="button">Send the refused form</button>
</form>
<script>
function put(form, name, files) {
  const dt = new DataTransfer();
  for (const [fname, type, bytes] of files) dt.items.add(new File([bytes], fname, {type}));
  form.elements[name].files = dt.files;
}
function fill(form) {
  form.elements["newline"].value = "line1\nline2\r\nline3\rline4";
}
function sendMultipart() {
  const f = document.getElementById("mp");
  fill(f);
  const all = new Uint8Array(256);
  for (let i = 0; i < 256; i++) all[i] = i;
  put(f, "ascii_file", [["hello.txt", "text/plain", "plain text\n"]]);
  put(f, "unicode_file", [["résumé ✓.txt", "text/plain", "unicode name\n"]]);
  put(f, "quote_file", [["na\"me.txt", "text/plain", "quote\n"]]);
  put(f, "percent_file", [["100%.txt", "text/plain", "percent\n"]]);
  put(f, "backslash_file", [["back\\slash.txt", "text/plain", "backslash\n"]]);
  put(f, "newline_file", [["line\nbreak.txt", "text/plain", "newline\n"]]);
  put(f, "multi", [["a.txt", "text/plain", "first\r\n--not a boundary\r\n"], ["b.bin", "application/octet-stream", all]]);
  f.submit();
}
function sendUrlencoded() {
  const f = document.getElementById("ue");
  fill(f);
  f.submit();
}
document.getElementById("go-mp").onclick = sendMultipart;
document.getElementById("go-ue").onclick = sendUrlencoded;
document.getElementById("go-rf").onclick = () => document.getElementById("rf").submit();
const auto = new URLSearchParams(location.search).get("auto");
if (auto === "multipart") sendMultipart();
if (auto === "urlencoded") sendUrlencoded();
if (auto === "refused") document.getElementById("rf").submit();
</script>
</body></html>
"""


# Sent in this order; each answer redirects to the next form.
KINDS = ["multipart", "urlencoded", "refused"]


class Server(http.server.ThreadingHTTPServer):
    def handle_error(self, request, client_address):
        # A browser closed when the capture ends drops its connection.
        if not isinstance(sys.exc_info()[1], ConnectionError):
            super().handle_error(request, client_address)


def label(agent):
    for pattern, name in [
        (r"Edg/(\d+)", "edge"),
        (r"Firefox/(\d+)", "firefox"),
        (r"Chrome/(\d+)", "chrome"),
        (r"Version/(\d+)[.\d]* .*Safari/", "safari"),
    ]:
        m = re.search(pattern, agent)
        if m:
            return f"{name}-{m.group(1)}"
    return "unknown"


class Handler(http.server.BaseHTTPRequestHandler):
    done = threading.Event()
    seen = set()
    out_dir = None
    expect = set(KINDS)

    def log_message(self, *args):
        pass

    def do_GET(self):
        body = PAGE.encode("utf-8")
        self.send_response(200)
        self.send_header("Content-Type", "text/html; charset=utf-8")
        self.send_header("Content-Length", str(len(body)))
        self.end_headers()
        self.wfile.write(body)

    def do_POST(self):
        kind = self.path.rsplit("/", 1)[-1]
        if kind not in KINDS:
            self.send_error(404)
            return
        length = int(self.headers.get("Content-Length", "0"))
        data = self.rfile.read(length)
        agent = self.headers.get("User-Agent", "")
        out = Handler.out_dir or OUT / label(agent)
        out.mkdir(parents=True, exist_ok=True)
        (out / f"{kind}.body").write_bytes(data)
        (out / f"{kind}.content-type").write_text(self.headers.get("Content-Type", "") + "\n", encoding="utf-8")
        (out / "user-agent").write_text(agent + "\n", encoding="utf-8")
        (out / "captured").write_text(datetime.date.today().isoformat() + "\n", encoding="utf-8")
        print(f"saved {kind} ({len(data)} bytes) to {out}", flush=True)
        Handler.seen.add(kind)
        if kind != KINDS[-1]:
            self.send_response(303)
            self.send_header("Location", "/?auto=" + KINDS[KINDS.index(kind) + 1])
            self.send_header("Content-Length", "0")
            self.end_headers()
        else:
            reply = b"captured; you can close this page\n"
            self.send_response(200)
            self.send_header("Content-Type", "text/plain")
            self.send_header("Content-Length", str(len(reply)))
            self.end_headers()
            self.wfile.write(reply)
        if Handler.seen >= Handler.expect:
            Handler.done.set()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--browser", help="browser executable to launch")
    ap.add_argument("--headless", action="store_true")
    ap.add_argument("--port", type=int, default=0)
    ap.add_argument("--out", help="directory for the captures (default: by User-Agent)")
    ap.add_argument("--expect", default=",".join(KINDS), help="forms to wait for, comma separated (default: all)")
    args = ap.parse_args()
    if args.out:
        Handler.out_dir = pathlib.Path(args.out)
    Handler.expect = set(args.expect.split(","))
    server = Server(("127.0.0.1", args.port), Handler)
    port = server.server_address[1]
    threading.Thread(target=server.serve_forever, daemon=True).start()
    url = f"http://127.0.0.1:{port}/?auto=multipart"
    print(f"listening on port {port}", flush=True)
    proc = None
    profile = None
    if args.browser:
        profile = tempfile.mkdtemp(prefix="form-net-capture-")
        cmd = [args.browser]
        if "firefox" in os.path.basename(args.browser).lower():
            cmd += ["-profile", profile, "-no-remote"] + (["-headless"] if args.headless else [])
        else:
            cmd += [f"--user-data-dir={profile}", "--no-first-run", "--no-default-browser-check"]
            cmd += ["--headless=new", "--disable-gpu"] if args.headless else []
        proc = subprocess.Popen(cmd + [url], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    else:
        print(f"open {url}", flush=True)
    ok = Handler.done.wait(timeout=60 if proc else 600)
    if proc:
        proc.terminate()
        try:
            proc.wait(timeout=10)
        except subprocess.TimeoutExpired:
            proc.kill()
    server.shutdown()
    if profile:
        shutil.rmtree(profile, ignore_errors=True)
    if not ok:
        print("timed out before every form arrived", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
