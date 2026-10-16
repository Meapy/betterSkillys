"""Deploys a new world server build without disconnecting anyone, behind the world proxy.

    python tools/rolling_restart.py up              # proxy + a world server, if they aren't running
    python tools/rolling_restart.py update          # up if nothing runs, else deploy (Run Server.bat)
    python tools/rolling_restart.py deploy          # build the current source and switch to it
    python tools/rolling_restart.py deploy --reuse  # same build again (the scheduled restart)
    python tools/rolling_restart.py status
    python tools/rolling_restart.py stop            # stops the proxy and the world servers: kicks everyone

    --iso N (before the command) works on isolated test stack N instead (Redis 6380+N, world port
    2052+N, under build/iso<N>/live; start its Redis and account server with
    `iso_stack.py --slot N start --rolling`).

How it fits together:
- tools/world_proxy.py owns the public world port (2051) and forwards each connection to world
  server slot a (port 12051, world ids from 1000000) or b (22051, from 2000000). Both run from
  build/live/<slot>, built from source/WorldServer with that slot's port and id range in their
  wServer.json (serverInfo.listenPort, serverSettings.worldIdBase).
- deploy builds into the slot that isn't active, starts it, waits until it listens, makes it the
  active one (the proxy sends the Nexus and new logins there) and asks the old one to hand over
  (drain.request in its folder, with the minutes to allow). Players on the old one keep their
  world, dungeons included, and switch at their next trip to the Nexus or a realm; those in its
  Nexus switch at once; when the time is up (--minutes, 60) everyone left is moved to the new
  Nexus. The old server then stops by itself.
- A deploy while the other slot is still handing over waits for it, or with --now moves its
  players to the new Nexus first.
"""
import argparse
import ctypes
import json
import os
import shutil
import socket
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(HERE)
SOURCE = os.path.join(ROOT, "source")
SLOTS = {"a": 1, "b": 2}

LIVE = None   # set by configure()
ISO = None


def configure(iso):
    global LIVE, ISO
    ISO = iso
    LIVE = os.path.join(ROOT, "build", "live" if iso is None else os.path.join("iso%s" % (iso or ""), "live"))
    os.makedirs(LIVE, exist_ok=True)


def front_port():
    return 2051 if ISO is None else 2052 + ISO


def slot_port(slot):
    return front_port() + 10000 * SLOTS[slot]


def state_path():
    return os.path.join(LIVE, "state.json")


def read_state():
    try:
        with open(state_path(), encoding="utf-8") as f:
            return json.load(f)
    except (OSError, ValueError):
        return {"front_port": front_port(), "bind": "0.0.0.0" if ISO is None else "127.0.0.1",
                "tool_args": [] if ISO is None else ["--iso", str(ISO)], "active": None, "backends": {}}


def write_state(st):
    tmp = state_path() + ".tmp"
    with open(tmp, "w", encoding="utf-8") as f:
        json.dump(st, f, indent=2)
    for _ in range(50):  # the proxy may be reading it
        try:
            os.replace(tmp, state_path())
            return
        except PermissionError:
            time.sleep(0.1)
    os.replace(tmp, state_path())


def alive(pid):
    if not pid:
        return False
    k = ctypes.windll.kernel32
    h = k.OpenProcess(0x1000, False, pid)
    if not h:
        return False
    code = ctypes.c_ulong()
    ok = k.GetExitCodeProcess(h, ctypes.byref(code))
    k.CloseHandle(h)
    return bool(ok) and code.value == 259


def listening(port):
    try:
        with socket.create_connection(("127.0.0.1", port), timeout=1):
            return True
    except OSError:
        return False


def popen(args, cwd, out_name):
    """Live: in its own console window, like the other servers. Test stacks: hidden, output to a file."""
    if ISO is None:
        return subprocess.Popen(args, cwd=cwd, creationflags=subprocess.CREATE_NEW_CONSOLE)
    return subprocess.Popen(args, cwd=cwd, stdout=open(os.path.join(cwd, out_name), "a"), stderr=subprocess.STDOUT,
                            creationflags=subprocess.CREATE_NO_WINDOW | subprocess.CREATE_NEW_PROCESS_GROUP)


