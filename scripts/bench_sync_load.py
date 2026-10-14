#!/usr/bin/env python3
"""Distinct-instance registration load generator (cluster sync profiling).

Why this exists
---------------
`scripts/benchmark.sh naming` registers the *same* instance over and over
(`ip=10.0.0.1&port=8080`). That never changes data, so distro never creates a
sync task and peer nodes stay completely idle — cluster sync cost becomes
invisible. This driver registers a DIFFERENT instance on every request so real
distro sync traffic is generated and can be profiled on the peer nodes.

Usage
-----
    TOKEN=$(curl -s -X POST http://127.0.0.1:8848/nacos/v3/auth/user/login \
              -d 'username=nacos&password=nacos' | python3 -c \
              'import sys,json;print(json.load(sys.stdin)["accessToken"])')
    python3 scripts/bench_sync_load.py "$TOKEN" 45 24 http://127.0.0.1:8848

    # while it runs, profile a peer node:
    #   sample <peer-pid> 30 -f /tmp/peer_sync.txt -mayDie

Arguments
---------
    token     accessToken from the login endpoint
    duration  seconds to run (default 40)
    threads   concurrent writers (default 24)
    host      base URL of the target node (default http://127.0.0.1:8848)

Prints elapsed time, total requests, achieved RPS and the HTTP status code
distribution (watch for non-200: it means the node is rejecting writes).
"""

import sys
import time
import threading
import collections
import urllib.request
import urllib.error

TOKEN = sys.argv[1] if len(sys.argv) > 1 else ""
DURATION = float(sys.argv[2]) if len(sys.argv) > 2 else 40.0
THREADS = int(sys.argv[3]) if len(sys.argv) > 3 else 24
HOST = sys.argv[4] if len(sys.argv) > 4 else "http://127.0.0.1:8848"

if not TOKEN:
    sys.exit("usage: bench_sync_load.py <token> [duration] [threads] [host]")

URL = f"{HOST}/nacos/v2/ns/instance?accessToken={TOKEN}"
codes = collections.Counter()
count = [0]
lock = threading.Lock()
stop = threading.Event()


def worker(tid):
    """POST a unique instance per iteration until stopped."""
    while not stop.is_set():
        with lock:
            count[0] += 1
            i = count[0]
        body = (
            f"serviceName=bench-sync&ip=10.{tid + 1}.{(i // 250) % 250 + 1}.{i % 250 + 1}"
            f"&port={8000 + i % 2000}&ephemeral=true&weight=1.0"
        ).encode()
        req = urllib.request.Request(
            URL,
            data=body,
            headers={"Content-Type": "application/x-www-form-urlencoded"},
            method="POST",
        )
        try:
            with urllib.request.urlopen(req, timeout=10) as r:
                code = r.status
        except urllib.error.HTTPError as e:
            code = e.code
        except Exception:
            code = -1
        with lock:
            codes[code] += 1


start = time.time()
threads = [threading.Thread(target=worker, args=(t,), daemon=True) for t in range(THREADS)]
for t in threads:
    t.start()
time.sleep(DURATION)
stop.set()
for t in threads:
    t.join(timeout=5)
elapsed = time.time() - start
total = sum(codes.values())
print(f"elapsed={elapsed:.1f}s total={total} rps={total / elapsed:.1f}")
print("status codes:", dict(codes))
