# 分析wiki

参照作品（曲、MV、音MAD、文字PV）を分析した結果を溜める場所である。
1作品を1ページとし、複数の作品にまたがる知見はトピックとして分ける。

## 作品

| ページ                                            | 作者               | 種類   | タグ       | 分析日     |
| ------------------------------------------------- | ------------------ | ------ | ---------- | ---------- |
| [まにまに 2025 Edit](works/yt-6eDhCz2vzVc.md)     | r-906              | 曲     | taste-ref  | 2026-10-04 |
| [イシェド](works/yt-jBGlPuMdkCM.md)               | yowanecity         | 曲     | taste-ref  | 2026-10-04 |
| [死んでしまったんだ](works/yt-eCSO4fL98Xk.md)     | 椎乃味醂           | 曲     | taste-ref  | 2026-10-04 |
| [Cheerleader](works/yt-CzJbz9qSsd0.md)            | Porter Robinson    | 曲     | taste-ref  | 2026-10-04 |
| [スライ村](works/yt-MFs7FgQU2xM.md)               | 稲むり             | 曲     | taste-ref  | 2026-10-04 |
| [Remember](works/yt-UE1y01q6wzQ.md)               | yuigot             | 曲     | taste-ref  | 2026-10-04 |
| [私だけの魔法。](works/yt-qd3q0dYpprs.md)         | アメリカ民謡研究会 | 曲     | taste-ref  | 2026-10-04 |
| [STARGAZER](works/yt-oqmkKOLlKQA.md)              | Tsundere Alley     | 曲     | taste-ref  | 2026-10-04 |
| [マゾチュウ JIZURA.Ver](works/nico-sm46868333.md) | 不明               | 文字PV | lyric-sync | 2026-10-04 |
| [Symphony in Acid](works/yt-_n_iKR3Icio.md)       | Max Cooper         | MV     |            | 2026-10-04 |

## トピック

| ページ                                      | 内容                                     |
| ------------------------------------------- | ---------------------------------------- |
| [好みプロファイル](topics/taste-profile.md) | 好きな8曲から読み取った音と映像の好み    |
| [反復と快](topics/repetition-aesthetics.md) | 反復が快を生むという先行研究とMVへの応用 |

## 書き方

作品のページは[テンプレート](template.md)を写して作る。
見出しは基本情報、URL、解析数値、所見、好みとの関係、参考にする点の6つで、順番も変えない。
書けない見出しは消さずに「未分析」と書く。
推測は推測と明記し、確かめた事実と分ける。
本文は箇条書きにせず、1行1文の地の文で書く。

ファイル名は配信元の接頭辞と、その配信元のIDをつないだものにする。

| 配信元       | ファイル名                         |
| ------------ | ---------------------------------- |
| YouTube      | works/yt-動画ID.md                 |
| ニコニコ動画 | works/nico-動画ID.md               |
| その他       | works/ドメイン-英数字の短い名前.md |

同じ作品を分析し直したときは、新しいページを作らずに既存のページを書き換え、分析日を更新する。
frontmatterのidはファイル名と揃える。
kindはsong、mv、otomad、lyric-videoのどれかにし、tagsで作品の束（taste-refなど）を表す。

複数の作品から言えることはtopics/に書き、各作品のページからリンクする。
解析で出した数値の元データはdata/にJSONで置き、名前に版（-v1など）を付ける。
音源、歌詞の全文、音源から抜き出したMIDIはリポジトリに入れない。

## 索引の更新

作品やトピックを足したら、同じコミットでこのページの表に1行足す。
作品の表は分析した順に並べ、新しい作品を末尾に足す。
分析し直したときは、表の分析日も書き換える。
