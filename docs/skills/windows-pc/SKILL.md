---
name: windows-pc
description: REAPER、OpenUtauの歌手、プラグインなど、ユーザーのWindows PCに入っているものが要る工程を進めるときに使う。曲の`vocal`、`audition`、`mix`タスクが該当し、クラウドのスレッドがPC上でRemote Controlのセッションを始めて実行させる。
---

# windows-pc

クラウドのスレッドは、分析、設計、MIDI、.RPPの生成、スクリプトを受け持つ。
REAPER、OpenUtauの歌手、プラグインなど、ユーザーのWindows PCに入っているものが要る工程は、そのPC上のRemote Controlのセッションで実行する。
クラウドのスレッドは、このリポジトリがあるPCのフォルダを指定して`start_remote_control`でセッションを始める。ユーザーは承認のプロンプトでセッションを承認する。

受け渡しはGitHubのブランチだけで行い、スレッド間でファイルの中身を送らない。
クラウドのスレッドは作業ブランチにpushし、セッションにはブランチ名と、曲のディレクトリで実行するタスク（`vp run vocal`、`vp run audition`、`vp run mix`など）だけを伝える。
セッションはブランチをpullし、`vp install`とタスクを実行して、計測した値を曲のlog.mdに書き、同じブランチにpushする。
書き出した音声は`.gitignore`に入っているので、PC上の曲のrenderディレクトリに残る。
タスクはREAPERとOpenUtauをコマンドラインから動かすので、セッションは画面を操作しない。
