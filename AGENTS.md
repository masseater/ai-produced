作業を始める前に `repo-conventions` と `vite-plus` の skill を読み、従う。どちらも masseater/skills にあり、`vp install` が `.agents/skills/` に入れる。このリポジトリのベースは https://github.com/masseater/typescript-template で、テンプレート由来の構成と方針はテンプレートに従う。

## 構成

- `songs/<連番>-<名前>/` に1曲ずつ工程別の成果物と log.md を置く。構成は `docs/songs.md` にある。
- `apps/` の各ワークスペースの内容は、それぞれの README.md にある。
- `tools/` 配下のツールは、Cargoワークスペース内のRustクレートである。ただし、`openutau-render` はC#である。`cargo run -p <crate> --bin <tool> -- <args>` で実行する。`vp run verify` を実行すると、rustfmt、clippy、およびクレートのテストが実行される。

## CI

- `pull_request` トリガーのワークフローは、最新の main を取り込んでいない PR のマージをルールセットで拒むための `up-to-date`（何も検査しない）だけにする。

## 制作

- 画面操作（スクリーンショット、クリック、キー送信）をしない。ツールは MCP、スクリプト、プロジェクトファイルだけで操作する。
- 曲は `docs/songs.md` の置き場所で、人間と同じ工程を飛ばさずに進める。工程を進めたら、その曲の log.md に判断とやり直しの理由を書く。
- 書き出した音声は、ラウドネス、スペクトログラム、ピッチなどの解析結果で判断し、その数値を log.md に残す。
- 不具合の調査は `systematic-debugging`、完了の確認は `verification-before-completion` スキルに従う。

## 成果物と素材

- 本番デプロイと外部公開は、ユーザーの指示を待つ。
- このリポジトリは public である。他人の曲や映像に由来する素材は入れず、`reference-materials` スキルに従う。
- REAPER や OpenUtau の歌手など、Windows PC にあるものが要る工程は、`windows-pc` スキルに従って PC 上の Remote Control のセッションで実行する。
