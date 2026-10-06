#!/usr/bin/env bash
set -euo pipefail

reaper="${REAPER:-/Applications/REAPER.app/Contents/MacOS/REAPER}"
here="$(cd "$(dirname "$0")" && pwd)"
samples="$here/samples"
out="$here/render"
mkdir -p "$samples" "$out/stems"

noise='(2*random(0)-1)'
wide='(2*random(1)-1)'
drum() {
  ffmpeg -hide_banner -loglevel error -y -f lavfi -i "aevalsrc=$2:s=48000:d=$3" -af "${4:-anull}" -c:a pcm_s24le "$samples/$1.wav"
}
tom() { drum "$1" "sin(2*PI*($2*t+$(($2 * 2))/20*(1-exp(-20*t))))*exp(-8*t)" 0.45; }
drum kick 'sin(2*PI*(50*t+6*(1-exp(-25*t))))*exp(-5*t)' 0.45
drum stick "0.6*sin(2*PI*1700*t)*exp(-80*t)+0.5*$noise*exp(-120*t)" 0.08
drum snare "0.5*sin(2*PI*190*t)*exp(-25*t)+0.6*$noise*exp(-14*t)|0.5*sin(2*PI*190*t)*exp(-25*t)+0.6*$wide*exp(-14*t)" 0.3 'highpass=f=150'
drum clap "$noise*exp(-15*t)|$wide*exp(-15*t)" 0.25 'bandpass=f=1200:w=900'
drum hat-closed "$noise*exp(-45*t)|$wide*exp(-45*t)" 0.1 'highpass=f=7000'
drum hat-open "$noise*exp(-7*t)|$wide*exp(-7*t)" 0.5 'highpass=f=7000'
drum crash "$noise*exp(-2.5*t)|$wide*exp(-2.5*t)" 2 'highpass=f=4000'
tom tom-low 90
tom tom-mid 130
tom tom-high 180

if pgrep -x REAPER >/dev/null; then
  echo "REAPER is already running; close it before mixing" >&2
  exit 1
fi
reaper_pid=
trap '[ -z "$reaper_pid" ] || kill "$reaper_pid" 2>/dev/null || true' EXIT
run_reaper() {
  rm -f "$out/mix-done.txt"
  "$reaper" -nosplash -ignoreerrors -new >/dev/null 2>&1 &
  reaper_pid=$!
  sleep 10
  "$reaper" -nonewinst "$here/mix.lua" >/dev/null
  for _ in $(seq 900); do [ -f "$out/mix-done.txt" ] && break; sleep 1; done
  kill "$reaper_pid" 2>/dev/null || true
  wait "$reaper_pid" 2>/dev/null || true
  reaper_pid=
  grep -qx ok "$out/mix-done.txt" || { cat "$out/mix-done.txt" >&2 2>/dev/null || echo "mix.lua did not finish" >&2; exit 1; }
}
lufs() {
  ffmpeg -nostdin -hide_banner -nostats -i "$1" -af ebur128=peak=true:framelog=quiet -f null - 2>&1 | awk '/I:/ { v = $2 } END { print v }'
}
setting() { awk -F'\t' -v k="$1" '$1 == k { print $2 }' "$here/master.tsv"; }

rm -f "$out"/stems/*.wav
printf 'pass\tstems\n' > "$out/pass.tsv"
run_reaper
vocal="$(lufs "$out/stems/Vocal.wav")"
while IFS=$'\t' read -r track target; do
  stem="$(lufs "$out/stems/$track.wav")"
  printf '%s\t%s\n' "$track" "$(awk -v t="$target" -v v="$vocal" -v s="$stem" 'BEGIN { printf "%.2f", t + v - s }')"
done < "$here/levels.tsv" | tee "$out/gains.tsv"

target="$(setting lufs)"
threshold=-6
for round in 1 2 3 4; do
  rm -f "$out/mou-ikkai-mix.wav"
  printf 'pass\tmix\nthreshold\t%s\nlow_shelf\t%s\nhigh_shelf\t%s\n' "$threshold" "$(setting low_shelf)" "$(setting high_shelf)" > "$out/pass.tsv"
  run_reaper
  measured="$(lufs "$out/mou-ikkai-mix.wav")"
  echo "round $round: threshold $threshold -> $measured LUFS"
  awk -v m="$measured" -v t="$target" 'BEGIN { exit !(m - t < 0.25 && t - m < 0.25) }' && break
  threshold="$(awk -v th="$threshold" -v m="$measured" -v t="$target" 'BEGIN { printf "%.2f", th - (t - m) }')"
done
cp "$out/pass.tsv" "$out/mou-ikkai-mix.settings.tsv"
