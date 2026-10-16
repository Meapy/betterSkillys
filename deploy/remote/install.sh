#!/bin/bash
# Installs or updates betterSkillys on a Linux server (Ubuntu 24.04), from the bundle that
# deploy/deploy.py unpacked into /opt/bsk/incoming. Runs as root:
#
#     bash /opt/bsk/incoming/deploy/install.sh [--import-data] [--force-data]
#
# - First run: installs Redis 7, the .NET 8 runtime, Caddy and Python, makes the bsk user and
#   /opt/bsk, a swap file if there is none, and the services (bsk-redis, bsk-account, bsk-world,
#   bsk-gateway, caddy).
# - Later runs replace what changed. A new world server build goes in with a rolling deploy
#   (rolling_restart.py request update), which kicks nobody. The account server and the gateway
#   restart only when their files changed (logins pause for a few seconds; browser players are
#   disconnected by a gateway restart).
# - --import-data copies the Redis data in the bundle (redis-data/) into /opt/bsk/redis/data,
#   with the servers stopped. It refuses to replace data already there unless --force-data.
set -euo pipefail

ROOT=/opt/bsk
IN=$ROOT/incoming
IMPORT_DATA=0
FORCE_DATA=0
for a in "$@"; do
    case "$a" in
        --import-data) IMPORT_DATA=1 ;;
        --force-data) FORCE_DATA=1 ;;
        *) echo "unknown option $a" >&2; exit 2 ;;
    esac
done
[ "$(id -u)" = 0 ] || { echo "run as root" >&2; exit 1; }
[ -d "$IN" ] || { echo "no bundle in $IN" >&2; exit 1; }

say() { echo "== $*"; }

