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
制作では、シェルから決まった手順で何度でも同じ結果を書き出すことと、表示値でつまみを合わせることが要る。
そこで公式の起動オプション（-nonewinst）でLuaスクリプトをREAPERに渡し、表示値はTrackFX_FormatParamValueNormalizedで探して合わせる。

## 歌声

候補はOpenUtauとSynthesizer V Studio 2である。
OpenUtauのプロジェクト（.ustx）はYAMLなので、ノート、歌詞、ピッチ、表情曲線まで全部をファイルとして書ける。
公式には画面なしで書き出す機能がない。
既存のOpenUtau-HeadlessはOpenUtau全体のフォークで、コマンドは書き出しと歌手一覧だけである。ピッチの自動生成や音素の書き出しはできず、スターも付いていない。
そこでOpenUtau本体の固定したコミットを参照するtools/openutau-renderを作り、画面の「Export Wav」と同じ処理で書き出す。
Synthesizer Vはtatat/svs-mcpでノート、歌詞、音素を入力できる。
歌手の選択にはスクリプトAPIがなく、スクリプトから書き出せるかも未確認である。
まずOpenUtauで全工程を通し、歌声の質が足りなければSynthesizer Vに切り替える。

## 映像

RemotionはReactのコードで映像を作るため、そのままAIが扱える。
3Dの演出にはBlenderのPython APIを使う。

## 参照

- [joanj94/fl-studio-mcp](https://github.com/joanj94/fl-studio-mcp)
- [calvinw/fl-studio-mcp](https://github.com/calvinw/fl-studio-mcp)
- [shiehn/total-reaper-mcp](https://github.com/shiehn/total-reaper-mcp)
- [TwelveTake-Studios/reaper-mcp](https://github.com/TwelveTake-Studios/reaper-mcp)
- [tatat/svs-mcp](https://github.com/tatat/svs-mcp)
- [turboegg1145/OpenUtau-MCP](https://github.com/turboegg1145/OpenUtau-MCP)
- [boxboy523/OpenUtau-Headless](https://github.com/boxboy523/OpenUtau-Headless)
- [OpenUtau Issue #1615](https://github.com/openutau/OpenUtau/issues/1615)

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
