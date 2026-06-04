#!/usr/bin/env python3
"""
Take a screenshot of the running Flutter app via the Dart VM Service.
Usage: python3 scripts/flutter-screenshot.py [output.png]

Reads the VM service URL from /tmp/flutter-run.log (written by:
  flutter run -d linux 2>&1 | tee /tmp/flutter-run.log
)
"""

import asyncio, base64, json, re, sys
from pathlib import Path

LOG_FILES = [
    "/tmp/flutter-run.log",   # from: flutter run ... | tee /tmp/flutter-run.log
    "/tmp/ui-tour.log",       # from: ui-tour.sh (flutter drive)
]
DEFAULT_OUT = "/tmp/nexus-ui.png"

def find_ws_url(log_paths: list) -> str:
    for log_path in log_paths:
        try:
            text = Path(log_path).read_text()
        except FileNotFoundError:
            continue
        # flutter drive logs the HTTP URL in VMServiceFlutterDriver lines
        match = re.search(r"Connecting to Flutter application at (http://[\d.:]+/\S+/=)", text)
        if match:
            http_url = match.group(1)
            return http_url.replace("http://", "ws://").rstrip("/") + "/ws"
        # flutter run logs a ws:// URL directly
        match = re.search(r"ws://[\d.:]+/\S+/ws", text)
        if match:
            return match.group(0)
        # fall back: http URL at end of line
        match = re.search(r"http://[\d.:]+/\S+=/$", text, re.MULTILINE)
        if match:
            return match.group(0).replace("http://", "ws://").rstrip("/") + "/ws"
    raise RuntimeError(f"No VM service URL found in: {log_paths}")

async def screenshot(ws_url: str, out_path: str):
    import websockets
    async with websockets.connect(ws_url) as ws:
        # Get isolate ID
        await ws.send(json.dumps({"jsonrpc": "2.0", "id": "1", "method": "getVM", "params": {}}))
        resp = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
        isolate_id = resp["result"]["isolates"][0]["id"]

        # Get root widget object ID
        await ws.send(json.dumps({
            "jsonrpc": "2.0", "id": "2",
            "method": "ext.flutter.inspector.getRootWidget",
            "params": {"isolateId": isolate_id, "objectGroup": "screenshot"},
        }))
        resp = json.loads(await asyncio.wait_for(ws.recv(), timeout=5))
        widget_id = resp["result"]["result"]["valueId"]

        # Take inspector screenshot of root widget
        await ws.send(json.dumps({
            "jsonrpc": "2.0", "id": "3",
            "method": "ext.flutter.inspector.screenshot",
            "params": {
                "isolateId": isolate_id,
                "id": widget_id,
                "objectGroup": "screenshot",
                "width": 800.0,
                "height": 600.0,
                "margin": 0.0,
                "maxPixelRatio": 2.0,
            },
        }))
        resp = json.loads(await asyncio.wait_for(ws.recv(), timeout=10))

        if "error" in resp:
            raise RuntimeError(f"VM error: {resp['error']}")

        # Response shape: {"result": {"result": "<base64png>", "type": ..., "method": ...}}
        png_b64 = resp["result"].get("result", "")
        if not png_b64 or not isinstance(png_b64, str):
            raise RuntimeError(f"No screenshot in response: {resp['result']}")

        Path(out_path).write_bytes(base64.b64decode(png_b64))
        print(out_path)

def main():
    out    = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_OUT
    ws_url = sys.argv[2] if len(sys.argv) > 2 else find_ws_url(LOG_FILES)
    asyncio.run(screenshot(ws_url, out))

if __name__ == "__main__":
    main()
