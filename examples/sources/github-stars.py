#!/usr/bin/env python3
"""An external source for Nexus Open: a GitHub repository's stars and open
issues, from GitHub's public API. Standard library only.

Copy this file next to nexus.toml and add:

    [[source]]
    name = "github"
    run = ["python3", "github-stars.py", "owner/repo"]

It publishes github.stars and github.issues every 15 minutes (GitHub allows
60 unauthenticated requests an hour). Failures go to stderr, which the
daemon logs, and it tries again next time.
"""

import json
import sys
import time
import urllib.error
import urllib.request

REPO = sys.argv[1] if len(sys.argv) > 1 else "mantonx/nexus-open"
EVERY = 15 * 60


def publish(signals):
    # flush: a pipe is block-buffered, so without it nothing would arrive
    # until about 4 KB had been printed.
    print(json.dumps(signals), flush=True)


while True:
    request = urllib.request.Request(
        f"https://api.github.com/repos/{REPO}",
        headers={"Accept": "application/vnd.github+json", "User-Agent": "nexus-open-source"},
    )
    try:
        with urllib.request.urlopen(request, timeout=20) as response:
            repo = json.load(response)
        publish({"github.stars": repo["stargazers_count"], "github.issues": repo["open_issues_count"]})
    except (urllib.error.URLError, KeyError, ValueError) as e:
        print(f"{REPO}: {e}", file=sys.stderr, flush=True)
    time.sleep(EVERY)
