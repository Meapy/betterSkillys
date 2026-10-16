"""The world proxy: owns the public world server port and forwards each game connection to one of
two world servers on this PC, so a new world server can take over without disconnecting anyone.

    python tools/world_proxy.py <state folder>       # build/live, or build/iso<N>/live

tools/rolling_restart.py starts it and keeps its state, <state folder>/state.json:

    {"front_port": 2051, "bind": "0.0.0.0", "active": "b",
     "backends": {"a": {"port": 12051, "pid": 123}, "b": {"port": 22051, "pid": 456}}}

- Each connection's first message is the client's Hello. Its game id says where it goes: world
  ids from 1000000 belong to world server slot a, from 2000000 to slot b (each server's
  serverSettings.worldIdBase), so a client changing worlds inside the old server stays there.
  The Nexus (-2), the other negative ids and anything unknown go to the active server.
- Before the Hello it sends a PROXY_INFO message (id 250) with the client's address, as the
  servers would otherwise see every player as 127.0.0.1 (IP bans, the account's IP).
- It reads the state again whenever the file changes: a deploy only rewrites it.
- When the active server asks for its scheduled restart (restart.request in its folder), it runs
  `rolling_restart.py deploy --reuse`, so the 2-hourly restart kicks nobody either.
"""
import asyncio
import ctypes
import json
import os
import socket
import struct
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
HELLO, PROXY_INFO = 16, 250
POLICY = struct.pack(">i", 1014001516)  # "<pol": a Flash policy-file request
SLOT_BY_RANGE = {1: "a", 2: "b"}       # world id // 1000000
MAX_FIRST = 1 << 20

STATE_DIR = None
state = {}
state_mtime = 0
log_file = None


def log(msg):
    line = time.strftime("%Y-%m-%d %H:%M:%S ") + msg
    print(line, flush=True)
    if log_file:
        log_file.write(line + "\n")
        log_file.flush()


def load_state():
    global state, state_mtime
    path = os.path.join(STATE_DIR, "state.json")
    try:
        mtime = os.stat(path).st_mtime
        if mtime == state_mtime:
            return
        with open(path, encoding="utf-8") as f:
            new = json.load(f)
    except (OSError, ValueError):
        return  # being rewritten: next time
    if state.get("active") != new.get("active"):
        log("active world server: %s (port %s)" % (new.get("active"), backend_port(new, new.get("active"))))
    state, state_mtime = new, mtime


def backend_port(st, slot):
    b = st.get("backends", {}).get(slot)
    return b and b.get("port")


def route(game_id):
    """The slot a Hello with this game id goes to."""
    active = state.get("active")
    if game_id is not None and game_id > 0:
        slot = SLOT_BY_RANGE.get(game_id // 1000000)
        if slot and backend_port(state, slot):
            return slot
    return active


def hello_game_id(body):
    """The game id in a Hello's body (after the message id), or None."""
    try:
        (n,) = struct.unpack_from(">h", body, 0)
        (game_id,) = struct.unpack_from(">i", body, 2 + n)
        return game_id
    except struct.error:
        return None


def proxy_info(address):
    data = address.encode("utf-8")
    body = struct.pack(">h", len(data)) + data
    return struct.pack(">iB", 5 + len(body), PROXY_INFO) + body


async def pipe(reader, writer):
    try:
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


def nodelay(writer):
    sock = writer.get_extra_info("socket")
    if sock is not None:
        try:
            sock.setsockopt(socket.IPPROTO_TCP, socket.TCP_NODELAY, 1)
        except OSError:
            pass


async def handle(creader, cwriter):
    nodelay(cwriter)
    peer = cwriter.get_extra_info("peername") or ("?", 0)
    address = peer[0]
    if address.startswith("::ffff:"):
        address = address[7:]
    try:
        first = await asyncio.wait_for(creader.readexactly(4), 30)
        game_id = None
        if first != POLICY:
            (length,) = struct.unpack(">i", first)
            if 5 <= length <= MAX_FIRST:
                rest = await asyncio.wait_for(creader.readexactly(length - 4), 30)
                first += rest
                if rest[0] == HELLO:
                    game_id = hello_game_id(rest[1:])
    except (asyncio.TimeoutError, asyncio.IncompleteReadError, OSError):
        cwriter.close()
        return

    load_state()
    slot = route(game_id)
    tried = []
    for s in (slot, state.get("active")):
        port = backend_port(state, s)
        if not port or s in tried:
            continue
        tried.append(s)
        try:
            breader, bwriter = await asyncio.wait_for(asyncio.open_connection("127.0.0.1", port), 5)
            break
        except (OSError, asyncio.TimeoutError):
            log("world server %s (port %s) didn't answer" % (s, port))
    else:
        log("no world server for %s (game %s)" % (address, game_id))
        cwriter.close()
        return

    nodelay(bwriter)
    if first != POLICY:
        bwriter.write(proxy_info(address))
    bwriter.write(first)
    log("%s -> %s (game %s)" % (address, tried[-1], game_id))
    await asyncio.gather(pipe(creader, bwriter), pipe(breader, cwriter))


def alive(pid):
    if not pid:
        return False
    k = ctypes.windll.kernel32
    h = k.OpenProcess(0x1000, False, pid)  # PROCESS_QUERY_LIMITED_INFORMATION
    if not h:
        return False
    code = ctypes.c_ulong()
    ok = k.GetExitCodeProcess(h, ctypes.byref(code))
    k.CloseHandle(h)
    return bool(ok) and code.value == 259  # STILL_ACTIVE


async def supervise():
    """Reloads the state, and runs a rolling restart when the active server asks for one."""
    restarting = None
    while True:
        load_state()
        active = state.get("active")
        request = active and os.path.join(STATE_DIR, active, "restart.request")
        if restarting is not None and restarting.poll() is not None:
            log("scheduled restart finished (exit code %s)" % restarting.returncode)
            restarting = None
        if request and os.path.exists(request) and restarting is None:
            try:
                os.remove(request)
            except OSError:
                pass
            log("world server %s asks for its scheduled restart: starting a new one beside it" % active)
            out = open(os.path.join(STATE_DIR, "restart.log"), "a")
            restarting = subprocess.Popen([sys.executable, os.path.join(HERE, "rolling_restart.py")]
                                          + state.get("tool_args", []) + ["deploy", "--reuse"],
                                          stdout=out, stderr=subprocess.STDOUT,
                                          creationflags=subprocess.CREATE_NO_WINDOW)
        await asyncio.sleep(1)


async def main():
    global STATE_DIR, log_file
    if len(sys.argv) != 2:
        sys.exit(__doc__)
    STATE_DIR = os.path.abspath(sys.argv[1])
    log_file = open(os.path.join(STATE_DIR, "proxy.log"), "a", encoding="utf-8")
    load_state()
    if not state:
        sys.exit("no state in %s (start it with tools/rolling_restart.py up)" % STATE_DIR)
    try:
        ctypes.windll.kernel32.SetConsoleTitleW("World Proxy (port %d)" % state["front_port"])
    except Exception:
        pass
    server = await asyncio.start_server(handle, state.get("bind", "0.0.0.0"), state["front_port"])
    log("world proxy on %s:%d, active world server %s" % (state.get("bind", "0.0.0.0"), state["front_port"], state.get("active")))
    with open(os.path.join(STATE_DIR, "proxy.pid"), "w") as f:
        f.write(str(os.getpid()))
    async with server:
        await asyncio.gather(server.serve_forever(), supervise())


if __name__ == "__main__":
    asyncio.run(main())
