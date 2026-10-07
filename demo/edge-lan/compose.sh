#!/usr/bin/env bash
# demo/edge-lan/compose.sh — the pictures record.sh took, as one video.
#
#   demo/edge-lan/compose.sh RECDIR [OUT.mp4] [SECONDS_PER_STEP]
#
# Reads RECDIR/shots.jsonl (one line per step: the caption and the two
# pictures), lays box1 and box2 side by side under the step's caption, and
# concatenates the steps into a slideshow. ffmpeg with drawtext is the only
# requirement (nix shell nixpkgs#ffmpeg-full works). The video is a project
# artefact, not repository content: keep it beside the recordings, link it
# from Markdown.
set -euo pipefail

rec=${1:?RECDIR}
out=${2:-$rec/edge-lan-two-boxes.mp4}
secs=${3:-7}
font=${LOSOS_FONT:-$(fc-match -f '%{file}' 'DejaVu Sans' 2>/dev/null || echo /usr/share/fonts/truetype/dejavu/DejaVuSans.ttf)}
work=$(mktemp -d)
trap 'rm -rf "$work"' EXIT

W=960 H=675 BAND=170
# Captions go through drawtext's textfile= rather than text=, so no
# character in a caption needs escaping (colons, quotes and percent signs
# all have meanings on a filter line).
wrap() { fold -s -w 110 | sed -e 's/[[:space:]]*$//'; }

n=0
: >"$work/list.txt"
# A title card.
title="LosOS: two boxes and an edge proxy on one network"
sub="Scenario A: no edge proxy anywhere. Scenario B: the edge appears. Recorded from the boxes' own admin UI."
printf '%s' "$title" >"$work/title.txt"
printf '%s' "$sub" >"$work/sub.txt"
ffmpeg -nostdin -v error -y -f lavfi -i "color=c=0x13203a:s=$((W * 2))x$((H + BAND)):d=4" \
  -vf "drawtext=fontfile=${font}:fontsize=46:fontcolor=white:x=(w-text_w)/2:y=(h/2)-70:expansion=none:textfile=${work}/title.txt,drawtext=fontfile=${font}:fontsize=26:fontcolor=0xdfe7f5:x=(w-text_w)/2:y=(h/2)+10:expansion=none:textfile=${work}/sub.txt" \
  -r 25 -pix_fmt yuv420p "$work/00.mp4"
echo "file '$work/00.mp4'" >>"$work/list.txt"

while IFS= read -r line; do
  n=$((n + 1))
  step=$(jq -r .step <<<"$line")
  caption=$(jq -r .caption <<<"$line")
  f1=$(jq -r '.files[0] // empty' <<<"$line")
  f2=$(jq -r '.files[1] // empty' <<<"$line")
  [ -n "$f1" ] && [ -n "$f2" ] || { echo "step $step has no pictures; skipped" >&2; continue; }
  printf '%s  —  %s' "$step" "$caption" | wrap >"$work/cap-$n.txt"
  ffmpeg -nostdin -v error -y -loop 1 -t "$secs" -i "$f1" -loop 1 -t "$secs" -i "$f2" \
    -filter_complex "[0:v]scale=${W}:${H}[a];[1:v]scale=${W}:${H}[b];[a][b]hstack=inputs=2[row];[row]pad=iw:ih+${BAND}:0:0:color=0x13203a[padded];[padded]drawtext=fontfile=${font}:fontsize=28:fontcolor=white:line_spacing=8:x=40:y=${H}+30:expansion=none:textfile=${work}/cap-${n}.txt[v]" \
    -map '[v]' -r 25 -pix_fmt yuv420p "$work/$(printf %02d "$n").mp4"
  echo "file '$work/$(printf %02d "$n").mp4'" >>"$work/list.txt"
done <"$rec/shots.jsonl"

ffmpeg -nostdin -v error -y -f concat -safe 0 -i "$work/list.txt" -c copy "$out"
echo "wrote $out ($n steps, ${secs}s each)"
