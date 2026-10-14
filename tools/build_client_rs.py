"""Build the Rust client (client-rs/) for the desktop or the browser.

Usage:
    python tools/build_client_rs.py
        build/client-rs/native: betterskillys(.exe) with its assets/ folder beside it.

    python tools/build_client_rs.py --web
        build/client-rs/web: index.html, the wasm module and assets/, ready for any static web
        server (python -m http.server, nginx...). Needs the wasm32-unknown-unknown target and
        wasm-bindgen-cli at the version in client-rs/Cargo.lock.

    --debug builds without optimisation (much slower to run).

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


def build_native(release, out):
    profile = "release" if release else "debug"
    run(["cargo", "build", "--bin", BIN] + (["--release"] if release else []), cwd=CRATE)
    exe = BIN + (".exe" if os.name == "nt" else "")
    os.makedirs(out, exist_ok=True)
    shutil.copy2(os.path.join(CRATE, "target", profile, exe), os.path.join(out, exe))
    copy_assets(out)


def build_web(release, out):
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
        + (["--release"] if release else []), cwd=CRATE)
    os.makedirs(out, exist_ok=True)
    wasm = os.path.join(CRATE, "target", "wasm32-unknown-unknown", profile, BIN + ".wasm")
    run(["wasm-bindgen", "--target", "web", "--no-typescript", "--out-dir", out, wasm])
    shutil.copy2(os.path.join(CRATE, "web", "index.html"), os.path.join(out, "index.html"))
    copy_assets(out)


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--web", action="store_true", help="build for the browser")
    parser.add_argument("--debug", action="store_true", help="unoptimised build")
    args = parser.parse_args()
    out = os.path.join(ROOT, "build", "client-rs", "web" if args.web else "native")
    (build_web if args.web else build_native)(not args.debug, out)
    print("built", os.path.relpath(out, ROOT))


if __name__ == "__main__":
    main()
