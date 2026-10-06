#!/usr/bin/env bash
set -euo pipefail

reaper="${REAPER:-/Applications/REAPER.app/Contents/MacOS/REAPER}"
here="$(cd "$(dirname "$0")" && pwd)"
out="$here/render/audition"
metrics="$here/../../../tools/mix-metrics"
mkdir -p "$out"
rm -f "$here/render/audition-done.txt" "$out"/*.wav

pkill -x REAPER || true
"$reaper" -nosplash -ignoreerrors -new >/dev/null 2>&1 &
sleep 10
"$reaper" -nonewinst "$here/audition.lua" >/dev/null
for _ in $(seq 1800); do [ -f "$here/render/audition-done.txt" ] && break; sleep 1; done
pkill -x REAPER || true
grep -qx ok "$here/render/audition-done.txt" || { cat "$here/render/audition-done.txt" >&2 2>/dev/null || echo "audition.lua did not finish" >&2; exit 1; }
for wav in "$out"/*.wav; do
  printf '%s\t%s\n' "$(basename "$wav" .wav)" "$(uv run -q --project "$metrics" "$metrics/mix_metrics.py" "$wav")"
done | tee "$out/metrics.tsv"
