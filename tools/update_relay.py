"""Serves only the published desktop client releases (dist/client, at /client/) on port 8080, for
clients built for this PC's address after the game moved to the VPS: they find the newest
release here, which connects to the VPS, and switch to it by themselves (on the sign-in screen,
the character list or at their next portal). Nothing else is served.

    python tools/update_relay.py [--port 8080]

Stop it once nobody plays an old client any more. Don't run it beside this PC's own account
server, which takes the same port.
"""
import argparse
import functools
import http.server
import os

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
DIST = os.path.join(ROOT, "dist")


class Handler(http.server.SimpleHTTPRequestHandler):
    def send_head(self):
        path = self.path.split("?", 1)[0]
        if not path.startswith("/client/") or path.endswith("/") or ".." in path:
            self.send_error(404)
            return None
        return super().send_head()

    def log_message(self, fmt, *args):
        pass


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--port", type=int, default=8080)
    args = ap.parse_args()
    handler = functools.partial(Handler, directory=DIST)
    with http.server.ThreadingHTTPServer(("0.0.0.0", args.port), handler) as server:
        print("serving %s/client on port %d" % (DIST, args.port), flush=True)
        server.serve_forever()


if __name__ == "__main__":
    main()
