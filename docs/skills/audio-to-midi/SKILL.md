---
name: audio-to-midi
description: ステムやミックスの音声をMIDIに採譜するとき、採譜の精度を正解のMIDIと比べるときに使う。
---

# 音声からMIDIへの採譜

素材の取得と置き場は `reference-materials` スキルに従う。
`tools/audio-to-midi` で動かす。

```sh
uv run python transcribe.py <音声...> -o <出力先>
uv run python score.py <正解MIDI> <採譜MIDI>
```

パートに合わせてエンジンを選ぶ。

| パート                   | エンジン                                                        |
| ------------------------ | --------------------------------------------------------------- |
| ピアノ、シンセのプラック | 既定のMuScriptor small                                          |
| ステムの無い曲のミックス | `-e yourmt3`                                                    |
| ベース                   | どのエンジンも1オクターブ下を拾うので、採った音高を確かめて直す |

MuScriptorの重みは、Hugging Faceでモデルのライセンスに同意したアカウントのトークンで取得する。
MuScriptorの重みはCC BY-NC 4.0なので、非商用の分析だけに使う。
比べた数値と選んだ理由は `docs/tool-selection.md` の「音声からMIDIへの採譜」にある。
