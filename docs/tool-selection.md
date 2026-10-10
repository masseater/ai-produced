# ツール選定の記録

2026-10-04に、ボカロ曲を1曲作る前提で制作ツールを選んだ。
工程は作詞、作曲、編曲、歌声合成、ミックスとマスタリング、映像である。

## 前提として決めたこと

AIは画面操作をしない。クリックやスクリーンショットで操作すると、音楽的な判断より操作の下手さを測る実験になるからである。
ツールはMCP、スクリプト、プロジェクトファイルだけで操作し、既存のMCPがなければ自作する。
作業はユーザーのMac（mac-mini、Apple M4、macOS 26.5）で行う。
AIは音を直接聴けないため、書き出した音声を解析して判断する仕組みを別に用意する。

## DAW

ユーザーはFL Studioを持っているが、REAPERを選んだ。
FL StudioのMac向けMCPでは、プラグインの読み込みとオートメーションの作成ができない。ピアノロールへの反映にもキー送信が要る。
REAPERはReaScriptからほぼ全操作ができ、既存のMCPも多い。
代わりにFL Studio付属のプラグインはREAPERから扱えないため、音源は単体のプラグインを都度追加する。

| ツール    | 主な既存MCP                                            | 画面操作なしでできないこと                      |
| --------- | ------------------------------------------------------ | ----------------------------------------------- |
| FL Studio | joanj94/fl-studio-mcp、calvinw/fl-studio-mcp           | Macでのプラグイン読み込み、オートメーション作成 |
| REAPER    | shiehn/total-reaper-mcp、TwelveTake-Studios/reaper-mcp | 目立った欠落はない                              |

REAPERの操作は、既存のMCPを使わずに公式のReaScript APIを直接呼ぶ。
2026-10-10にtotal-reaper-mcpのREADMEを確認した。このMCPもReaScriptを呼ぶLuaのブリッジで、REAPERの起動設定にブリッジを常駐させ、MCPクライアントからの対話で使う作りである。
エフェクトのつまみをdBやHzの表示値で指定する方法はREADMEに書かれていない。
同じ日にTwelveTake-Studios/reaper-mcpのREADMEも確認した。こちらもREAPER内のLuaブリッジとファイルでやり取りし、書き出しの機能を持つ。
ただしサーバーはMCPクライアントの接続を待つ作りで、シェルから個々の操作を呼ぶ方法は書かれていない。表示値で指定できるのはReaEQの帯域だけで、Surge XTなど他のプラグインは未記載である。
制作では、シェルから決まった手順で何度でも同じ結果を書き出すことと、表示値でつまみを合わせることが要る。
そこで公式の起動オプション（-nonewinst）でLuaスクリプトをREAPERに渡し、表示値はTrackFX_FormatParamValueNormalizedで探して合わせる。

Surge XTの音色は、songs/001-mou-ikkai/04-reaper/surge.luaが.fxpの中身をREAPER公式APIのvst_chunkに渡して読み込む。
2026-10-10に代わりの手段を調べた。REAPERのTrackFX_SetPresetは、VST3ではファイルとして.vstpresetしか受け付けず、Surge XTの音色ファイルは独自の.fxpである。
Surge XT 1.3のOSCには/patch/loadがある。ただし受信は楽器ごとの設定で有効にし、ポートも割り当てる必要がある。読み込みは非同期の予約である。
ReaPackにも、Surge XTの.fxpをトラックへ読み込むスクリプトは見つからなかった。
読み込む中身は、.fxpの先頭60バイトを除いた残りである。60バイトは、Surge XTが.fxpを保存するときに書くfxChunkSetCustomの大きさである（src/common/SurgeSynthesizerIO.cpp）。内訳は4バイトの値7つ、28バイトの音色名、4バイトの中身の長さである。
vst_chunkに渡す枠（中身の長さ、1、中身、8バイトの0）は、2026-10-06にREAPERが書き出したSurge XTの状態と同じ形に合わせたもので、REAPERの仕様書には書かれていない。
REAPERのAPIにはbase64の関数が無い。2026-10-10に、手書きしていたエンコーダーをmacOS標準のbase64コマンドの呼び出しに置き換えた。純Luaのiskolbin/lbase64（パブリックドメイン）も候補だったが、リポジトリへ取り込む必要がない標準コマンドを選んだ。

