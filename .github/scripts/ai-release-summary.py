#!/usr/bin/env python3
"""
Calls GitHub Models (gpt-4o-mini) to write a prose summary paragraph for
a release, given a structured cliff changelog on stdin.

Prints the summary to stdout, or exits 1 silently if the API call fails
(the caller falls back to cliff notes alone).

Usage:
    python3 ai-release-summary.py < /tmp/cliff-notes.md
"""
import json
import os
import sys
import urllib.request
import urllib.error

cliff_notes = sys.stdin.read().strip()
if not cliff_notes:
    sys.exit(1)

token = os.environ.get("GH_TOKEN", "")
if not token:
    sys.exit(1)

prompt = (
    "You are writing release notes for Nexus Open, a Go+Flutter Linux daemon "
    "that drives the Corsair iCUE Nexus 640×48 hardware display strip.\n\n"
    "Here is the structured changelog for this release:\n\n"
    f"{cliff_notes}\n\n"
    "Write a 2-3 sentence plain-English summary suitable for the top of a "
    "GitHub release. Focus on what changed from a user or developer perspective. "
    "Be specific, not generic. Do not use phrases like \"exciting\" or "
    "\"we are pleased\". Do not repeat the version number. Do not include any "
    "markdown heading. Output only the summary paragraph, nothing else."
)

payload = json.dumps({
    "model": "gpt-4o-mini",
    "messages": [{"role": "user", "content": prompt}],
    "max_tokens": 200,
    "temperature": 0.3,
}).encode()

req = urllib.request.Request(
    "https://models.inference.ai.azure.com/chat/completions",
    data=payload,
    headers={
        "Content-Type": "application/json",
        "Authorization": f"Bearer {token}",
    },
)

try:
    with urllib.request.urlopen(req, timeout=30) as resp:
        body = json.load(resp)
    print(body["choices"][0]["message"]["content"].strip())
except Exception:
    sys.exit(1)
