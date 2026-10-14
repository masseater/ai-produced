# ai-produced

AI が人間のボカロPと同じ工程を踏んだときに、本当に良い曲を作れるかを確かめる実験リポジトリである。
完成品を一度に出す音楽生成 AI とは違い、作詞、作曲、編曲、歌声の打ち込みと調声、ミックス、映像の順に工程を進める。
各工程の成果物と、AI が何を判断して何をやり直したかの記録を残す。

## 曲と記録を読む

曲は [`songs/`](songs/) に 1 曲ずつある。
各曲の `00-brief.md` が企画で、`log.md` が工程ごとの判断とやり直しの記録である。
どの工程の成果物がどこにあるかは [`docs/songs.md`](docs/songs.md) にある。

## 手元で動かす

前提は次のとおりである。

- Vite+ の CLI である `vp`
- 解析と変換のツールを動かす uv と .NET
- 歌声と音源の書き出しに使う OpenUtau と REAPER
- Web アプリをデプロイするなら Cloudflare のアカウント

```sh
vp install
cp .env.example .env
vp run verify
```

`.env` の `OPENROUTER_API_KEY` には OpenRouter の API キーを入れる。
`vp run verify` が通れば、手元の準備はできている。

曲の映像は Remotion で作る。
REAPER で書き出した音声を、`apps/video/package.json` の `studio` が `--public-dir` に指定する場所へ置いてから、次のコマンドで確かめる。

```sh
cd apps/video
vp run studio
```

Web アプリをデプロイするときは、Cloudflare の認証情報を Alchemy のプロファイルに登録してから `apps/web` でデプロイする。

```sh
vp exec --filter web alchemy profile edit --add Cloudflare
cd apps/web
vp run deploy
```

## GitHub Actions の準備

main への push で検査とデプロイが走る。
リポジトリの Actions secrets に次を登録する。

| 名前                      | 用途                                      |
| ------------------------- | ----------------------------------------- |
| `OPENROUTER_API_KEY`      | 検査で使う jev-lint の判定                |
| `CLAUDE_CODE_OAUTH_TOKEN` | 検査が落ちたときの Claude Code の自動修正 |
| `CLOUDFLARE_ACCOUNT_ID`   | Web アプリのデプロイ                      |
| `CLOUDFLARE_API_TOKEN`    | Web アプリのデプロイ                      |

Settings → Actions → General では「Allow GitHub Actions to create and approve pull requests」を有効にする。
release-please と自動修正が PR を作るためである。