## 歌声

候補はOpenUtauとSynthesizer V Studio 2である。
OpenUtauのプロジェクト（.ustx）はYAMLなので、ノート、歌詞、ピッチ、表情曲線まで全部をファイルとして書ける。
公式には画面なしで書き出す機能がない。
既存のOpenUtau-HeadlessはOpenUtau全体のフォークで、コマンドは書き出しと歌手一覧だけである。ピッチの自動生成や音素の書き出しはできない。
制作ではこの2つを使う。songs/001-mou-ikkai/03-vocal/render.shは毎回--phonemesで音素を書き出して「っ」の扱いなどを検査し、PITCHを指定したときは--pitchでピッチを自動生成してから書き出す。
turboegg1145/OpenUtau-MCPは.ustxの生成と表情曲線を扱うが、音声のプレビューは独自の合成で、OpenUtau本体の書き出しは使わない。
OpenUtau Issue #1615は画面なしの書き出しを求める要望である。報告者はOpenUtau.CoreのPlaybackManager.RenderToFilesを呼んだが、画面側のスレッドを前提とする処理で詰まった。返答のないまま「not planned」で閉じられた。
そこでOpenUtau本体の固定したコミットを参照するtools/openutau-renderを作った。Issueと同じRenderToFilesを、自前の処理ループを画面スレッドの代わりに渡して呼び、画面の「Export Wav」と同じ処理で書き出す。
2026-10-06の追加時のコミットには画面なしのレンダラーが無かったと書いたが、OpenUtau-Headlessは2026-10-04に見つけていた。上の理由で使わない。
Synthesizer Vはtatat/svs-mcpでノート、歌詞、音素を入力できる。
歌手の選択にはスクリプトAPIがなく、スクリプトから書き出せるかも未確認である。
まずOpenUtauで全工程を通し、歌声の質が足りなければSynthesizer Vに切り替える。

## 映像

RemotionはReactのコードで映像を作るため、そのままAIが扱える。
3Dの演出にはBlenderのPython APIを使う。

## 分析wikiの閲覧と検索

apps/wikiは、docs/wiki/のMarkdownを読んで日本語の自然文で意味検索するアプリである。
要件は、ページを足すだけで一覧と検索に入ること、手書きの索引を持たないこと、Cloudflare Workers上でAlchemyからデプロイできることである。
2026-10-04の作成時に検討の記録が残っていたのは、Cloudflare AI SearchとVectorizeの2つだけだった。
2026-10-10に、静的wiki生成器と検索ライブラリも公式ドキュメントで確認した。

Starlightの既定の検索はPagefindで、ほかにAlgolia DocSearchとTypesense DocSearchのプラグインがある。
VitePressの既定の検索はMiniSearchによるあいまいな全文検索で、ほかにAlgolia DocSearchとPagefind、Typesense、Cloudflare AI Searchのプラグインがある。
Pagefindは日本語の分かち書きに対応するが、語の一致で探す全文検索であり、意味検索の機能はない。
MiniSearchとAlgolia DocSearchの既定のモードも、語で探す検索である。
Algolia DocSearchとTypesenseは外部の検索サービスに索引を同期する作りで、Workersだけでは完結しない。
VitePressのCloudflare AI Searchプラグインは、作成時にR2への本文同期の手段がないため見送ったAI Searchを使う。
したがって、どちらの生成器を選んでも日本語の意味検索は別に用意する必要があり、置き換えても自作の部分は減らない。