def ensure_proxy(st):
    pid_file = os.path.join(LIVE, "proxy.pid")
    pid = int(open(pid_file).read()) if os.path.exists(pid_file) else 0
    if alive(pid) and listening(st["front_port"]):
        return
    if listening(st["front_port"]):
        sys.exit("port %d is taken by something else (an old world server outside the proxy?): stop it first"
                 % st["front_port"])
    p = popen([sys.executable, os.path.join(HERE, "world_proxy.py"), LIVE], LIVE, "proxy.out")
    for _ in range(50):
        if listening(st["front_port"]):
            print("world proxy started on port %d (pid %d)" % (st["front_port"], p.pid))
            return
        time.sleep(0.2)
    sys.exit("the world proxy didn't start: see %s" % os.path.join(LIVE, "proxy.log"))


def set_json(path, fn):
    with open(path, encoding="utf-8-sig") as f:
        data = json.load(f)
    fn(data)
    with open(path, "w", encoding="utf-8") as f:
        json.dump(data, f, indent=2)


def prepare(slot, reuse_from=None):
    """Puts a world server build in build/live/<slot>, configured for that slot."""
    folder = os.path.join(LIVE, slot)
    if reuse_from:
        src = os.path.join(LIVE, reuse_from)
        print("copying the running build from slot %s to slot %s" % (reuse_from, slot))
        skip = shutil.ignore_patterns("logs", "GenerateXMLS", "*.request", "*.status", "*.out")
        shutil.copytree(src, folder, ignore=skip, dirs_exist_ok=True)
    else:
        print("building source/WorldServer into %s" % os.path.relpath(folder, ROOT))
        subprocess.check_call(["dotnet", "build", os.path.join(SOURCE, "WorldServer", "WorldServer.csproj"),
                               "-c", "Debug", "-o", folder, "-v", "q", "--nologo"])
        shutil.copyfile(os.path.join(SOURCE, "WorldServer", "wServer.json"), os.path.join(folder, "wServer.json"))
    return configure_slot(slot)


def configure_slot(slot):
    """Sets a slot's port and world id range in its wServer.json, and clears old requests."""
    folder = os.path.join(LIVE, slot)
    for name in ("drain.request", "drain.status", "restart.request"):
        if os.path.exists(os.path.join(folder, name)):
            os.remove(os.path.join(folder, name))

    def cfg(d):
        d["serverInfo"]["listenPort"] = slot_port(slot)
        d["serverSettings"]["worldIdBase"] = 1000000 * SLOTS[slot]
        if ISO is not None:
            d["dbInfo"]["port"] = 6380 + ISO
            d["serverInfo"]["port"] = front_port()
            d["serverInfo"]["address"] = "127.0.0.1"
    set_json(os.path.join(folder, "wServer.json"), cfg)
    return folder


def start_backend(st, slot, folder):
    if listening(slot_port(slot)):
        sys.exit("port %d (slot %s) is already taken" % (slot_port(slot), slot))
    args = [os.path.join(folder, "WorldServer.exe")] if ISO is None else ["dotnet", "WorldServer.dll"]
    p = popen(args, folder, "ws.out")
    print("started world server %s (pid %d, port %d); waiting until it listens..." % (slot, p.pid, slot_port(slot)))
    deadline = time.time() + 300
    while not listening(slot_port(slot)):
        if not alive(p.pid):
            sys.exit("world server %s stopped while starting: see %s" % (slot, os.path.join(folder, "logs")))
        if time.time() > deadline:
            sys.exit("world server %s didn't start listening in 5 minutes" % slot)
        time.sleep(1)
    st["backends"][slot] = {"port": slot_port(slot), "pid": p.pid, "started": time.strftime("%Y-%m-%d %H:%M:%S")}
    print("world server %s is up" % slot)


def prune(st):
    """Forgets world servers that have stopped."""
    for slot, b in list(st["backends"].items()):
        if not alive(b.get("pid")):
            del st["backends"][slot]
    if st["active"] not in st["backends"]:
        st["active"] = None


def players_left(slot):
    try:
        return int(open(os.path.join(LIVE, slot, "drain.status")).read().strip() or 0)
    except (OSError, ValueError):
        return None


def drain(slot, minutes):
    with open(os.path.join(LIVE, slot, "drain.request"), "w") as f:
        f.write(str(minutes))


def lock():
    path = os.path.join(LIVE, "deploy.lock")
    try:
        if time.time() - os.stat(path).st_mtime > 1800:
            os.remove(path)  # left by a deploy that died
    except OSError:
        pass
    try:
        fd = os.open(path, os.O_CREAT | os.O_EXCL | os.O_WRONLY)
    except FileExistsError:
        sys.exit("another deploy is running (%s)" % path)
    os.write(fd, str(os.getpid()).encode())
    os.close(fd)
    return path


