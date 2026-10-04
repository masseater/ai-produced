# SPEC.md

完成品を一発で出す生成AIではなく、人間のボカロP（Producer）と同じ制作工程をAIが踏んだときに、本当に良い曲が作れるかを確かめる実験リポジトリである。

## 実験の要求

工程は作詞、作曲、編曲、歌声の打ち込みと調声、ミックス、映像の順に人間と同じく踏む。
AIは画面操作をしない。ツールはMCP、スクリプト、プロジェクトファイルだけで操作する。
既存のMCPがなければ、探したうえでapps/に自作する。
作業はユーザーのMac（mac-mini）で行う。
各工程の中間成果物と、AIが何を判断して何をやり直したかの記録を残す。

## 制作ツール

DAWはREAPERを使い、音源は必要になったものを都度追加する。
歌声はOpenUtauかSynthesizer V Studio 2で打ち込み、調声する。
映像はRemotionで作り、必要ならBlenderを使う。
選定の経緯はdocs/tool-selection.mdにある。曲ごとの置き場所はdocs/songs.mdに従う。
参照作品の分析結果はdocs/wiki/に溜める。

## テンプレートの要求

必須技術を使っていなければ、検査が失敗する。ガイドに書くだけでは強制しない。
ハーネスは、より厳しい手段をClaudeが自ら調べて適用できる構成とする。
要求はあえて曖昧に書く。ただし完了時には検証できる形にする。
テストは持たない。CIはGitHub Actionsで検証だけを走らせる。RCやbetaのバージョンも許容する。

## 技術

言語はTypeScript 7で、effectと@effect/tsgoを使う。
リポジトリはVite+によるモノレポである。
アプリはTanStack Startで組み、APIはElysiaJSとEden、サーバー状態はTanStack Queryで扱う。
UI状態はeffect-atom、UIはshadcn/ui、構成はFeature-Sliced Designに従う。
認証はbetter-auth、データベースアクセスはDrizzle、機能フラグはOpenFeature、計測はOpenTelemetryを使う。
デプロイ先はCloudflare WorkersとD1で、Alchemyで定義する。
検査にはoxlint、oxfmt、fallow、steiger、textlint、yomiyasuを使う。
依存の更新はRenovateで行い、CIが通ったものだけを自動でマージする。
