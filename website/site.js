// The home page's live parts: the server status from the account server's server list (through
// the gateway's /api), the visitor's own download first, and a warning when the browser can't
// run the game.
(function () {
  "use strict";

  // --- Server status ------------------------------------------------------------------------
  // POST api/app/serverList answers <Servers><Servers><Server>... with UsageText "3/85" (players
  // and room), or "- NEW!" for a server that just started; <Error> when none are up.
  var status = document.getElementById("status");
  var statusText = document.getElementById("status-text");

  function parseServers(xml) {
    var doc = new DOMParser().parseFromString(xml, "application/xml");
    var servers = [];
    var nodes = doc.getElementsByTagName("Server");
    for (var i = 0; i < nodes.length; i++) {
      var get = function (tag) {
        var el = nodes[i].getElementsByTagName(tag)[0];
        return el ? el.textContent : "";
      };
      if (/^true$/i.test(get("AdminOnly"))) continue;
      var m = /^(\d+)\s*\/\s*(\d+)$/.exec(get("UsageText"));
      servers.push({ name: get("Name"), players: m ? +m[1] : 0 });
    }
    return servers;
  }

  function showStatus(servers) {
    if (!status) return;
    if (servers === null) {
      statusText.textContent = "Server status unavailable";
      return;
    }
    if (servers.length === 0) {
      status.classList.add("offline");
      statusText.textContent = "The server is down for a moment";
      return;
    }
    var players = servers.reduce(function (n, s) { return n + s.players; }, 0);
    status.classList.add("online");
    statusText.textContent = "Online · " + players + (players === 1 ? " player" : " players") + " in the realm";
  }

  function refreshStatus() {
    fetch("api/app/serverList", { method: "POST", headers: { "Content-Type": "application/x-www-form-urlencoded" }, body: "" })
      .then(function (r) { return r.ok ? r.text() : Promise.reject(r.status); })
      .then(function (text) { showStatus(parseServers(text)); })
      .catch(function () { showStatus(null); });
  }

  if (status) {
    refreshStatus();
    setInterval(function () {
      if (!document.hidden) refreshStatus();
    }, 60000);
  }

  // --- Downloads: the visitor's system first --------------------------------------------------
  var ua = navigator.userAgent;
  var os = /Windows/i.test(ua) ? "windows" : /Mac OS X|Macintosh/i.test(ua) ? "macos" : /Linux|X11/i.test(ua) && !/Android/i.test(ua) ? "linux" : "";
  var list = document.getElementById("dl-list");
  if (list && os) {
    var mine = list.querySelector('[data-os="' + os + '"]');
    if (mine) {
      mine.classList.add("mine");
      list.insertBefore(mine, list.firstChild);
      var main = document.getElementById("download-main");
      if (main) main.textContent = "Download for " + mine.getAttribute("data-label");
    }
  }

  // --- Can this browser run the game? -------------------------------------------------------
  var note = document.getElementById("play-note");
  if (note) {
    var touchOnly = matchMedia("(pointer: coarse)").matches && !matchMedia("(any-pointer: fine)").matches;
    var webgl2 = false;
    try { webgl2 = !!document.createElement("canvas").getContext("webgl2"); } catch (e) { /* none */ }
    if (!webgl2) {
      note.textContent = "Your browser doesn't have WebGL 2, which the game needs. Update it, turn on hardware acceleration, or download the client.";
      note.classList.add("warn");
    } else if (touchOnly) {
      note.textContent = "The game needs a keyboard and mouse: open this page on a computer to play.";
      note.classList.add("warn");
    }
  }

  // For tests: window.bskSite.parseServers(xml)
  window.bskSite = { parseServers: parseServers, showStatus: showStatus };
})();