def cmd_up(args):
    st = read_state()
    prune(st)
    if not st["active"]:
        slot = "a" if "a" not in st["backends"] else "b"
        if args.no_build and os.path.exists(os.path.join(LIVE, slot, "WorldServer.dll")):
            folder = configure_slot(slot)
        else:
            folder = prepare(slot)
        start_backend(st, slot, folder)
        st["active"] = slot
    write_state(st)
    ensure_proxy(st)
    print("world server %s is active behind the proxy on port %d" % (st["active"], st["front_port"]))


def cmd_deploy(args):
    st = read_state()
    prune(st)
    if not st["active"]:
        sys.exit("no world server is running: use `up`")
    old = st["active"]
    new = "b" if old == "a" else "a"
    path = lock()
    try:
        if new in st["backends"]:
            left = players_left(new)
            if not args.now:
                print("world server %s is still handing over (%s players left): waiting for it (--now moves them)"
                      % (new, "?" if left is None else left))
            else:
                print("moving the players still on world server %s to the Nexus" % new)
                drain(new, 0)
            while alive(st["backends"][new]["pid"]):
                time.sleep(2)
            del st["backends"][new]
            write_state(st)
        folder = prepare(new, reuse_from=old if args.reuse else None)
        start_backend(st, new, folder)
        st["active"] = new
        write_state(st)
        print("the proxy now sends new connections and the Nexus to world server %s" % new)
        if alive(st["backends"][old]["pid"]):
            drain(old, args.minutes)
            print("world server %s hands its players over (at their next Nexus trip, within %s minutes) and then stops"
                  % (old, args.minutes))
    finally:
        os.remove(path)


def cmd_update(args):
    """What Run Server.bat does: start the proxy and a world server if none is running, otherwise
    deploy the current source to them without kicking anyone. A deploy started while the previous
    one is still handing over moves that one's last players to the Nexus rather than waiting up to
    an hour."""
    st = read_state()
    prune(st)
    if not st["active"]:
        args.no_build = False
        cmd_up(args)
        return
    ensure_proxy(st)
    args.reuse, args.now = False, True
    cmd_deploy(args)


def cmd_status(_):
    st = read_state()
    pid_file = os.path.join(LIVE, "proxy.pid")
    pid = int(open(pid_file).read()) if os.path.exists(pid_file) else 0
    print("proxy: port %d, %s" % (st["front_port"], "running (pid %d)" % pid if alive(pid) else "not running"))
    for slot, b in sorted(st["backends"].items()):
        state = "running" if alive(b.get("pid")) else "stopped"
        left = players_left(slot)
        extra = "" if left is None else ", handing over: %d players left" % left
        print("world server %s: port %d, pid %s, %s%s%s" % (slot, b["port"], b.get("pid"), state,
                                                           " (active)" if slot == st["active"] else "", extra))


def kill(pid):
    if alive(pid):
        subprocess.run(["taskkill", "/PID", str(pid), "/T", "/F"], capture_output=True)


def cmd_stop(_):
    st = read_state()
    pid_file = os.path.join(LIVE, "proxy.pid")
    if os.path.exists(pid_file):
        kill(int(open(pid_file).read()))
        os.remove(pid_file)
        print("stopped the proxy")
    for slot, b in st["backends"].items():
        kill(b.get("pid"))
        print("stopped world server", slot)
    st["backends"], st["active"] = {}, None
    write_state(st)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--iso", type=int, help="isolated test stack N instead of the live servers")
    sub = ap.add_subparsers(dest="cmd", required=True)
    u = sub.add_parser("up")
    u.add_argument("--no-build", action="store_true", help="start the last build of the slot")
    d = sub.add_parser("deploy")
    d.add_argument("--reuse", action="store_true", help="copy the running build instead of building the source")
    d.add_argument("--minutes", type=float, default=60, help="how long players get to reach the Nexus (60)")
    d.add_argument("--now", action="store_true", help="if the other slot is still handing over, move its players now")
    upd = sub.add_parser("update")
    upd.add_argument("--minutes", type=float, default=60, help="how long players get to reach the Nexus (60)")
    sub.add_parser("status")
    sub.add_parser("stop")
    args = ap.parse_args()
    configure(args.iso)
    {"up": cmd_up, "update": cmd_update, "deploy": cmd_deploy, "status": cmd_status, "stop": cmd_stop}[args.cmd](args)


if __name__ == "__main__":
    main()
