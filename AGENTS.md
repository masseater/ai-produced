作業を始める前に `repo-conventions` と `vite-plus` の skill を読み、従う。どちらも masseater/skills にあり、`vp install` が `.agents/skills/` に入れる。このリポジトリのベースは https://github.com/masseater/typescript-template で、テンプレート由来の構成と方針はテンプレートに従う。

## 構成

- `songs/<連番>-<名前>/` に1曲ずつ工程別の成果物と log.md を置く。構成は `docs/songs.md` にある。
- `apps/video` は Remotion の映像で、`songs/` の書き出し音声と、`tools/lyric-timing` が作る歌詞のタイミングを読む。
- `apps/wiki` は `docs/wiki/` の分析 wiki を閲覧と検索のために配信する。
- `tools/` の解析や変換は Python（`uv run --project tools/<name>`）か C#（`openutau-render`）で書かれ、`vp run verify` の対象外である。
- コードコメントは書かず、lint が落とす。必須技術のパッケージは pnpm の stack カタログから参照し、未参照のカタログ項目と未使用の依存は fallow が落とす。
- Claude Code では `.claude/settings.json` が pnpm・pnpx・pn・pnx の実行を拒否する。

## CI

- CI は main への push で走らせる。ワークフローは `.github/workflows/verify.yml` で、デプロイやリリースのジョブは `needs: verify` で main の必須 CI が通ってから動く。
- `pull_request` トリガーのワークフローは、最新の main を取り込んでいない PR のマージをルールセットで拒むための `up-to-date`（何も検査しない）だけにする。
- Claude のクラウドセッションではプロキシが GitHub API を拒むため、session-start フック（`.claude/hooks/session-start.sh`）が `ZIZMOR_NO_ONLINE_AUDITS=true` を設定し、zizmor はオフライン監査だけを行う。オンライン監査は main の CI が担う。

## 制作

- 画面操作（スクリーンショット、クリック、キー送信）をしない。ツールは MCP、スクリプト、プロジェクトファイルだけで操作する。
- 曲は `docs/songs.md` の置き場所で、人間と同じ工程を飛ばさずに進める。工程を進めたら、その曲の log.md に判断とやり直しの理由を書く。
- 書き出した音声は、ラウドネス、スペクトログラム、ピッチなどの解析結果で判断し、その数値を log.md に残す。
- 不具合の調査は `systematic-debugging`、完了の確認は `verification-before-completion` スキルに従う。

## 成果物と素材

- 本番デプロイと外部公開は、ユーザーの指示を待つ。
- このリポジトリは public である。他人の曲や映像に由来する素材は入れず、`reference-materials` スキルに従う。
- REAPER や Mac 上のプラグインが要る作業は、`mac-mini-handoff` スキルに従って Mac mini 常駐のスレッドへ渡す。
