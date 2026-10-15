"""Isolated test servers, next to the live ones and sharing nothing with them.

    python tools/testclient/iso_stack.py start [--fresh-data] [--no-build] [--data-from DIR]
    python tools/testclient/iso_stack.py stop
    python tools/testclient/iso_stack.py status

Redis 6380, account server 8081, world server 2052, all under build/iso/. Build the test client
with --port 8081 to use it.

- --slot N (before the command) runs another, independent stack on ports N higher, under
  build/iso<N>/: slot 1 is Redis 6381, account 8082, world 2053. Use it when another session's
  stack holds slot 0.
- --data-from DIR copies the Redis data from DIR instead of source/Redis/data (a worktree has no
  live data; point it at the main checkout's source/Redis/data).

- start builds App and WorldServer from the current source into build/iso (the live servers'
  bin folders are locked while they run, and are left alone), sets their ports, and starts them.
- The Redis data is a copy of the live database, made on the first start (or with --fresh-data),
  so test characters and accounts can be changed freely.
- The processes don't use the live servers' image names (App.exe, WorldServer.exe,
  redis-server.exe): Run Server.bat skips starting a server when a process with its name is
  already running, so a test stack with those names would stop the live servers from starting.
- stop only stops the processes this script started (their ids are in build/iso/pids.json).
"""
import argparse
import json
import os
import shutil
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
SOURCE = os.path.join(ROOT, "source")
ISO = os.path.join(ROOT, "build", "iso")
PIDS = os.path.join(ISO, "pids.json")
REDIS_PORT, APP_PORT, WORLD_PORT = 6380, 8081, 2052


def use_slot(slot):
    global ISO, PIDS, REDIS_PORT, APP_PORT, WORLD_PORT
    if slot:
        ISO = os.path.join(ROOT, "build", "iso%d" % slot)
        PIDS = os.path.join(ISO, "pids.json")
        REDIS_PORT, APP_PORT, WORLD_PORT = 6380 + slot, 8081 + slot, 2052 + slot


def set_json(path, fn):
    data = json.load(open(path, encoding="utf-8-sig"))
    fn(data)
    json.dump(data, open(path, "w", encoding="utf-8"), indent=2)


def build(project, out):
    subprocess.check_call(["dotnet", "build", os.path.join(SOURCE, project), "-c", "Debug", "-o", out,
                           "-v", "q", "--nologo"])


def running(pid):
    out = subprocess.run(["tasklist", "/FI", "PID eq %d" % pid, "/NH"], capture_output=True, text=True).stdout
    return str(pid) in out


def start(args):
    if os.path.exists(PIDS) and any(running(p) for p in json.load(open(PIDS)).values()):
        sys.exit("already running (python tools/testclient/iso_stack.py stop)")
    redis_dir = os.path.join(ISO, "redis")
    app_dir = os.path.join(ISO, "app")
    world_dir = os.path.join(ISO, "ws")
    os.makedirs(redis_dir, exist_ok=True)

    if args.fresh_data or not os.path.exists(os.path.join(redis_dir, "appendonly.aof")):
        for name in ("dump.rdb", "appendonly.aof"):
            live = os.path.join(args.data_from or os.path.join(SOURCE, "Redis", "data"), name)
            if os.path.exists(live):
                shutil.copyfile(live, os.path.join(redis_dir, name))
    shutil.copyfile(os.path.join(SOURCE, "Redis", "redis-server.exe"), os.path.join(redis_dir, "iso-redis-server.exe"))
    with open(os.path.join(redis_dir, "redis.conf"), "w") as f:
        f.write("bind 127.0.0.1\nport %d\nprotected-mode yes\ndir ./\ndbfilename dump.rdb\n"
                "appendonly yes\nappendfilename \"appendonly.aof\"\nappendfsync everysec\n" % REDIS_PORT)

    if not args.no_build:
        build(os.path.join("App", "App.csproj"), app_dir)
        build(os.path.join("WorldServer", "WorldServer.csproj"), world_dir)

    def app_cfg(d):
        d["dbInfo"]["port"] = REDIS_PORT
        d["serverInfo"]["port"] = APP_PORT
        # Its own published client (build_client_rs.py --publish-to build/iso<N>/client).
        d["serverSettings"]["clientFolder"] = os.path.join(ISO, "client")
    set_json(os.path.join(app_dir, "server.json"), app_cfg)

    def world_cfg(d):
        d["dbInfo"]["port"] = REDIS_PORT
        d["serverInfo"]["port"] = WORLD_PORT
        d["serverInfo"]["address"] = "127.0.0.1"
    set_json(os.path.join(world_dir, "wServer.json"), world_cfg)

    flags = subprocess.CREATE_NO_WINDOW
    pids = {}
    pids["redis"] = subprocess.Popen([os.path.join(redis_dir, "iso-redis-server.exe"), "redis.conf"], cwd=redis_dir,
                                     stdout=open(os.path.join(redis_dir, "redis.out"), "w"),
                                     stderr=subprocess.STDOUT, creationflags=flags).pid
    time.sleep(2)
    pids["app"] = subprocess.Popen(["dotnet", "App.dll"], cwd=app_dir,
                                   stdout=open(os.path.join(app_dir, "app.out"), "w"),
                                   stderr=subprocess.STDOUT, creationflags=flags).pid
    pids["world"] = subprocess.Popen(["dotnet", "WorldServer.dll"], cwd=world_dir,
                                     stdout=open(os.path.join(world_dir, "ws.out"), "w"),
                                     stderr=subprocess.STDOUT, creationflags=flags).pid
    json.dump(pids, open(PIDS, "w"))
    print("started", pids, "- Redis %d, account %d, world %d; world log %s"
          % (REDIS_PORT, APP_PORT, WORLD_PORT, os.path.relpath(os.path.join(world_dir, "ws.out"), ROOT)))


def stop(_):
    if not os.path.exists(PIDS):
        print("not running")
        return
    for name, pid in json.load(open(PIDS)).items():
        if running(pid):
            subprocess.run(["taskkill", "/PID", str(pid), "/T", "/F"], capture_output=True)
            print("stopped", name, pid)
    os.remove(PIDS)


def status(_):
    if not os.path.exists(PIDS):
        print("not running")
        return
    for name, pid in json.load(open(PIDS)).items():
        print(name, pid, "running" if running(pid) else "stopped")


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--slot", type=int, default=0, help="run stack N, on ports N higher (default 0)")
    sub = ap.add_subparsers(dest="cmd", required=True)
    s = sub.add_parser("start")
    s.add_argument("--fresh-data", action="store_true", help="copy the live database again")
    s.add_argument("--no-build", action="store_true", help="reuse the last build in build/iso")
    s.add_argument("--data-from", help="copy the Redis data from this folder")
    sub.add_parser("stop")
    sub.add_parser("status")
    args = ap.parse_args()
    use_slot(args.slot)
    {"start": start, "stop": stop, "status": status}[args.cmd](args)


if __name__ == "__main__":
    main()
