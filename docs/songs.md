# 曲の置き場所

1曲を1ディレクトリとし、songs/001-slug/のように連番と短い名前で作る。
工程ごとに次のディレクトリへ分ける。

| パス            | 中身                                     |
| --------------- | ---------------------------------------- |
| 00-brief.md     | お題、狙い、参考曲                       |
| 01-lyrics/      | 歌詞を版ごとに残す                       |
| 02-composition/ | メロディとコードのMIDIまたはMusicXML     |
| 03-arrangement/ | REAPERのプロジェクト（.RPP）             |
| 04-vocal/       | OpenUtauの.ustxまたはSynthesizer Vの.svp |
| 05-mix/         | ミックスとマスタリングの設定、解析結果   |
| 06-video/       | 映像の素材と設定                         |
| renders/        | 書き出した音声（Git LFS）                |
| log.md          | 工程ごとの判断とやり直しの記録           |

自作のMCPサーバーもRemotionのプロジェクト（apps/video/）も、アプリはすべてapps/に置く。
