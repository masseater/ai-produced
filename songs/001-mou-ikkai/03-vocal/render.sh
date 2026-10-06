#!/usr/bin/env bash
set -euo pipefail

OPENUTAU_COMMIT=799ba8de27c19fb305b1d3c989f1823ad4a8e605
SINGER_PATTERN="${SINGER_PATTERN:-Nishiren}"
SINGER_TYPE="${SINGER_TYPE:-DiffSinger}"
PITCH="${PITCH-1}"

song="$(cd "$(dirname "$0")/.." && pwd)"
repo="$(cd "$song/../.." && pwd)"
tool="$repo/tools/openutau-render"
source_dir="$HOME/.cache/ai-produced/openutau"
out="$song/03-vocal/render"
mkdir -p "$out"

if ! command -v dotnet >/dev/null || ! dotnet --list-sdks | grep -q '^10\.'; then
  curl -sSL https://builds.dotnet.microsoft.com/dotnet/scripts/v1/dotnet-install.sh | bash -s -- --channel 10.0 --install-dir "$HOME/.dotnet"
  export DOTNET_ROOT="$HOME/.dotnet" PATH="$HOME/.dotnet:$PATH"
fi
export DOTNET_CLI_TELEMETRY_OPTOUT=1 DOTNET_NOLOGO=1

if [ ! -d "$source_dir" ]; then
  git clone --filter=blob:none https://github.com/stakira/OpenUtau.git "$source_dir"
fi
git -C "$source_dir" fetch --quiet origin "$OPENUTAU_COMMIT"
git -C "$source_dir" checkout --quiet "$OPENUTAU_COMMIT"
dotnet build "$tool" -c Release --nologo -v quiet -clp:ErrorsOnly
render() { dotnet "$tool/bin/Release/net10.0/openutau-render.dll" "$@"; }

render singers | tee "$out/singers.tsv"
singer_line="$(awk -F'\t' -v name="$SINGER_PATTERN" -v type="$SINGER_TYPE" 'tolower($1 $2) ~ tolower(name) && $3 == type { print; exit }' "$out/singers.tsv")"
[ -n "$singer_line" ] || { echo "no $SINGER_TYPE singer matching $SINGER_PATTERN" >&2; exit 1; }
singer_id="$(cut -f1 <<<"$singer_line")"
colors="${COLORS-$(cut -f4 <<<"$singer_line")}"
phonemizer="${PHONEMIZER:-}"
echo "singer=$singer_id colors=$colors phonemizer=${phonemizer:-default}"

(cd "$repo/tools/ustx-from-vocal" && uv run ustx_from_vocal.py \
  "$song/02-composition/generated/vocal.tsv" \
  --score "$song/02-composition/score.toml" \
  --out "$out/mou-ikkai.ustx" \
  --tuning "$song/03-vocal/tuning.toml" \
  --singer "$singer_id" \
  ${colors:+--colors "$colors"} \
  ${phonemizer:+--phonemizer "$phonemizer"})

render render "$out/mou-ikkai.ustx" ${PITCH:+--pitch} --save "$out/mou-ikkai.ustx" --phonemes "$out/phonemes.tsv" --out "$out"

closures="$(cut -f3 "$out/phonemes.tsv" | grep -cx 'っ' || true)"
sung_cl="$(cut -f3,5 "$out/phonemes.tsv" | grep -cx $'っ\tcl' || true)"
echo "closures: $closures, sung as cl: $sung_cl"
[ "$closures" = "$sung_cl" ]
ffmpeg -hide_banner -nostats -i "$out/mou-ikkai_Vocal.wav" -af ebur128=framelog=quiet -f null - 2>&1 | sed -n '/Summary/,$p' | tee "$out/loudness.txt"