Oramaはベクトル検索を持ち、埋め込みを生成するプラグインはTensorFlow.jsのUniversal Sentence Encoderを使う。
このプラグインのモデルは差し替えられず、多言語の埋め込みを使うには自分で作ったベクトルを渡すことになる。
自分で渡す場合でも、Workers AIの呼び出しと節ごとの分割は自作のまま残る。
Oramaが置き換えるのはコサイン類似度と並べ替えの十数行だけで、依存を足すほどの得がないため採用しなかった。

CloudflareでWorkersからベクトル検索をする公式の手段はVectorizeである。
2026-10-10にVectorizeの公式ドキュメントとAlchemy 2.0.0-beta.81の実装を確認した。
Alchemyはインデックスを作ってWorkerに結び付けられる。
ただしベクトルの投入はWorkerの実行時にupsertを呼ぶ作りで、デプロイ時にwiki本文を入れる手段はない。
Vectorizeのinsert、upsert、deleteByIdsは非同期で、反映には通常数秒かかるとされ、上限の保証はない。
採用すると、本文の変化を検出して埋め込み直す処理、消えたページのベクトルを消す処理、反映待ちの処理を自作することになる。
その量は、今の実装で自作しているコサイン類似度と並べ替えの十数行より多い。
2026-10-10時点のwikiは25ページ、約150節で、768次元のベクトルをWorkerのメモリに持っても問題にならない。
Workers上の小規模な意味検索で、全件のコサイン計算とVectorizeのどちらを選ぶべきかを論じた記事やissueも探したが、見つからなかった。
見つかったのはVectorizeの使い方の解説だけで、メタデータでの絞り込みが要るならVectorizeを選ぶという判断材料が得られた。
今のwikiは検索結果を属性で絞り込まないため、この点でも採用の理由にならない。
ページ数が数百を超えてWorker内で全ベクトルを持つのが重くなるか、属性での絞り込みが要るようになったら、Vectorizeへの移行を改めて検討する。

## 参照

