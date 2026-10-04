# ai-vocaloid ツール選定案

作成は2026-10-04。ボカロ曲（作詞 → 作曲 → 編曲 → 歌声合成 → ミックス・マスタリング → 動画）を1曲作る前提で比較した。

> 価格・対応OSは2026年6月時点の知識ベース。導入時に公式サイトで再確認する（「要確認」と書いたものは特に）。

## 1. 最初に決める軸：「人間と同様の操作」の定義

| 案 | 中身 | 長所 | 短所 |
|---|---|---|---|
| A. GUI操作のみ | 画面を見てマウス・キーボードで操作（computer use / xdotool） | 実験として一番純粋 | 1曲に膨大な操作回数。ピアノロール入力やフェーダー操作は精度が落ち、「AIの音楽的判断」より「操作の下手さ」を測る実験になりがち |
| **B. 同じツール・同じ工程、操作手段は自由（推奨）** | 人間と同じDAW・歌声ソフトを使い、同じ順番で作業。操作はGUI・スクリプトAPI・プロジェクトファイル編集のどれでも可 | 「プロセスを踏めば良くなるか」という本題を測れる。各工程の中間成果物（譜面、MIDI、DAWプロジェクト）が人間の制作物と同じ形で残る | GUI純粋主義ではない |
| C. コード生成のみ | Python等で音を直接生成 | 速い | 人間のプロセスと別物。実験の趣旨から外れる |

**重要な制約：AIは音を「聴けない」。** 人間の「聴いて直す」ループの代わりに、書き出した音声を解析（スペクトログラム画像、ラウドネス、ピッチ追従、音域チェック、可能なら音声理解モデル）して判断する仕組みが必要。これ自体が実験の見どころになる。

## 2. 実行環境

| 案 | 内容 | 長所 | 短所 |
|---|---|---|---|
| **クラウドLinux（推奨）** | このプロジェクトのコンテナにLinux版ツールを入れ、仮想ディスプレイ（Xvfb）上で動かす | ユーザーのPCを占有しない。全工程を再現可能に記録できる。ほぼ無料で始められる | Windows/Mac専用ソフト（VOCALOID6、Cubase等）が使えない。コンテナは使い捨てなので成果物はリポジトリ等に保存が必要 |
| 手元PC（Remote Control + computer use） | ユーザーのWindows/Mac上で実際のソフトを操作 | 本家VOCALOID・市販DAW・手持ちプラグインが使える | 操作中はPCを占有。速度が遅い。要デスクトップアプリ |

## 3. 工程ごとの候補

### 作詞・企画
- テキスト（Markdown）でClaude自身が書く。人間もテキストエディタで書くので差がない。ツール選定不要。

### 作曲（メロディ・コード）
| ツール | 操作手段 | コスト | OS | メモ |
|---|---|---|---|---|
| **MuseScore 4** | GUI / MusicXML編集 / CLIで書き出し | 無料 | Win/Mac/Linux | 譜面で考える人間の工程そのもの。MusicXML・MIDI書き出しが歌声ソフトに直結 |
| DAWのピアノロール | DAW次第 | — | — | 作曲もDAW内で完結させる流派ならこちら |

### 編曲・打ち込み（DAW）
| ツール | 操作手段 | コスト | OS | メモ |
|---|---|---|---|---|
| **REAPER** | GUI / ReaScript（Python・Lua）/ .RPPはテキスト / CLIレンダリング | 個人$60（評価期間あり） | Win/Mac/Linux | AIが操作するDAWとして最有力。ほぼ全機能をスクリプトから叩ける |
| Ardour | GUI / Luaスクリプト | 無料〜少額 | Win/Mac/Linux | オープンソース。REAPERよりスクリプト周りは弱い |
| LMMS | GUI / .mmpはXML | 無料 | Win/Mac/Linux | 手軽だが本格的なミックスには弱い |
| Studio One / Cubase / Logic | GUIのみ（実質） | 有料 | Win/Mac | 手元PC案のときだけ候補。自動化手段が乏しい |

