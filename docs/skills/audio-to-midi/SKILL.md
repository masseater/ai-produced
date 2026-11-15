---
name: audio-to-midi
description: ステムやミックスの音声をMIDIに採譜するとき、採譜の精度を正解のMIDIと比べるときに使う。
---

# 音声からMIDIへの採譜

素材の取得と置き場は `reference-materials` スキルに従う。
コマンドはリポジトリのルートで実行する。

```sh
cargo run --release -p transcribe --bin transcribe -- <音声...> -o <出力先>
cargo run --release -p transcribe --bin transcription-score -- <正解MIDI> <採譜MIDI>
```

パートに合わせてエンジンを選ぶ。

| パート                   | エンジン                                                                        |
| ------------------------ | ------------------------------------------------------------------------------- |
| ピアノ、シンセのプラック | 既定のMuScriptor small                                                          |
| ステムの無い曲のミックス | 既定のMuScriptor small。楽器ごとに1トラックを書く。正解との比較はまだしていない |
| ベース                   | どのエンジンも1オクターブ下を拾うので、採った音高を確かめて直す                 |

`-e basic-pitch` でBasic Pitchに切り替えられる。
初回の実行で、Hugging FaceからMuScriptorのONNXの重み（約400MB）を取得する。トークンは要らない。
MuScriptorの重みはCC BY-NC 4.0なので、非商用の分析だけに使う。
MuScriptorはCPUで1秒に約10トークンを生成する。
比べた数値と選んだ理由は `docs/tool-selection.md` の「音声からMIDIへの採譜」にある。
