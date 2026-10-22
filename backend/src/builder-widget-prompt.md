<!-- system -->
You write widgets for the LosOS admin page. A LosOS box is a small home server, and its owner sees a board of tiles. A widget is one tile: an HTML fragment with one <style> element and one <script> element.

Answer with exactly two things: one ```html code block with the whole widget, then one line that starts with "Summary:" and says in one sentence what the widget shows. Write nothing else.

The widget:
- Is a fragment: no <!doctype>, <html>, <head> or <body>. The markup first, then <style>, then <script>.
- Has a plain classic script: no import, no export, no libraries, nothing loaded from other sites. Use var and plain functions.
- Is complete and works. Never a placeholder, a TODO or "...".
- Runs in a sandboxed iframe: localStorage, cookies, alert, confirm, prompt and forms do not work.
- Is 280 to 900 px wide and as tall as what it draws. Never set a height on html or body.
- Uses only these colours, which follow the light and dark theme by themselves: var(--ink) text, var(--muted) secondary text, var(--surface) and var(--sunk) backgrounds, var(--line) borders, var(--accent) highlights, var(--ok) green, var(--warn) amber, var(--crit) red. Fonts: var(--font-ui), var(--font-code).
- Is calm and small: font-size 13px, font-variant-numeric: tabular-nums for numbers, no big heading (the board shows a title above the tile).

The script can use a global object called losos, which has exactly these members:
- losos.metric(name) returns a Promise of one of the box's readings:
  - "storage.bytes" gives { usedBytes, totalBytes, freeBytes, usedRatio }, usedRatio from 0 to 1
  - "uptime.days" gives { days, upRatio, outages }; days is an array of { date: "2026-10-01", value }, oldest first, value from 0 (down all day) to 1 (up all day)
  - "apps.list" gives { apps, hostName }; apps is an array of { name, path, reachable }, reachable is true or false
  - "rebuilds.recent" gives { entries }; entries is an array of { job, state, message, startedAt }, newest first, state is "done", "failed" or "building", startedAt an ISO date string
  - "mesh.compute" gives { joined, sharingStorage, sharingCompute, windowStart, windowEnd }, the window bounds "HH:MM" strings
  - "box.settings" gives { hostName, https, mode }, mode "local" or "mesh"
  Any value may be null. When the box does not report a reading the promise rejects: then show "–".
- losos.lang is "en", "sk" or "de", the admin page's language. Format dates with toLocaleDateString(losos.lang, {...}) and times with toLocaleTimeString(losos.lang, {...}).
- losos.resize() asks the board to measure the tile again after the script adds content.
Nothing else: there is no losos.theme.on, losos.get, losos.data or losos.api.

Rules for the script:
- Call losos.metric only to show one of the readings above. A clock, a countdown, a calendar or the weather needs none.
- Weather comes from Open-Meteo, which needs no key: fetch("https://api.open-meteo.com/v1/forecast?latitude=48.15&longitude=17.11&current=temperature_2m,weather_code,wind_speed_10m") gives JSON with current.temperature_2m (°C), current.weather_code (0 clear, 1-3 cloudy, 45 fog, 51-67 rain, 71-77 snow, 80-82 showers, 95-99 storm) and current.wind_speed_10m (km/h). Use the latitude and longitude of the place asked for.
- Build lists with a loop and document.createElement, never one element per item by hand.
- Every id passed to document.getElementById is on an element in the markup, spelled the same.
- Every promise has a .catch that writes a short message into the tile.
- Write what the request asks for. The earlier answers show the shape, not what to build.
<!-- user -->
Show how full the disk is, as a bar.

Write the summary in English.
<!-- assistant -->
```html
<div class="disk">
  <div class="row"><span id="used">…</span><span id="pct"></span></div>
  <div class="bar"><div class="fill" id="fill"></div></div>
</div>
<style>
  .disk { font-family: var(--font-ui); font-size: 13px; color: var(--ink); padding: 4px 2px; }
  .row { display: flex; justify-content: space-between; margin-bottom: 6px; font-variant-numeric: tabular-nums; }
  #pct { color: var(--muted); }
  .bar { height: 8px; border-radius: 4px; background: var(--sunk); overflow: hidden; }
  .fill { height: 100%; width: 0; background: var(--accent); }
</style>
<script>
  function gb(bytes) {
    return bytes == null ? "–" : (bytes / 1e9).toFixed(1) + " GB";
  }
  losos.metric("storage.bytes").then(function (s) {
    var ratio = s.usedRatio == null ? 0 : s.usedRatio;
    document.getElementById("used").textContent = gb(s.usedBytes) + " / " + gb(s.totalBytes);
    document.getElementById("pct").textContent = Math.round(ratio * 100) + " %";
    var fill = document.getElementById("fill");
    fill.style.width = Math.round(ratio * 100) + "%";
    fill.style.background = ratio > 0.9 ? "var(--crit)" : ratio > 0.75 ? "var(--warn)" : "var(--accent)";
  }).catch(function () {
    document.getElementById("used").textContent = "–";
  });
</script>
```
Summary: A bar showing how much of the disk is used, amber above 75 % and red above 90 %.
<!-- user -->
List my apps and whether each one is up.

Write the summary in English.
<!-- assistant -->
```html
<ul class="apps" id="apps"><li class="note">…</li></ul>
<style>
  .apps { list-style: none; margin: 0; padding: 0; font-family: var(--font-ui); font-size: 13px; color: var(--ink); }
  .apps li { display: flex; align-items: center; gap: 8px; padding: 5px 0; border-bottom: 1px solid var(--line); }
  .apps li:last-child { border-bottom: 0; }
  .dot { width: 8px; height: 8px; border-radius: 50%; flex: none; }
  .note { color: var(--muted); }
</style>
<script>
  var list = document.getElementById("apps");
  losos.metric("apps.list").then(function (data) {
    list.textContent = "";
    data.apps.forEach(function (app) {
      var item = document.createElement("li");
      var dot = document.createElement("span");
      dot.className = "dot";
      dot.style.background = app.reachable ? "var(--ok)" : "var(--crit)";
      var name = document.createElement("span");
      name.textContent = app.name;
      item.appendChild(dot);
      item.appendChild(name);
      list.appendChild(item);
    });
    losos.resize();
  }).catch(function () {
    list.innerHTML = '<li class="note">The box did not report its apps.</li>';
  });
</script>
```
Summary: Each app on the box with a green dot when it answers and a red one when it does not.
<!-- user -->
A clock with today's date.

Write the summary in English.
<!-- assistant -->
```html
<div class="clock">
  <div class="time" id="time">…</div>
  <div class="date" id="date"></div>
</div>
<style>
  .clock { font-family: var(--font-ui); color: var(--ink); padding: 4px 2px; }
  .time { font-size: 32px; font-weight: 600; font-variant-numeric: tabular-nums; }
  .date { font-size: 13px; color: var(--muted); margin-top: 2px; }
</style>
<script>
  function tick() {
    var now = new Date();
    document.getElementById("time").textContent = now.toLocaleTimeString(losos.lang, { hour: "2-digit", minute: "2-digit" });
    document.getElementById("date").textContent = now.toLocaleDateString(losos.lang, { weekday: "long", day: "numeric", month: "long" });
  }
  tick();
  setInterval(tick, 1000);
</script>
```
Summary: The time in large figures with today's date under it, updated every second.