音源（Linuxで動くもの）：Surge XT・Vital（シンセ）、Dexed（FM）、sfizz + 無料サンプル（ピアノ・ドラム等）。生楽器のリアルさは有料音源に劣る。

### 歌声合成（ボーカル）
| ツール | 操作手段 | コスト | OS | メモ |
|---|---|---|---|---|
| **Synthesizer V Studio 2 Pro** | GUI / Lua・JSスクリプト / プロジェクトはJSON | 本体＋ボイス各1〜2万円程度 | Win/Mac/Linux（Linux版は要確認） | 品質・調声機能・スクリプト対応のバランスが最良。日本語ボイス多数 |
| OpenUtau | GUI / .ustxはYAML | 無料（ボイスも無料多数） | Win/Mac/Linux | DiffSinger系ボイスで品質も十分。お金をかけずに始めるならこれ |
| NEUTRINO | CLI（MusicXML入力） | 無料 | Win/Mac/Linux（要確認） | 譜面から自動で歌わせる。調声の余地が小さく「人間の調声工程」の再現には不向き |
| VOCALOID6 | GUIのみ | 有料 | Win/Macのみ | 「本物のボカロ」にこだわるならこれ。手元PC案が前提になる |
| VOICEVOX（ソング） | GUI / HTTP API | 無料 | Win/Mac/Linux | 歌唱機能は簡易的 |

### ミックス・マスタリング
- REAPER内で完結（ReaEQ・ReaComp等の標準プラグイン＋LSP/x42/Calfなど無料プラグイン）。
- マスタリング：REAPER内で手動、または参考音源に合わせる Matchering（Python, 無料）を比較用に。
- 確認用：ffmpeg / librosa でラウドネス（LUFS）、スペクトル、ピッチを数値化・画像化。

### 動画（MV）
| ツール | 操作手段 | コスト | メモ |
|---|---|---|---|
| **Krita**（イラスト） | GUI / Pythonスクリプト | 無料 | 1枚絵＋歌詞のリリックビデオなら十分 |
| **Blender**（動画編集・モーション） | GUI / Python API | 無料 | 歌詞アニメーションや簡単なモーショングラフィックス |
| Kdenlive | GUI / MLT XML | 無料 | シンプルな編集向け |
| ffmpeg | CLI | 無料 | 最終書き出し |

## 4. 推奨構成（クラウドLinux × 案B）

| 工程 | ツール | 費用 |
|---|---|---|
| 作詞 | テキスト | 0 |
| 作曲 | MuseScore 4 | 0 |
| 編曲・ミックス | REAPER ＋ 無料音源/プラグイン | 評価期間中0、継続なら$60 |
| 歌声 | OpenUtau で開始 → 品質不足なら Synthesizer V に切替 | 0 → 1〜3万円 |
| 聴く代わり | ffmpeg / librosa による解析＋スペクトログラム画像 | 0 |
| 動画 | Krita ＋ Blender ＋ ffmpeg | 0 |
| 保存 | GitHubリポジトリ（各工程の中間成果物と作業ログを残す） | 0 |

実験として面白くするための提案を挙げる。
- 各工程で「AIが判断したこと・やり直したこと」をログに残し、後で人間の制作手順と比較できるようにする。
- 比較対象として、同じお題で完成品を一発で出す生成AI（Suno等）の出力も1本作っておくと「プロセスを踏む価値」を評価しやすい。

## 5. 決めてほしいこと
1. 操作の定義：B（同じツール・同じ工程、操作手段は自由）でよいか
2. 実行環境：クラウドLinuxでよいか（手元PCにするなら VOCALOID6 等も候補に入る）
3. 歌声：OpenUtau（無料）から始めるか、最初から Synthesizer V（有料）か、VOCALOID6 にこだわるか

