"""Test client: build/test/client.swf and build/test/test-app.xml.

    python tools/testclient/build_test_client.py [--port 8081] [--release]

- Its own AIR app id (betterSkillysPortTest) and window title ("betterSkillys port test"), so it
  never shares settings or a window with the real client.
- Signs in to the throwaway account in test-account.local.txt ("email: ..." and "password: ..."
  lines) when no login is saved.
- TestHook.as: commands from <app storage>/autochat.txt, uncaught errors to errors.txt.
- --port: the account server port (8081 for the isolated stack, see iso_stack.py).
- --release: no debug info. Use it for timing (debug builds run noticeably slower); the
  allocation sampler ("#sample N") only works in a debug build.
"""
import argparse
import os
import re
import shutil
import subprocess
import sys

HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.dirname(os.path.dirname(HERE))
sys.path.insert(0, os.path.join(ROOT, "tools"))
import build_client  # noqa: E402

APP_ID = "betterSkillysPortTest"
TITLE = "betterSkillys port test"


def edit(path, fn):
    t = open(path, encoding="utf-8-sig").read()
    t = fn(t)
    open(path, "w", encoding="utf-8").write(t)


def insert_after(t, anchor, text):
    assert t.count(anchor) == 1, anchor
    return t.replace(anchor, anchor + text, 1)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--port", type=int, default=8080)
    ap.add_argument("--release", action="store_true")
    args = ap.parse_args()

    creds = open(os.path.join(ROOT, "test-account.local.txt")).read()
    email = re.search(r"email: (\S+)", creds).group(1)
    password = re.search(r"password: (\S+)", creds).group(1)

    src = os.path.join(ROOT, "build", "testsrc")
    if os.path.isdir(src):
        shutil.rmtree(src)
    shutil.copytree(os.path.join(ROOT, "client", "src"), src)
    game = os.path.join(src, "com", "company", "assembleegameclient", "game")
    shutil.copyfile(os.path.join(HERE, "TestHook.as"), os.path.join(game, "TestHook.as"))

    edit(os.path.join(src, "kabam", "rotmg", "account", "web", "services", "WebLoadAccountTask.as"),
         lambda t: insert_after(t, '            rotmg.data["Password"] && (this.data.password = rotmg.data["Password"]);',
                                '\n            if (!this.data.username) { this.data.username = "%s"; this.data.password = "%s"; }' % (email, password)))

    def set_port(t):
        t, n = re.subn(r'(LOCALHOST:String = "http://[^":/]+:)\d+"', r'\g<1>%d"' % args.port, t)
        assert n == 1
        return t
    edit(os.path.join(src, "kabam", "rotmg", "application", "impl", "LocalhostSetup.as"), set_port)

    def web_main(t):
        t = insert_after(t, "    public function WebMain() {",
                         '\n        loaderInfo.uncaughtErrorEvents.addEventListener("uncaughtError", TestHook.onError);')
        return t.replace("import ", "import com.company.assembleegameclient.game.TestHook;\nimport ", 1)
    edit(os.path.join(src, "WebMain.as"), web_main)

    def game_sprite(t):
        t = insert_after(t, "      var dt:int = time - this.lastUpdate_;",
                         "\n      var testLine:String = TestHook.next();"
                         "\n      if (testLine != null && !TestHook.local(testLine, this)) this.gsc_.playerText(testLine);"
                         "\n      TestHook.tick();"
                         "\n      if (TestHook.skip == 1) { this.lastUpdate_ = time; return; }")
        m = re.search(r"public function connect\(\) : void\s*\{", t)
        assert m
        t = t[:m.end()] + "\n      TestHook.reset();" + t[m.end():]
        a = "this.map.draw(this.camera_,time);"
        assert t.count(a) == 1
        return t.replace(a, "if (TestHook.skip != 2) " + a)
    edit(os.path.join(game, "GameSprite.as"), game_sprite)

    def renderer(t):
        a = "this.context3D.present();"
        assert t.count(a) == 1
        t = t.replace(a, "if (TestHook.skip != 3) " + a)
        return t.replace("import ", "import com.company.assembleegameclient.game.TestHook;\n   import ", 1)
    edit(os.path.join(src, "kabam", "rotmg", "stage3D", "Renderer.as"), renderer)

    out = os.path.join(ROOT, "build", "test")
    os.makedirs(out, exist_ok=True)
    subprocess.check_call([build_client.tool("amxmlc.bat"), "-source-path=" + src,
                           "-library-path+=" + os.path.join(ROOT, "client", "libs"),
                           "-output=" + os.path.join(out, "client.swf"),
                           "-debug=" + ("false" if args.release else "true")]
                          + build_client.COMPILER_ARGS + [os.path.join(src, "WebMain.as")],
                          stdout=subprocess.DEVNULL)

    xml = open(os.path.join(ROOT, "client", "WebMain-app.xml"), encoding="utf-8-sig").read()
    xml = re.sub(r"<id>[^<]*</id>", "<id>%s</id>" % APP_ID, xml, 1)
    xml = re.sub(r"<title>[^<]*</title>", "<title>%s</title>" % TITLE, xml, 1)
    xml = re.sub(r"\s*<icon>.*?</icon>", "", xml, flags=re.S)
    open(os.path.join(out, "test-app.xml"), "w", encoding="utf-8").write(xml)
    print("built", os.path.join(out, "client.swf"), "(release)" if args.release else "(debug)", "for port", args.port)


if __name__ == "__main__":
    main()
