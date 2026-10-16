"""A stand-in for the VPS's Caddy, for testing the desktop client over https and wss on this PC.

    python tools/testclient/tls_front.py --listen 8443 --to 8098 [--dir build/iso1/tls]
                                         [--forwarded-for 203.0.113.7]

It takes TLS connections on 127.0.0.1:<listen> and passes the bytes on to the gateway on
127.0.0.1:<to> (`bsk-gateway`, which serves /api and /ws as behind Caddy). The certificate is for
127.0.0.1, signed by a test CA it makes on first use with openssl (Git for Windows has it): the
client trusts it when started with BSK_CA=<dir>/ca.pem.

    set BSK_CA=build\\iso1\\tls\\ca.pem
    betterskillys --account https://127.0.0.1:8443/api --credentials test-account.local.txt

--forwarded-for adds an X-Forwarded-For header to the first request of each connection, as Caddy
adds the player's address, to check that it reaches the account and world servers. Test use only:
it's not a web server, and nothing here is meant for players.
"""
import argparse
import asyncio
import os
import shutil
import ssl
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))


def openssl():
    found = shutil.which("openssl")
    if not found and os.name == "nt":
        for base in (os.environ.get("ProgramFiles", r"C:\Program Files"), os.environ.get("ProgramFiles(x86)", "")):
            candidate = os.path.join(base, "Git", "usr", "bin", "openssl.exe")
            if base and os.path.isfile(candidate):
                found = candidate
                break
    if not found:
        sys.exit("openssl not found: install Git for Windows, or put openssl on PATH")
    return found


def make_certs(folder):
    """A test CA and a certificate for 127.0.0.1 and localhost signed by it, made once."""
    leaf, key, ca = (os.path.join(folder, n) for n in ("leaf.pem", "leaf.key", "ca.pem"))
    if all(os.path.isfile(p) for p in (leaf, key, ca)):
        return leaf, key, ca
    os.makedirs(folder, exist_ok=True)
    tool = openssl()
    ca_key, csr, ext, ca_ext = (os.path.join(folder, n) for n in ("ca.key", "leaf.csr", "leaf.ext", "ca.cnf"))
    with open(ext, "w") as f:
        f.write("basicConstraints=CA:FALSE\nkeyUsage=digitalSignature,keyEncipherment\n"
                "extendedKeyUsage=serverAuth\nsubjectAltName=IP:127.0.0.1,DNS:localhost\n")
    env = dict(os.environ, MSYS_NO_PATHCONV="1")  # keeps Git's openssl from rewriting -subj
    run = lambda *a: subprocess.run([tool, *a], check=True, env=env, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    with open(ca_ext, "w") as f:
        f.write("[req]\ndistinguished_name=dn\nprompt=no\n[dn]\nCN=betterSkillys test CA\n"
                "[ca]\nbasicConstraints=critical,CA:TRUE\nkeyUsage=critical,keyCertSign,cRLSign\n"
                "subjectKeyIdentifier=hash\n")
    # its own extension section, so openssl.cnf's defaults don't repeat an extension (which
    # rustls rejects)
    run("req", "-x509", "-newkey", "rsa:2048", "-nodes", "-keyout", ca_key, "-out", ca, "-days", "30",
        "-subj", "/CN=betterSkillys test CA", "-config", ca_ext, "-extensions", "ca")
    run("req", "-newkey", "rsa:2048", "-nodes", "-keyout", key, "-out", csr, "-subj", "/CN=127.0.0.1")
    run("x509", "-req", "-in", csr, "-CA", ca, "-CAkey", ca_key, "-CAcreateserial", "-out", leaf,
        "-days", "30", "-extfile", ext)
    print("made a test CA: %s" % ca)
    return leaf, key, ca


async def pipe(reader, writer, first=b""):
    try:
        if first:
            writer.write(first)
        while True:
            data = await reader.read(65536)
            if not data:
                break
            writer.write(data)
            await writer.drain()
    except (OSError, asyncio.CancelledError):
        pass
    finally:
        try:
            writer.close()
        except OSError:
            pass


async def first_request(reader, forwarded_for):
    """The first request's head with X-Forwarded-For added (and any the client sent removed)."""
    head = await reader.readuntil(b"\r\n\r\n")
    lines = [l for l in head[:-4].split(b"\r\n") if not l.lower().startswith(b"x-forwarded-for:")]
    lines.insert(1, b"X-Forwarded-For: " + forwarded_for.encode())
    return b"\r\n".join(lines) + b"\r\n\r\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    ap.add_argument("--listen", type=int, required=True)
    ap.add_argument("--to", type=int, required=True, help="the gateway's port on 127.0.0.1")
    ap.add_argument("--dir", default=os.path.join(ROOT, "build", "tls"), help="where the test CA and certificate are kept")
    ap.add_argument("--forwarded-for")
    args = ap.parse_args()

    leaf, key, ca = make_certs(args.dir)
    context = ssl.create_default_context(ssl.Purpose.CLIENT_AUTH)
    context.load_cert_chain(leaf, key)

    async def handle(creader, cwriter):
        try:
            head = await asyncio.wait_for(first_request(creader, args.forwarded_for), 30) if args.forwarded_for else b""
            breader, bwriter = await asyncio.open_connection("127.0.0.1", args.to)
        except (OSError, asyncio.TimeoutError, asyncio.IncompleteReadError, asyncio.LimitOverrunError):
            cwriter.close()
            return
        await asyncio.gather(pipe(creader, bwriter, head), pipe(breader, cwriter))

    async def serve():
        server = await asyncio.start_server(handle, "127.0.0.1", args.listen, ssl=context)
        print("https://127.0.0.1:%d -> 127.0.0.1:%d (trust %s with BSK_CA)" % (args.listen, args.to, ca), flush=True)
        async with server:
            await server.serve_forever()

    try:
        asyncio.run(serve())
    except KeyboardInterrupt:
        pass


if __name__ == "__main__":
    main()
