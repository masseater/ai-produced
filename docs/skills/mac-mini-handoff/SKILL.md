---
name: mac-mini-handoff
description: REAPERやMac上のプラグインが要る作業を、クラウドのスレッドからMac mini常駐のスレッドへ渡すとき、またはMac mini側で受けるときに使う。.RPPを開く、書き出す、Macのプラグインで鳴らす作業が該当する。
---

# Mac miniへの受け渡し

クラウド側は分析、設計、MIDI、.RPPの生成、スクリプトを受け持つ。
Mac mini側は、REAPERとMac上のプラグインを使う作業を受け持つ。
受け渡しはGitHubのブランチだけで行い、スレッド間でファイルの中身を送らない。

クラウド側は作業ブランチにpushし、常駐スレッドにはブランチ名とやることだけを短く伝える。
要るときは対象の.RPP、書き出しの設定、出力先も書く。
Mac mini側はそのブランチをpullして作業し、書き出した音声や更新した.RPPを同じブランチにpushする。
クラウド側はそれをpullして続きを進める。

REAPERをシェルから動かすスクリプトは `songs/001-mou-ikkai/04-reaper/` にある。
