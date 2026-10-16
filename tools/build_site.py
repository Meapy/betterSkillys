"""Builds the website players land on (website/) into build/site: the home page with the server
status, the downloads and the newest changes, and the full changelog page.

    python tools/build_site.py [--name "Do Damage"] [--out build/site]

- {{NAME}}, {{VERSION}} and {{BUILD}} in the pages are filled in; {{CHANGELOG_LATEST}} and
  {{CHANGELOG_ALL}} come from client-rs/CHANGELOG.md (its Unreleased section is left out).
- {{DOWNLOADS}} lists the desktop client zips in dist/ (Do-Damage-<os>.zip, from
  tools/build_client_rs.py --package), which are copied into downloads/. Build them with the
  account server the site's players use (--account http://<public address>:8080).
- Everything else in website/ (style.css, site.js, favicon.svg) is copied as it is. The pictures
  are pixel sprites drawn by site.js, so there are no image files to make.

On the VPS, Caddy serves the result at the site's root, with the browser client at /play/ and the
gateway's /api and /ws beside it (deploy/deploy.py writes the Caddyfile).
"""
import argparse
import hashlib
import html
import os
import re
import shutil

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
SRC = os.path.join(ROOT, "website")
CHANGELOG = os.path.join(ROOT, "client-rs", "CHANGELOG.md")
DOWNLOADS = [("windows", "Windows"), ("linux", "Linux"), ("macos", "macOS")]
LATEST = 2


def releases():
    """[(version, date, [item, ...])] from the changelog, newest first, without Unreleased."""
    with open(CHANGELOG, encoding="utf-8") as f:
        text = f.read()
    out = []
    for block in re.split(r"^## ", text, flags=re.M)[1:]:
        head, _, body = block.partition("\n")
        m = re.match(r"([\d.]+)\s*(?:\((.*?)\))?", head.strip())
        if not m:
            continue  # Unreleased
        items = []
        for line in body.splitlines():
            if line.startswith("- "):
                items.append(line[2:].strip())
            elif line.startswith("  ") and items:
                items[-1] += " " + line.strip()
        out.append((m.group(1), m.group(2) or "", items))
    return out


def inline(text):
    """The changelog's bits of markdown: `code` and **bold**."""
    text = html.escape(text, quote=False)
    text = re.sub(r"`([^`]+)`", r"<code>\1</code>", text)
    return re.sub(r"\*\*([^*]+)\*\*", r"<strong>\1</strong>", text)


def releases_html(rel):
    parts = []
    for version, date, items in rel:
        when = '<time datetime="%s">%s</time>' % (date, date) if date else ""
        lis = "\n".join("      <li>%s</li>" % inline(i) for i in items)
        parts.append('<article class="release" id="v%s">\n  <h3>%s %s</h3>\n  <ul>\n%s\n  </ul>\n</article>'
                     % (version, version, when, lis))
    return "\n".join(parts)


def downloads(out):
    found = []
    for key, label in DOWNLOADS:
        src = os.path.join(ROOT, "dist", "Do-Damage-%s.zip" % key)
        if not os.path.isfile(src):
            continue
        os.makedirs(os.path.join(out, "downloads"), exist_ok=True)
        shutil.copy2(src, os.path.join(out, "downloads", os.path.basename(src)))
        size = os.path.getsize(src) / 1e6
        found.append('<a class="dl" data-os="%s" data-label="%s" href="downloads/%s" download>'
                     '<span><span class="dl-os">%s</span><br><span class="dl-meta">%.1f MB · zip</span></span>'
                     '<span class="dl-arrow">Download ↓</span></a>'
                     % (key, label, os.path.basename(src), label, size))
    if not found:
        return '<p class="muted">Desktop downloads are coming soon. Until then, play in your browser.</p>'
    return "\n".join(found)


def main():
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("--name", default="Do Damage", help="the game's name on the site")
    ap.add_argument("--out", default=os.path.join(ROOT, "build", "site"))
    args = ap.parse_args()

    out = os.path.abspath(args.out)
    if os.path.isdir(out):
        shutil.rmtree(out)
    os.makedirs(out)

    rel = releases()
    version = rel[0][0] if rel else ""
    dl = downloads(out)

    # ?v= on the stylesheet and script: a new build is never mixed with cached files
    h = hashlib.sha1()
    for name in sorted(os.listdir(SRC)):
        with open(os.path.join(SRC, name), "rb") as f:
            h.update(f.read())
    h.update(version.encode())
    values = {
        "NAME": html.escape(args.name),
        "VERSION": version,
        "BUILD": h.hexdigest()[:10],
        "DOWNLOADS": dl,
        "CHANGELOG_LATEST": releases_html(rel[:LATEST]),
        "CHANGELOG_ALL": releases_html(rel),
    }
    for name in os.listdir(SRC):
        src = os.path.join(SRC, name)
        if name.endswith(".html"):
            with open(src, encoding="utf-8") as f:
                text = f.read()
            text = re.sub(r"\{\{(\w+)\}\}", lambda m: values[m.group(1)], text)
            with open(os.path.join(out, name), "w", encoding="utf-8", newline="\n") as f:
                f.write(text)
        else:
            shutil.copy2(src, os.path.join(out, name))
    print("site: %s (version %s, %d releases)" % (os.path.relpath(out, ROOT), version, len(rel)))


if __name__ == "__main__":
    main()
