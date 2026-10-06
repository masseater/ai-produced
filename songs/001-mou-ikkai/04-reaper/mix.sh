#!/usr/bin/env bash
set -euo pipefail

reaper="${REAPER:-/Applications/REAPER.app/Contents/MacOS/REAPER}"
here="$(cd "$(dirname "$0")" && pwd)"
samples="$here/samples"
out="$here/render"
mix="$here/mou-ikkai-mix.rpp"
mkdir -p "$samples" "$out"

noise='(2*random(0)-1)'
drum() {
  ffmpeg -hide_banner -loglevel error -y -f lavfi -i "aevalsrc=$2:s=48000:d=$3" -af "${4:-anull}" -ac 1 -c:a pcm_s24le "$samples/$1.wav"
}
tom() { drum "$1" "sin(2*PI*($2*t+$(($2 * 2))/20*(1-exp(-20*t))))*exp(-8*t)" 0.45; }
drum kick 'sin(2*PI*(50*t+6*(1-exp(-25*t))))*exp(-5*t)' 0.45
drum stick "0.6*sin(2*PI*1700*t)*exp(-80*t)+0.5*$noise*exp(-120*t)" 0.08
drum snare "0.5*sin(2*PI*190*t)*exp(-25*t)+0.6*$noise*exp(-14*t)" 0.3 'highpass=f=150'
drum clap "$noise*exp(-15*t)" 0.25 'bandpass=f=1200:w=900'
drum hat-closed "$noise*exp(-45*t)" 0.1 'highpass=f=7000'
drum hat-open "$noise*exp(-7*t)" 0.5 'highpass=f=7000'
drum crash "$noise*exp(-2.5*t)" 2 'highpass=f=4000'
tom tom-low 90
tom tom-mid 130
tom tom-high 180

if pgrep -x REAPER >/dev/null; then
  echo "REAPER is already running; close it before mixing" >&2
  exit 1
fi
rm -f "$mix" "$out/mix-done.txt" "$out/mou-ikkai-mix.wav"
"$reaper" -nosplash -ignoreerrors -new >/dev/null 2>&1 &
reaper_pid=$!
trap 'kill "$reaper_pid" 2>/dev/null || true' EXIT
sleep 10
"$reaper" -nonewinst "$here/mix.lua" >/dev/null
for _ in $(seq 600); do [ -f "$out/mix-done.txt" ] && break; sleep 1; done
kill "$reaper_pid" 2>/dev/null || true
grep -qx ok "$out/mix-done.txt" || { cat "$out/mix-done.txt" >&2 2>/dev/null || echo "mix.lua did not finish" >&2; exit 1; }
wav="$out/mou-ikkai-mix.wav"
{
  ffmpeg -hide_banner -nostats -i "$wav" -af ebur128=peak=true:framelog=quiet -f null - 2>&1 | sed -n '/Summary/,$p'
  ffmpeg -hide_banner -nostats -i "$wav" -af astats=measure_perchannel=none:measure_overall=Peak_level+RMS_level -f null - 2>&1 | grep -E 'Peak level|RMS level'
} | tee "$out/loudness.txt"