- [joanj94/fl-studio-mcp](https://github.com/joanj94/fl-studio-mcp)
- [calvinw/fl-studio-mcp](https://github.com/calvinw/fl-studio-mcp)
- [shiehn/total-reaper-mcp](https://github.com/shiehn/total-reaper-mcp)
- [TwelveTake-Studios/reaper-mcp](https://github.com/TwelveTake-Studios/reaper-mcp)
- [tatat/svs-mcp](https://github.com/tatat/svs-mcp)
- [turboegg1145/OpenUtau-MCP](https://github.com/turboegg1145/OpenUtau-MCP)
- [boxboy523/OpenUtau-Headless](https://github.com/boxboy523/OpenUtau-Headless)
- [OpenUtau Issue #1615](https://github.com/openutau/OpenUtau/issues/1615)
- [Starlight Site search](https://starlight.astro.build/guides/site-search/)
- [VitePress Search](https://vitepress.dev/reference/default-theme-search)
- [Pagefind Multilingual search](https://pagefind.app/docs/multilingual/)
- [Orama Plugin Embeddings](https://docs.orama.com/open-source/plugins/plugin-embeddings)
- [Vectorize Limits](https://developers.cloudflare.com/vectorize/platform/limits/)
- [Vectorize Client API](https://developers.cloudflare.com/vectorize/reference/client-api/)
- [jamorasep](https://pypi.org/project/jamorasep/)
- [pyloudnorm](https://github.com/csteinmetz1/pyloudnorm)

## 自作の補助ツール

2026-10-10に、tools/にある自作ツールのうち次の4つについて既存の代わりを調べた。

かなをモーラに分ける処理は、PyPIで見つけたjamorasepに置き換えた。
mora-checkとlyric-timingは拗音をつなぐ処理と母音の表を手書きしていた。
jamorasepは拗音、長音、促音、撥音をそれぞれ1モーラに分け、母音もsimple-ipaの形式で返す。
置き換えの前後で、1曲目の歌詞の検査表と歌詞タイミングのJSONは1バイトも変わらなかった。
無声化しやすい音の判定は手書きのまま残す。pyopenjtalkは話し言葉の無声化を返すが、歌では音の長さで無声化が変わり、判定の前提が合わない。

lyric-timingの残りは、score.tomlとvocal.tsvを動画用のJSONへ変える処理である。
歌詞と時刻を扱う既存の形式はLRCなどの行単位のもので、モーラごとの16分音符の位置とフックの印は表せない。
変換元と変換先はどちらもこのリポジトリ独自の形式なので、変換の部分は自作のままにする。

ustx-from-vocalはturboegg1145/OpenUtau-MCPと比べた。
OpenUtau-MCPの.ustx生成は歌詞、長さ、音高だけを読み、ピッチ点とビブラートは固定値、表情曲線は空で書き出す。
声色（clr）、ポルタメント、ビブラートの量、息継ぎの音符、DiffSingerの曲線（brec、tenc、voic、velc、dyn）は書けない。
OpenUtau本体のMIDI読み込みも同様に、ノートと歌詞以外を持ち込めない。
.ustxはYAMLなので、生成はpyyamlで書く今の形を続ける。
公式シリアライザ（Ustx.Load/Save）は生成には使わない。render.shがtools/openutau-renderの--saveでpyyamlの出力を読み込んで保存し直すので、形式の誤りはそこで止まる。生成をC#へ移すと、調声の設定を変えるたびにMacでOpenUtau本体ごとビルドが要る。

mix-metricsは、ラウドネス、ラウドネスレンジ、トゥルーピークをffmpegのebur128フィルターで測る。
pyloudnormも統合ラウドネスとラウドネスレンジは測れるが、READMEにトゥルーピークの記載がない。ebur128なら3つを1回で測れる。
周波数帯の比率、スペクトル重心、オンセット数、歌のピッチはlibrosaで求め、ツールでは比率の計算だけを書いている。
帯域の比率はessentiaのEnergyBandRatioでも求められるが、参照曲の分析と同じSTFTと帯の区切りで比べる必要があるため、librosaのSTFTから計算する形を続ける。essentiaはAGPLで、Macへの導入も別に要る。

tools/のmidi-rppとtempo-octaveは、追加時のコミットに既存の手段と比べた結果がある。midi-rppはREAPERプロジェクトの直列化に既存のrppライブラリを使い、MIDIソースを扱えないreathonは採らなかった。MIDIのイベント行の組み立てだけを自作している。
REAPER公式のInsertMediaでMIDIファイルを読み込めば、このイベント行の組み立ては不要になりうる。ただしテンポとセクションのマーカーを同時に入れられるかはMacで試していない。
tempo-octaveは、librosa、madmom、essentiaの拍追跡がテンポの候補を出すだけで、倍と半分のどちらが正しいかをドラムの役割から決める手段を持たないため自作した。
lyric-timingについて上に書いた「LRCなどは行単位」は、モーラ単位の時刻を持つ形式を検索して確かめたものではない。

## 導入済みの環境

2026-10-04にmac-miniへ導入した。

| ツール              | 版                         | 場所                        |
| ------------------- | -------------------------- | --------------------------- |
| REAPER              | 7.81                       | /Applications/REAPER.app    |
| OpenUtau            | 0.1.565（Apple Silicon版） | /Applications/OpenUtau.app  |
| Nishiren DiffSinger | v2.0                       | ~/Library/OpenUtau/Singers/ |

シンセはSurge XT 1.3.4を使う。Vitalは配布元へのログインが要るため見送った。
REAPERの操作と書き出しは、起動中のREAPERにLuaスクリプトを渡して行う。手順はsongs/001-mou-ikkai/04-reaper/にある。

## 歌声ライブラリの規約

Nishiren DiffSingerの作者はGardananaである。
非商用の利用には許可が要らず、商用の利用には作者の個別許可が要る。
公開する作品にはライブラリ名を表記する。
元音声と生成音声を許可なく機械学習に使ってはならない。
性的、暴力的、政治的、宗教的な内容には許可が要る。
作者はいつでも公開作品の取り下げを求められる。
再配布は未編集の完全な形に限られるため、ライブラリ本体はリポジトリに入れない。
