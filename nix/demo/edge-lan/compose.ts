#!/usr/bin/env bun
// Compose recorded edge-lan pictures into a slideshow video. Shim target.
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const rec = Bun.argv[2];
if (!rec) { console.error("usage: compose.ts RECDIR [OUT.mp4] [SECONDS_PER_STEP]"); process.exit(2); }
const recDir = resolve(rec);
const out = resolve(Bun.argv[3] || join(recDir, "edge-lan-two-boxes.mp4"));
const secs = Bun.argv[4] || "7";
const fontRes = spawnSync("fc-match", ["-f", "%{file}", "DejaVu Sans"], { encoding: "utf8" });
const font = Bun.env.LOSOS_FONT || (fontRes.stdout?.trim() || "/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf");
const work = mkdtempSync(join(tmpdir(), "edge-lan-compose-"));
process.on("exit", () => rmSync(work, { recursive: true, force: true }));
const W = 960, H = 675, BAND = 170;
const wrap = (s: string) => spawnSync("fold", ["-s", "-w", "110"], { input: s, encoding: "utf8" }).stdout.replace(/[ \t]+$/gm, "").trimEnd();
const ff = (args: string[]) => { const p = spawnSync("ffmpeg", args, { stdio: "inherit" }); if (p.status !== 0) process.exit(p.status ?? 1); };

let list = "";
writeFileSync(join(work, "title.txt"), "LosOS: two boxes and an edge proxy on one network");
writeFileSync(join(work, "sub.txt"), "Scenario A: no edge proxy anywhere. Scenario B: the edge appears. Recorded from the boxes' own admin UI.");
ff(["-nostdin", "-v", "error", "-y", "-f", "lavfi", "-i", `color=c=0x13203a:s=${W * 2}x${H + BAND}:d=4`,
  "-vf", `drawtext=fontfile=${font}:fontsize=46:fontcolor=white:x=(w-text_w)/2:y=(h/2)-70:expansion=none:textfile=${work}/title.txt,drawtext=fontfile=${font}:fontsize=26:fontcolor=0xdfe7f5:x=(w-text_w)/2:y=(h/2)+10:expansion=none:textfile=${work}/sub.txt`,
  "-r", "25", "-pix_fmt", "yuv420p", `${work}/00.mp4`]);
list += `file '${work}/00.mp4'\n`;

let n = 0;
for (const line of readFileSync(join(recDir, "shots.jsonl"), "utf8").split("\n").filter(Boolean)) {
  n++;
  const ev = JSON.parse(line);
  const f1 = ev.files?.[0], f2 = ev.files?.[1];
  if (!f1 || !f2) { console.error(`step ${ev.step} has no pictures; skipped`); n--; continue; }
  const nn = String(n).padStart(2, "0");
  writeFileSync(join(work, `cap-${n}.txt`), wrap(`${ev.step}  —  ${ev.caption}`));
  ff(["-nostdin", "-v", "error", "-y", "-loop", "1", "-t", secs, "-i", f1, "-loop", "1", "-t", secs, "-i", f2,
    "-filter_complex", `[0:v]scale=${W}:${H}[a];[1:v]scale=${W}:${H}[b];[a][b]hstack=inputs=2[row];[row]pad=iw:ih+${BAND}:0:0:color=0x13203a[padded];[padded]drawtext=fontfile=${font}:fontsize=28:fontcolor=white:line_spacing=8:x=40:y=${H}+30:expansion=none:textfile=${work}/cap-${n}.txt[v]`,
    "-map", "[v]", "-r", "25", "-pix_fmt", "yuv420p", `${work}/${nn}.mp4`]);
  list += `file '${work}/${nn}.mp4'\n`;
}
writeFileSync(join(work, "list.txt"), list);
ff(["-nostdin", "-v", "error", "-y", "-f", "concat", "-safe", "0", "-i", `${work}/list.txt`, "-c", "copy", out]);
console.log(`wrote ${out} (${n} steps, ${secs}s each)`);
