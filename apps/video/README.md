# video

曲の映像は Remotion で作る。
`songs/` から書き出した音声と、`tools/song` の `lyric-timing` が作る歌詞のタイミングを読み込む。

## 曲の映像を確かめる

REAPER で書き出した音声を、`vite.config.ts` の `studio` タスクが `--public-dir` に指定する場所へ置いてから、次を実行する。

```sh
vp run studio
```
