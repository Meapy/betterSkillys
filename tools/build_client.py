"""Build the betterSkillys client with the AIR SDK in tools/airsdk.

Usage:
    python tools/build_client.py
        client/bin/client.swf, connecting to this PC (127.0.0.1). "Play Client.bat" runs it.

    python tools/build_client.py <server-host> --package
        dist/betterSkillys-Client/ and dist/betterSkillys-Client.zip: a self-contained Windows
        app (AIR captive runtime, nothing to install) for other players, connecting to <server-host>.

The account server address is compiled into LocalhostSetup.as, so the sources are copied to
build/src and that one constant is rewritten there; client/src is left untouched.
"""

import argparse
import os
import re
import secrets
import shutil
import subprocess
import sys
import zipfile

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
CLIENT = os.path.join(ROOT, "client")
SDK_BIN = os.path.join(ROOT, "tools", "airsdk", "bin")
BUILD = os.path.join(ROOT, "build")
SETUP_AS = os.path.join("kabam", "rotmg", "application", "impl", "LocalhostSetup.as")
NAME = "betterSkillys-Client"

# Compiler options from client/.actionScriptProperties (Flash Builder project), for an AIR 32 SWF.
COMPILER_ARGS = [
    "-default-size", "800", "600",
    "-default-frame-rate", "60",
    "-default-background-color", "0x000000",
    "-swf-version", "43",
    "-optimize=true",
    "-keep-as3-metadata+=Inject",
    "-keep-as3-metadata+=Embed",
    "-keep-as3-metadata+=PostConstruct",
    "-keep-as3-metadata+=ArrayElementType",
]

LAUNCHER = "\r\n".join([
    "@ECHO OFF",
    "CD /D \"%~dp0\"",
    "START \"\" betterSkillys.exe",
    "",
])

README = """betterSkillys
=============

1. Unzip this whole folder somewhere (e.g. your Desktop).
2. Double-click "Play betterSkillys.bat" (or betterSkillys.exe).
3. Click "log in" > "New user? Click here to Register", then "play".

The game runs on the Adobe AIR runtime included in this folder; nothing needs installing.
Windows only. If Windows SmartScreen warns about betterSkillys.exe, click "More info" > "Run anyway".

Server: {host}
"""


def tool(name):
    path = os.path.join(SDK_BIN, name)
    if not os.path.exists(path):
        sys.exit("Missing %s - unzip AIRSDK_Compiler.zip (AIR SDK 32) into tools/airsdk first." % path)
    return path


def compile_client(host, output):
    src = os.path.join(BUILD, "src")
    if os.path.isdir(src):
        shutil.rmtree(src)
    shutil.copytree(os.path.join(CLIENT, "src"), src)
    setup = os.path.join(src, SETUP_AS)
    text = open(setup, encoding="utf-8").read()
    text, n = re.subn(r'(LOCALHOST:String = "http://)[^":/]+(:8080")', r"\g<1>%s\g<2>" % host, text)
    if n != 1:
        sys.exit("Could not find the account server address in " + SETUP_AS)
    open(setup, "w", encoding="utf-8").write(text)

    os.makedirs(os.path.dirname(output), exist_ok=True)
    print("Compiling client for %s:8080 ..." % host)
    subprocess.check_call([tool("amxmlc.bat"), "-source-path=" + src,
                           "-library-path+=" + os.path.join(CLIENT, "libs"),
                           "-output=" + output] + COMPILER_ARGS + [os.path.join(src, "WebMain.as")])


def certificate():
    """Self-signed signing certificate for adt (AIR packages must be signed), made once."""
    cert = os.path.join(BUILD, "signing.p12")
    pw_file = os.path.join(BUILD, "signing-password.txt")
    if not os.path.exists(cert):
        password = secrets.token_urlsafe(18)
        subprocess.check_call([tool("adt.bat"), "-certificate", "-cn", "betterSkillys", "2048-RSA", cert, password])
        open(pw_file, "w").write(password)
    return cert, open(pw_file).read().strip()


def package(host, swf):
    stage = os.path.join(BUILD, "package")
    if os.path.isdir(stage):
        shutil.rmtree(stage)
    os.makedirs(stage)
    shutil.copyfile(swf, os.path.join(stage, "client.swf"))
    shutil.copyfile(os.path.join(CLIENT, "src", "moe.png"), os.path.join(stage, "moe.png"))

    dist = os.path.join(ROOT, "dist")
    # Built beside the current package and swapped in at the end: the current one may be in use
    # (someone playing from it), and deleting it half way breaks it.
    final_dir = os.path.join(dist, NAME)
    out_dir = final_dir + ".new"
    if os.path.isdir(out_dir):
        shutil.rmtree(out_dir)
    os.makedirs(dist, exist_ok=True)
    cert, password = certificate()
    print("Packaging %s ..." % out_dir)
    # -tsa none: Adobe's signing timestamp server no longer answers
    subprocess.check_call([tool("adt.bat"), "-package", "-storetype", "pkcs12", "-keystore", cert,
                           "-storepass", password, "-tsa", "none", "-target", "bundle", out_dir,
                           os.path.join(CLIENT, "WebMain-app.xml"), "-C", stage, "client.swf", "moe.png"])
    with open(os.path.join(out_dir, "Play betterSkillys.bat"), "w", newline="") as f:
        f.write(LAUNCHER)
    with open(os.path.join(out_dir, "README.txt"), "w", newline="\r\n") as f:
        f.write(README.format(host=host))

    zip_path = os.path.join(dist, NAME + ".zip")
    with zipfile.ZipFile(zip_path, "w", zipfile.ZIP_DEFLATED) as z:
        for dirpath, _, files in sorted(os.walk(out_dir)):
            for f in sorted(files):
                path = os.path.join(dirpath, f)
                z.write(path, NAME + "/" + os.path.relpath(path, out_dir).replace(os.sep, "/"))
    print("Wrote %s (%.1f MB)" % (zip_path, os.path.getsize(zip_path) / 1e6))
    old_dir = final_dir + ".old"
    try:
        if os.path.isdir(old_dir):
            shutil.rmtree(old_dir)
        if os.path.isdir(final_dir):
            os.rename(final_dir, old_dir)
        os.rename(out_dir, final_dir)
        shutil.rmtree(old_dir, ignore_errors=True)
        print("Updated %s" % final_dir)
    except OSError as e:
        if not os.path.isdir(final_dir) and os.path.isdir(old_dir):
            os.rename(old_dir, final_dir)
        print("%s is in use (%s); the new one is in %s. Close the client and run this again." % (final_dir, e, out_dir))


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("host", nargs="?", default="127.0.0.1", help="account/world server address (default 127.0.0.1)")
    ap.add_argument("--package", action="store_true", help="build the self-contained app for other players")
    args = ap.parse_args()
    if not re.fullmatch(r"[A-Za-z0-9.-]+", args.host):
        sys.exit("Invalid host: " + args.host)

    if args.package:
        swf = os.path.join(BUILD, "client-%s.swf" % args.host)
        compile_client(args.host, swf)
        package(args.host, swf)
    else:
        swf = os.path.join(CLIENT, "bin", "client.swf")
        compile_client(args.host, swf)
        shutil.copyfile(os.path.join(CLIENT, "src", "moe.png"), os.path.join(CLIENT, "bin", "moe.png"))
        print("Wrote " + swf)


if __name__ == "__main__":
    main()