# --- packages, user, folders -------------------------------------------------------------
need=()
command -v redis-server >/dev/null || need+=(redis-server)
[ -d /usr/lib/dotnet/shared/Microsoft.AspNetCore.App ] || [ -d /usr/share/dotnet/shared/Microsoft.AspNetCore.App ] || need+=(aspnetcore-runtime-8.0)
command -v caddy >/dev/null || need+=(caddy)
command -v python3 >/dev/null || need+=(python3)
if [ ${#need[@]} -gt 0 ]; then
    say "installing ${need[*]}"
    export DEBIAN_FRONTEND=noninteractive
    apt-get update -q
    apt-get install -y -q "${need[@]}"
fi
# the distro's own Redis would take the port: ours is bsk-redis
if systemctl list-unit-files redis-server.service >/dev/null 2>&1; then
    systemctl disable --now redis-server.service >/dev/null 2>&1 || true
    systemctl mask redis-server.service >/dev/null 2>&1 || true
fi

id bsk >/dev/null 2>&1 || { say "adding the bsk user"; useradd --system --home-dir $ROOT --shell /usr/sbin/nologin bsk; }
mkdir -p $ROOT/redis/data $ROOT/build/live $ROOT/bin $ROOT/dist/client $ROOT/app $ROOT/worldserver

# Two world servers run side by side during a rolling deploy (about 800 MB each): keep some swap
# as a cushion on a small VPS.
if [ -z "$(swapon --noheadings 2>/dev/null)" ] && [ ! -e /swapfile ] && ! grep -qi microsoft /proc/version; then
    say "adding a 2 GB swap file"
    fallocate -l 2G /swapfile && chmod 600 /swapfile && mkswap /swapfile >/dev/null && swapon /swapfile
    grep -q '^/swapfile ' /etc/fstab || echo '/swapfile none swap sw 0 0' >> /etc/fstab
fi

# Redis forks to save: without overcommit a save can fail when memory is short
if [ "$(sysctl -n vm.overcommit_memory 2>/dev/null)" != 1 ]; then
    echo 'vm.overcommit_memory = 1' > /etc/sysctl.d/90-bsk-redis.conf
    sysctl -q -p /etc/sysctl.d/90-bsk-redis.conf || true
fi

# --- configuration and tools (always safe to replace) --------------------------------------
units_changed=0  # the units or their settings: restart what doesn't kick players
cmp -s $IN/bsk.env $ROOT/bsk.env || { install -m 644 $IN/bsk.env $ROOT/bsk.env; units_changed=1; }
rm -rf $ROOT/deploy && cp -r $IN/deploy $ROOT/deploy
rm -rf $ROOT/tools && cp -r $IN/tools $ROOT/tools
for u in $IN/deploy/systemd/*.service; do
    dest=/etc/systemd/system/$(basename "$u")
    cmp -s "$u" "$dest" || { install -m 644 "$u" "$dest"; units_changed=1; }
done
[ $units_changed = 1 ] && systemctl daemon-reload

# --- Redis --------------------------------------------------------------------------------
if [ $IMPORT_DATA = 1 ]; then
    [ -d $IN/redis-data ] || { echo "no redis-data/ in the bundle" >&2; exit 1; }
    if [ -n "$(ls -A $ROOT/redis/data)" ] && [ $FORCE_DATA = 0 ]; then
        echo "$ROOT/redis/data already has data: add --force-data to replace it" >&2
        exit 1
    fi
    say "importing the Redis data (servers stopped meanwhile)"
    systemctl stop bsk-world bsk-account bsk-redis 2>/dev/null || true
    if [ -n "$(ls -A $ROOT/redis/data)" ]; then
        mv $ROOT/redis/data "$ROOT/redis/data.before-import.$(date +%Y%m%d-%H%M%S)"
        mkdir -p $ROOT/redis/data
    fi
    cp -r $IN/redis-data/. $ROOT/redis/data/
fi
chown -R bsk:bsk $ROOT/redis
systemctl enable --quiet bsk-redis
systemctl start bsk-redis

# --- account server -----------------------------------------------------------------------
if ! diff -rq --exclude=logs $IN/app $ROOT/app >/dev/null 2>&1; then
    say "account server changed: installing it"
    systemctl stop bsk-account 2>/dev/null || true
    find $ROOT/app -mindepth 1 -maxdepth 1 ! -name logs -exec rm -rf {} +
    cp -r $IN/app/. $ROOT/app/
fi
if [ -d $IN/client ]; then
    # published client releases, served at /client/ for players' clients to update from
    rm -rf $ROOT/dist/client.new && cp -r $IN/client $ROOT/dist/client.new
    rm -rf $ROOT/dist/client.old && mv $ROOT/dist/client $ROOT/dist/client.old && mv $ROOT/dist/client.new $ROOT/dist/client
    rm -rf $ROOT/dist/client.old
fi

# --- browser client and gateway -----------------------------------------------------------
rm -rf $ROOT/web.new && cp -r $IN/web $ROOT/web.new
[ -d $ROOT/web ] && mv $ROOT/web $ROOT/web.old
mv $ROOT/web.new $ROOT/web && rm -rf $ROOT/web.old
if [ -d $IN/site ]; then
    # the website, which Caddy serves at the site's root
    rm -rf $ROOT/site.new && cp -r $IN/site $ROOT/site.new
    [ -d $ROOT/site ] && mv $ROOT/site $ROOT/site.old
    mv $ROOT/site.new $ROOT/site && rm -rf $ROOT/site.old
fi
gateway_changed=0
if ! cmp -s $IN/bin/bsk-gateway $ROOT/bin/bsk-gateway; then
    install -m 755 $IN/bin/bsk-gateway $ROOT/bin/bsk-gateway.new
    mv $ROOT/bin/bsk-gateway.new $ROOT/bin/bsk-gateway
    gateway_changed=1
fi

# --- world server build (the service copies it into a slot) --------------------------------
world_changed=0
if ! diff -rq $IN/worldserver $ROOT/worldserver >/dev/null 2>&1; then
    world_changed=1
    rm -rf $ROOT/worldserver.new && cp -r $IN/worldserver $ROOT/worldserver.new
    rm -rf $ROOT/worldserver.old && mv $ROOT/worldserver $ROOT/worldserver.old && mv $ROOT/worldserver.new $ROOT/worldserver
    rm -rf $ROOT/worldserver.old
fi

chown -R bsk:bsk $ROOT
chown root:root $ROOT/bsk.env

# --- (re)start ----------------------------------------------------------------------------
systemctl enable --quiet bsk-account bsk-world bsk-gateway
systemctl is-active --quiet bsk-account && [ $units_changed = 0 ] || systemctl restart bsk-account
# restart only what changed: a world server change is a rolling deploy, kicking nobody
if systemctl is-active --quiet bsk-world; then
    if [ $world_changed = 1 ]; then
        say "rolling deploy of the new world server (players switch at their next Nexus trip)"
        runuser -u bsk -- /usr/bin/python3 $ROOT/tools/rolling_restart.py request update
    else
        say "world server unchanged"
    fi
else
    say "starting the world servers"
    systemctl start bsk-world
fi
if [ $gateway_changed = 1 ] || [ $units_changed = 1 ] || ! systemctl is-active --quiet bsk-gateway; then
    systemctl restart bsk-gateway
fi

if [ -f $IN/Caddyfile ]; then
    if ! cmp -s $IN/Caddyfile /etc/caddy/Caddyfile; then
        install -m 644 $IN/Caddyfile /etc/caddy/Caddyfile
        systemctl enable --quiet caddy
        systemctl reload-or-restart caddy
    fi
    systemctl is-active --quiet caddy || systemctl start caddy
fi

say "done"
systemctl --no-pager --lines=0 status bsk-redis bsk-account bsk-world bsk-gateway caddy | grep -E '^(●|  *Active)' || true
