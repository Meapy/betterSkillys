"""Build the Rust client (client-rs/) for the desktop or the browser.

Usage:
    python tools/build_client_rs.py
        build/client-rs/native: betterskillys(.exe) with its assets/ folder beside it.

    python tools/build_client_rs.py --web
        build/client-rs/web: index.html, the wasm module and assets/, ready for any static web
        server (python -m http.server, nginx...). Needs the wasm32-unknown-unknown target and
        wasm-bindgen-cli at the version in client-rs/Cargo.lock.

    --debug builds without optimisation (much slower to run).

    python tools/build_client_rs.py --account http://188.141.12.3:8080 --package
        As above with that account server built in, then dist/betterSkillys-rs-<os>.zip (the
        game and a short readme) to hand to other players, as build_client.py --package does
        for the AIR client. With --web, the page's files go in the zip instead.

Run tools/gen_rs_manifest.py first if the AIR client's asset lists changed.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CRATE = os.path.join(ROOT, "client-rs")
SRC = os.path.join(ROOT, "client", "src")
MANIFEST = os.path.join(CRATE, "assets", "manifest.json")
BIN = "betterskillys"


def run(cmd, **kw):
    print(">", " ".join(cmd))
    subprocess.run(cmd, check=True, **kw)


def manifest_files():
    with open(MANIFEST, encoding="utf-8") as f:
        m = json.load(f)
    files = set()
    for entry in m["image_sets"] + m["animated_chars"] + m["models"]:
        files.update(p for p in (entry.get("file"), entry.get("mask")) if p)
    files.update(m["ground_xml"] + m["object_xml"] + m["region_xml"])
    files.update([m["particles"], m["fonts"]["regular"], m["fonts"]["bold"]])
    files.update(m.get("screens", {}).values())
    return sorted(files)


def copy_assets(out):
    """assets/manifest.json plus every file it names, at the same relative paths."""
    assets = os.path.join(out, "assets")
    if os.path.isdir(assets):
        shutil.rmtree(assets)
    os.makedirs(assets)
    shutil.copy2(MANIFEST, os.path.join(assets, "manifest.json"))
    files = manifest_files()
    for rel in files:
        dst = os.path.join(assets, rel)
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        shutil.copy2(os.path.join(SRC, rel), dst)
    print(f"{len(files)} asset files copied")


def bindgen_version():
    with open(os.path.join(CRATE, "Cargo.lock"), encoding="utf-8") as f:
        m = re.search(r'name = "wasm-bindgen"\nversion = "([^"]+)"', f.read())
    return m.group(1)


def cargo_env(account):
    """The environment for cargo: BSK_ACCOUNT builds that account server in."""
    env = dict(os.environ)
    if account:
        env["BSK_ACCOUNT"] = account
    return env


README = """betterSkillys
=============

Run {exe} to play. Keep the assets folder beside it.

The game connects to {account}.
To play on another server, start it with: {exe} --account http://<address>:8080

Settings and a remembered login are kept in {store}.
"""


def package(out, web, account):
    """dist/betterSkillys-rs-<os>.zip: the built files and a readme."""
    import platform
    import zipfile
    name = "web" if web else {"Windows": "windows", "Darwin": "macos"}.get(platform.system(), "linux")
    dist = os.path.join(ROOT, "dist")
    os.makedirs(dist, exist_ok=True)
    path = os.path.join(dist, f"betterSkillys-rs-{name}.zip")
    exe = BIN + (".exe" if os.name == "nt" and not web else "")
    store = {"windows": "%APPDATA%\\betterSkillys-rs", "macos": "~/Library/Application Support/betterSkillys-rs",
             "linux": "~/.config/betterSkillys-rs", "web": "the browser's storage"}[name]
    with zipfile.ZipFile(path, "w", zipfile.ZIP_DEFLATED) as z:
        for dirpath, _, files in os.walk(out):
            for f in files:
                full = os.path.join(dirpath, f)
                z.write(full, os.path.join("betterSkillys", os.path.relpath(full, out)))
        if not web:
            z.writestr("betterSkillys/README.txt", README.format(exe=exe, account=account or "this PC (127.0.0.1:8080)", store=store))
    print("packaged", os.path.relpath(path, ROOT), f"({os.path.getsize(path) // 1024} KB)")


def build_native(release, out, account=None):
    profile = "release" if release else "debug"
    run(["cargo", "build", "--bin", BIN] + (["--release"] if release else []), cwd=CRATE, env=cargo_env(account))
    exe = BIN + (".exe" if os.name == "nt" else "")
    os.makedirs(out, exist_ok=True)
    shutil.copy2(os.path.join(CRATE, "target", profile, exe), os.path.join(out, exe))
    copy_assets(out)


def build_web(release, out, account=None):
    want = bindgen_version()
    try:
        have = subprocess.run(["wasm-bindgen", "--version"], capture_output=True, text=True).stdout.split()[-1]
    except (FileNotFoundError, IndexError):
        have = None
    if have != want:
        sys.exit(f"wasm-bindgen-cli {want} is needed (found {have}): "
                 f"cargo install wasm-bindgen-cli --version {want} --locked")
    profile = "release" if release else "debug"
    run(["cargo", "build", "--bin", BIN, "--target", "wasm32-unknown-unknown"]
        + (["--release"] if release else []), cwd=CRATE, env=cargo_env(account))
    os.makedirs(out, exist_ok=True)
    wasm = os.path.join(CRATE, "target", "wasm32-unknown-unknown", profile, BIN + ".wasm")
    run(["wasm-bindgen", "--target", "web", "--no-typescript", "--out-dir", out, wasm])
    shutil.copy2(os.path.join(CRATE, "web", "index.html"), os.path.join(out, "index.html"))
    copy_assets(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--web", action="store_true", help="build for the browser")
    parser.add_argument("--debug", action="store_true", help="unoptimised build")
    parser.add_argument("--account", help="the account server to build in, e.g. http://188.141.12.3:8080")
    parser.add_argument("--package", action="store_true", help="also zip it into dist/ for other players")
    args = parser.parse_args()
    out = os.path.join(ROOT, "build", "client-rs", "web" if args.web else "native")
    (build_web if args.web else build_native)(not args.debug, out, args.account)
    print("built", os.path.relpath(out, ROOT))
    if args.package:
        package(out, args.web, args.account)


if __name__ == "__main__":
    main()