## 6. ユーザーの想定（2026-10-04）
- DAWで曲を作る（DAWは未確定。推奨は REAPER）
- 打ち込み・調声は Synthesizer V か OpenUtau（推奨：OpenUtau で通してから必要なら SynthV へ）
- 映像は Remotion（React製。個人利用は無料。Linuxのヘッドレス Chromium で書き出し可能）
- 未決定：DAWの種類、作業場所（クラウド or 手元PC）

## 7. 画面操作なし（MCP/スクリプトのみ）で使えるか（2026-10-04調査）

ユーザー方針：画面操作は禁止。操作はMCP（既存を探すか自作）経由のみ。作業場所は手元のMac。

| ツール | 既存MCP | Macで画面操作なしにできること | 足りないこと |
|---|---|---|---|
| FL Studio | karl-andres/fl-studio-mcp と派生（joanj94版はMac対応、IAC Driver経由のMIDI通信）、calvinw/fl-studio-mcp（ピアノロール特化、Mac主対象） | チャンネル追加・パラメータ、ミキサー音量/パン、エフェクトON/OFF、ピアノロールのノート書き込み。書き出しはFL本体のコマンドライン（`-R -Ewav -F`）で可能 | joanj94版はMacでプラグイン読み込み・レンダリング非対応。オートメーション作成不可。ピアノロール反映はCmd+Opt+Yのキー送信（osascript）が必要。rosasynthesiz版（67ツール）はWindows専用 |
| REAPER | total-reaper-mcp（ReaScript全網羅をうたう）、TwelveTake-Studios（176ツール）、mthines（ミックス・メータリング）など多数 | ReaScript経由でほぼ全操作（トラック、MIDI、プラグイン読み込み、オートメーション、レンダリング）。Vitalも使える | FL Studio付属プラグインは基本使えない。ライセンス$60 |
| Synthesizer V Studio 2 | tatat/svs-mcp（Mac＋SV2 Pro限定。ノート、歌詞、音素）、ocadaruma/mcp-svstudio | ノート・歌詞・音素の入力と編集 | 歌手の選択はスクリプトAPIがなく手動。音声書き出しをスクリプトから行えるかは要確認。調声パラメータ（ピッチ曲線等）は拡張が必要 |
| OpenUtau | turboegg1145/OpenUtau-MCP（.ustx生成、DiffSinger表情曲線。ただし星1の小規模プロジェクト、プレビューは独自合成） | .ustx はYAMLなのでノート・歌詞・ピッチ・表情曲線まで全部ファイルで書ける。書き出しは OpenUtau-Headless（コミュニティ製CLI）で可能性あり | 公式にはヘッドレス書き出し非対応（Issue #1615）。自前MCPを作る前提 |
| Remotion | 不要（Reactのコードそのもの） | 全部 | なし |
| Blender | 既存MCPあり（blender-mcp） | Python API で全部 | なし |

### 推奨の変化
画面操作禁止だと、DAWは FL Studio より REAPER の方がはるかに自動化しやすい（プラグイン読み込み・オートメーション・レンダリングが全部MCP経由で可能）。
全工程を画面なしで完結できる歌声ソフトは OpenUtau である（ファイル編集＋ヘッドレス書き出し、必要なら自作MCP）。SynthV は品質で勝るが、歌手選択などに手動作業が残る。

出典を以下に示す。
- https://github.com/joanj94/fl-studio-mcp
- https://github.com/calvinw/fl-studio-mcp
- https://github.com/rosasynthesiz/flstudio-mcp
- https://github.com/shiehn/total-reaper-mcp
- https://github.com/TwelveTake-Studios/reaper-mcp
- https://github.com/tatat/svs-mcp
- https://github.com/ocadaruma/mcp-svstudio
- https://github.com/turboegg1145/OpenUtau-MCP
- https://github.com/boxboy523/OpenUtau-Headless
- https://github.com/openutau/OpenUtau/issues/1615
- https://www.image-line.com/fl-studio-learning/fl-studio-online-manual/html/fformats_save_export.htm
