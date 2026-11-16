# ai-produced

AI が人間のボカロPと同じ工程を踏んだときに、本当に良い曲を作れるかを確かめる実験リポジトリである。
完成品を一度に出す音楽生成 AI とは違い、作詞、作曲、編曲、歌声の打ち込みと調声、ミックス、映像の順に工程を進める。
各工程の成果物と、AI が何を判断して何をやり直したかの記録を残す。

## 曲と記録を読む

曲は [`songs/`](songs/) に 1 曲ずつある。
各曲の `00-brief.md` が企画で、`log.md` が工程ごとの判断とやり直しの記録である。
どの工程の成果物がどこにあるかは [`docs/songs.md`](docs/songs.md) にある。

## 手元で動かす

- Vite+ の CLI である `vp`（`vp install` が Rust のツールチェーンとクレートも取得する）
- `tools/openutau-render` をビルドする .NET
- 歌声と音源の書き出しに使う OpenUtau と REAPER

```sh
vp install
vp run verify
```

曲の映像を確かめる手順など、アプリごとの使い方は `apps/` の各ワークスペースの README.md にある。
