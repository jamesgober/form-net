#!/usr/bin/env python3
"""Opens a URL in a browser through a WebDriver server, for capture.py.

capture.py serves the capture page; this opens it in a browser that only a
WebDriver server can drive unattended, such as Safari through safaridriver
(the macOS job in .github/workflows/capture.yml). The page sends its forms
by itself; this keeps the session open until capture.py has them.

    python3 tools/capture/webdriver.py --driver http://127.0.0.1:4444 \\
        --browser safari --url http://127.0.0.1:8765/?auto=multipart

Standard library only.
"""

import argparse
import json
import sys
import time
import urllib.request


def call(method, url, body=None):
    data = None if body is None else json.dumps(body).encode("utf-8")
    req = urllib.request.Request(url, data=data, method=method)
    req.add_header("Content-Type", "application/json")
    with urllib.request.urlopen(req, timeout=60) as resp:
        return json.loads(resp.read() or b"{}")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--driver", required=True, help="WebDriver server URL")
    ap.add_argument("--browser", required=True, help="browserName capability")
    ap.add_argument("--url", required=True, help="page to open")
    ap.add_argument("--hold", type=int, default=30, help="seconds to keep the session open")
    args = ap.parse_args()
    driver = args.driver.rstrip("/")
    # The driver may still be starting.
    for attempt in range(30):
        try:
            call("GET", driver + "/status")
            break
        except OSError:
            time.sleep(1)
    caps = {"capabilities": {"alwaysMatch": {"browserName": args.browser}}}
    session = call("POST", driver + "/session", caps)["value"]["sessionId"]
    try:
        call("POST", f"{driver}/session/{session}/url", {"url": args.url})
        time.sleep(args.hold)
    finally:
        call("DELETE", f"{driver}/session/{session}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
