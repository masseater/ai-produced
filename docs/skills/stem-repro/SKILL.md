---
name: stem-repro
description: 公開されたステムとMIDIがある曲を、パートごとに作り直して原曲と計測で比べ、音作りを数値で取り出すときに使う。
---

# ステム再現

手法の考え方、指標の定義、結果の読み方は `docs/wiki/topics/stem-reproduction.md` にある。
素材と曲ごとの `song.json` は `reference-materials` スキルに従って reference 側に置く。

## 環境

画面を使わずクラウドで動かす。

| 用途   | 使うもの                                                     |
| ------ | ------------------------------------------------------------ |
| シンセ | Surge XT 1.3.4のLinux版debを展開し、VST3を`/opt/surge`に置く |
| 音色   | debに入っている`/usr/share/surge-xt/patches_*`               |
| ピアノ | FluidSynthとSalamander Grand PianoのSF2（CC-BY 3.0）         |

VST3と音色の場所は、環境変数`SURGE_VST3`と`SURGE_PATCHES`で変えられる。

## 曲の設定

`song.json`には、テンポ、ステムのパス、プリマスターとマスターのパス、セクション、MIDIのあるパートを書く。
パスは`song.json`からの相対パスである。

```json
{
  "bpm": 140,
  "stems": "stem/sentaku_{name}.wav",
  "premaster": "ただ選択があった_off_noMst.wav",
  "master": "ただ選択があった_off_mst.wav",
  "stem_names": ["drums_kick", "drums_top", "bass", "arp1"],
  "sections": { "intro": [0, 8], "B": [16, 40], "break": [40, 48] },
  "parts": {
    "bass": { "midi": "song.mid", "track": 4, "search_bars": [8, 10], "rescore_bars": [8, 12] },
    "arp2": { "midi": "transcribed/arp2.mid", "search_bars": [40, 42], "rescore_bars": [40, 44] }
  }
}
```

## 手順

`tools/stem-repro`で`uv run python cli.py <song.json> <コマンド>`として、上から順に動かす。
結果はJSONで標準出力に出る。

| 順  | コマンド                                                                     | やること                                                                                       |
| --- | ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------- |
| 1   | `stems`                                                                      | ステムごとのラウドネス、ピーク、ステレオ幅、鳴っている区間、帯域の配分を出す                   |
| 2   | `interstem --section B --kick drums_kick --bass bass`                        | ステム同士の関係を出す                                                                         |
| 3   | `mastering --bus`                                                            | ステムの和からプリマスター、プリマスターからマスターへの変化を出す                             |
| 4   | `transcribe.py`（下記）                                                      | MIDIの無いパートを採譜する                                                                     |
| 5   | `filter-notes --stem arp2 --midi <採譜MIDI>`                                 | ステムに無い音を捨て、ベロシティを音量から付け直す                                             |
| 6   | `search --out <作業先>`                                                      | Surgeの全音色を各パートで採点し、上位を長い区間で採点し直す                                    |
| 7   | `optimize --part bass --patch <fxp>`                                         | 音量とフィルターの包絡とノート全体の時間のずれを動かし、原曲との差を下げる。出たずれを13に渡す |
| 8   | `kick-fit --stem drums_kick --section B`                                     | 複数発を揃えた中央値の波形に、キックの合成パラメータを合わせる                                 |
| 9   | `kick-track --stem drums_kick --section B --params <8の出力>`                | 1発ずつ位置と強さを検出して並べ、音量包絡を5 msごとに原曲へ寄せる                              |
| 10  | `resynth --stem drums_top --out <wav>`                                       | 音色で作れないパートをノイズから作り直す。所見には使わない                                     |
| 11  | `place --stem bass --raw <wav> --out <wav>`                                  | EQ、ステレオ幅、左右の偏り、ラウドネスを原曲のステムに合わせる                                 |
| 12  | `shape --stem piano --raw <wav> --out <wav>`                                 | 小節ごとと1拍の中の音量の形を原曲に合わせてから`place`をかける                                 |
| 13  | `note-shape --stem arp1 --part arp1 --raw <wav> --out <wav> --shift-ms <ms>` | 鳴っていないノートを除き、ノート単位の切り方と包絡と音量の組み合わせから一番近いものを選ぶ     |
| 14  | `fit-bus-master --stems-dir <ステムの書き出し先> --out <dir>`                | バスのコンプレッサーと、マスターのEQとソフトクリッパーを原曲に当てはめる                       |

13まで進めて包絡差が残ったパートは、次の順で原因を分けてから作り直す。

| 順  | コマンド                                                                           | やること                                                                                     |
| --- | ---------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| 1   | `timbre --stem arp2 --part arp2 --repro <wav> --bars 40 48`                        | 倍音と残差の入れ替え、記述子と倍音の時間変化の差を出し、主因を決める                         |
| 2   | `edit-notes --midi <mid> --out <mid> --splice <別の採譜> --splice-bars 40 48`      | ノートが原因の区間を別の採譜に差し替える。`--grid 16 --fixed`、`--level-bars`でも起こし直す  |
| 3   | `design --part arp2 --template "Init Saw" --filter lp24 --bars 40 48 --midi <mid>` | 初期音色からフィルターの種類を決め、オシレーター、フィルター、包絡を倍音の時間変化に合わせる |
| 4   | `render --part arp2 --params <3の出力> --midi <mid> --out <wav>`                   | 曲全体を書き出し、13に渡す。13は原曲の無音区間を写す候補も比べる                             |

採譜は`uv run --python 3.11 --with "basic-pitch[onnx]" --with "numpy<2" python transcribe.py <出力先> <BPM> <ステム...>`で動かす。
`interstem`に`--repro <ディレクトリ>`を付けると、作り直したステムの値も並べて出す。
最後に、9〜14が当てた量と、6〜8で残った差を作品のwikiページに記録する。
