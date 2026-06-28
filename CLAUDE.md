# FileMaker MCP プロジェクト (仮)

## ディレクトリ構成

```
filemaker-mcp-workspace/
├── CLAUDE.md
├── mcp-server/                  ← Rust (MCP Server)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              ← JSON-RPC 2.0 ハンドラ、ツール定義
│       ├── ipc.rs               ← IPC クライアント (Unix: UDS / Windows: Named Pipe)
│       ├── clipboard.rs         ← クリップボード書き込み (macOS/Windows 直接実装)
│       ├── layout_gen.rs        ← レイアウト XML 生成 (fmxmlsnippet)
│       └── ax_navigate.rs       ← macOS AX API でヘルプメニュー検索・ナビゲーション
└── fm-plugin/                   ← C++ FileMaker Plugin
    ├── Headers/FMWrapper/       ← FileMaker SDK ヘッダ
    └── FileMakerMCP/FileMakerMCP/
        └── FileMakerMCP.cpp     ← Plugin 本体
```

## アーキテクチャ

```
Claude Desktop
  │  stdin/stdout (JSON-RPC 2.0)
  ▼
mcp-server (Rust バイナリ)
  │  set_clipboard → clipboard.rs で OS API を直接呼び出し
  │                  xml パラメータ: 文字列をそのまま書き込み
  │                  file パラメータ: ファイルを読み込んで書き込み
  │                  macOS: NSPasteboard (UTI: dyn.ah62d4rv4gk8zuxnqgk)
  │                  Windows: RegisterClipboardFormat("Mac-XML2") + 4byte header
  │
  │  generate_layout → IPC で get_fields を呼び出し → layout_gen.rs で XML 生成
  │                    → ファイルに保存（返り値はパスのみ、XML はコンテキストに乗せない）
  │
  │  navigate_to_feature → ax_navigate.rs で macOS AX API を直接呼び出し
  │                        FileMaker をフォアグラウンドへ → ヘルプメニューを開く
  │                        → 検索フィールドにキーワード入力 → 先頭結果をハイライト
  │                        ※ macOS のみ。アクセシビリティ権限が必要
  │
  │  get_fields / get_tables / ping →
  │  macOS/Linux: Unix Domain Socket  /tmp/filemaker_mcp.sock
  │  Windows:     Named Pipe          \\.\pipe\filemaker_mcp
  ▼
fm-plugin (C++ .fmplugin / .fmx64)  ← FileMaker Pro に読み込まれる
```

## IPC プロトコル (JSON 改行区切り)

リクエスト (Rust → Plugin):
```json
{"command": "get_tables", "args": {}}
```

レスポンス (Plugin → Rust):
```json
{"status": "ok", "tables": ["Contacts", "Invoices", "Products"]}
{"status": "error", "message": "エラー内容"}
```

## MCP ツール一覧

| ツール名            | 処理場所              | 説明                                                                        |
|---------------------|-----------------------|-----------------------------------------------------------------------------|
| `hello_filemaker`   | IPC → Plugin          | Plugin に ping を送り疎通状態を返す                                         |
| `get_tables`        | IPC → Plugin          | 現在開いている FM ファイルのテーブル一覧                                    |
| `get_records`       | IPC → Plugin          | 指定テーブルのレコード一覧を返す                                            |
| `debug_eval`        | IPC → Plugin          | 任意の FileMaker 計算式を評価して結果を返す（デバッグ用）                   |
| `get_fields`        | IPC → Plugin          | 指定テーブルのフィールド名・型一覧を取得                                    |
| `set_clipboard`     | Rust 直接             | `xml` または `file` で指定した FileMaker レイアウト XML をクリップボードに書き込む |
| `generate_layout`   | IPC → Plugin + Rust   | `get_fields` の結果からレイアウト XML を生成してファイルに保存              |
| `navigate_to_feature` | Rust 直接 (macOS AX) | `keyword` でヘルプ検索し `menu_item` に一致する行をハイライト。レスポンスに候補一覧を含む |

### generate_layout のワークフロー

```
generate_layout [table=...] [output_file=...]
  → IPC で get_fields を呼び出し
  → layout_gen::generate() で fmxmlsnippet XML を生成
  → /tmp/filemaker_layout_<table>.xml に保存（output_file 省略時）
  → 返り値: "テーブル名 の N フィールドを生成。\nファイル: /tmp/..."

set_clipboard --file=/tmp/filemaker_layout_<table>.xml
  → FileMaker で Cmd+V してレイアウトにペースト
```

XML はファイル経由で受け渡すため、会話コンテキストにトークンを消費しない。

### navigate_to_feature のワークフロー

```
navigate_to_feature { keyword: "レイアウト管理", menu_item: "レイアウトの管理" }
  → FileMaker をフォアグラウンドへ
  → ヘルプメニューを開く
  → 検索フィールドに keyword を入力
  → 結果テーブルの行から menu_item に部分一致する行を選択してハイライト
    （一致なしは先頭行にフォールバック）
  → レスポンス例:
    「レイアウト管理」で検索し「レイアウトの管理」をハイライトしました。
    候補: レイアウトの管理 / 新しいレイアウト / レイアウトの削除

menu_item を省略すると先頭行を選択する（候補確認モード）。
候補一覧がレスポンスに含まれるので、2回目の呼び出しで正確な menu_item を指定できる。
```

行のテキスト抽出は `ax_navigate.rs::ax_row_text()` が AXTitle / AXValue →
子要素 → 孫要素の順に試みるため、AX ツリーの深さに依存しない。

## 実装状況

### Rust 側（実装済み）

| ツール              | 状態   | 備考                                              |
|---------------------|--------|---------------------------------------------------|
| `hello_filemaker`   | ✅     |                                                   |
| `get_tables`        | ✅     |                                                   |
| `get_records`       | ✅     |                                                   |
| `debug_eval`        | ✅     |                                                   |
| `get_fields`        | ✅     |                                                   |
| `set_clipboard`     | ✅     | `xml` または `file` パラメータで入力              |
| `generate_layout`   | ✅     | IPC → layout_gen → ファイル保存（XML はコンテキスト非通過）|
| `navigate_to_feature` | ✅   | macOS AX API → ヘルプ検索 → メニューハイライト（macOS のみ）|

### C++ Plugin 側（IPC コマンド）

| command        | 状態   | 説明                                                              |
|----------------|--------|-------------------------------------------------------------------|
| `ping`         | ✅     | `{"status":"ok","message":"pong"}` を返す                        |
| `get_tables`   | ✅     | `TableNames(Get(FileName))` を評価してテーブル一覧を返す          |
| `get_records`  | ✅     | 指定テーブルのレコード一覧を返す                                  |
| `evaluate`     | ✅     | 任意の計算式を評価して結果を返す                                  |
| `get_fields`   | ✅     | `ExecuteSQL` で `FileMaker_BaseTableFields` からフィールド名・型・繰り返し数を取得 |

`get_fields` の C++ 実装（`GetFieldsJSON`）:
- `ExecuteFileSQLTextResult` で `FileMaker_BaseTableFields` を `FileMaker_Tables` とサブクエリ結合し、テーブルオカレンス名からベーステーブルのフィールド一覧を取得
- 引数 `table`（テーブルオカレンス名）が空の場合は `Get(LayoutTableName)` で現在のレイアウトのテーブルを使用
- `kFMXT_Idle` 内で処理（FM 主スレッドのみ API 呼び出し可）
- レスポンス形式: `{"status":"ok","table":"...","fields":[{"name":"...","id":1,"type":"Text","repetitions":1}, ...]}`

## get_tables のデータフロー

```
Claude → tools/call get_tables
  → Rust: ipc::send_to_plugin("get_tables")  [Unix Socket]
    → C++ UDS スレッド: McpRequest をキューに積んで cv.wait_for(5s)
      → kFMXT_Idle (FM 主スレッド):
          ExprEnvUniquePtr::Evaluate("TableNames(Get(FileName))")
          CR 区切りリスト → JSON 配列に変換
          McpRequest.response に書き戻し、cv.notify_one()
    → C++ UDS スレッド: response を送信
  → Rust: JSON パース → テキスト整形 → Claude に返す
```

## Plugin の設計方針

- **FMX_API は FileMaker 主スレッドからのみ呼び出し可能**
- IPC スレッドはリクエストをキュー (`gRequestQueue`) に積む
- `kFMXT_Idle` でキューを消化し、`ExprEnvUniquePtr` を使って FM API を呼ぶ
- `kFMXT_Unsafe` の間は FM API 呼び出しを禁止
- Plugin 未接続時は Rust 側がエラーメッセージをそのまま返す（モックなし）
- IPC スレッドは 1 接続ずつ順番に処理（並列接続非対応）

## ビルド

### Rust MCP Server

```bash
# macOS / Linux
cd mcp-server && cargo build
cd mcp-server && cargo build --release

# Windows (Windows 機上で実行)
cd mcp-server
cargo build --release
# → target\release\filemaker-mcp.exe

# Windows 向けクロスコンパイル (Mac から)
rustup target add x86_64-pc-windows-gnu
brew install mingw-w64
cd mcp-server
cargo build --release --target x86_64-pc-windows-gnu
# → target/x86_64-pc-windows-gnu/release/filemaker-mcp.exe
```

### C++ Plugin

```bash
# macOS: Xcode で FileMakerMCP スキームをビルド
# → FileMakerMCP.fmplugin バンドルが生成される

# Linux
cd fm-plugin && bash linux_build_plugin.sh

# Windows: Visual Studio で FileMakerMCP.sln を開きビルド
# → fm-plugin\FileMakerMCP\x64\Release\FileMakerMCP.fmx64 が生成される
# ※ Windows へのファイル転送は pack_for_windows.sh で zip を作成して転送
```

## インストール・設定

### MCP Server バイナリ配置
```bash
cp mcp-server/target/release/filemaker-mcp /usr/local/bin/
```

### Claude Desktop 設定
`~/Library/Application Support/Claude/claude_desktop_config.json`:
```json
{
  "mcpServers": {
    "filemaker": {
      "command": "/usr/local/bin/filemaker-mcp"
    }
  }
}
```

### Plugin インストール
`.fmplugin` バンドルを以下にコピーして FileMaker を再起動:
```
~/Library/Application Support/FileMaker/Extensions/
```

## Plugin ID・定数

| 定数                    | 値                                            |
|-------------------------|-----------------------------------------------|
| Plugin ID               | `MCps`                                        |
| IPC パス (macOS/Linux)  | `/tmp/filemaker_mcp.sock` (Unix Domain Socket)|
| IPC パス (Windows)      | `\\.\pipe\filemaker_mcp` (Named Pipe)         |

## navigate_to_feature スキル設計

### 概要

「どこにその機能があるか」の知識は MCP サーバー側には持たせず、Claude Desktop の
プロジェクト指示（システムプロンプト）に `{ keyword, menu_item }` のペアとして記述する。
Claude はユーザーの質問から対応行を選び、**迷わず確実に** `navigate_to_feature` を呼べる。

```
ユーザー: 「レイアウトを管理したい」
  ↓ Claude がプロジェクト指示の JSON ペア対応表を参照
  ↓ { "keyword": "レイアウト管理", "menu_item": "レイアウトの管理" } を選択
  ↓ navigate_to_feature を呼び出す
  ↓ FileMaker のヘルプ検索が開き、該当メニュー項目がハイライトされる
```

`keyword` と `menu_item` を両方事前に決め打ちすることで、
検索結果の中から何を選ぶかを Claude が推測する必要がなくなる。

### プロジェクト指示サンプル

Claude Desktop の「プロジェクト」→「指示を追加」に貼るプロンプト:

---

```
ユーザーが FileMaker の機能の場所を尋ねたら、以下の手順を取ってください。

1. ユーザーの意図から機能名を特定する
2. 下記の JSON 対応表から一致する行を選ぶ
3. navigate_to_feature ツールを keyword・menu_item を指定して呼び出す
4. ツールが返したメッセージをユーザーに伝える

「どこにありますか」「使い方を教えて」「〜したい」などの表現が対象です。

## JSON 対応表

| ユーザーが言いそうなこと               | keyword              | menu_item            |
|----------------------------------------|----------------------|----------------------|
| レイアウトを管理したい、レイアウト一覧 | レイアウト管理       | レイアウトの管理     |
| レイアウトを作りたい、新規レイアウト   | 新しいレイアウト     | 新しいレイアウト     |
| フィールドを作りたい、フィールド定義   | フィールドの定義     | フィールドの定義     |
| スクリプトを書きたい、スクリプト作成   | スクリプトワークスペース | スクリプトワークスペース |
| リレーションを設定したい               | リレーションシップ   | リレーションシップの編集 |
| 値一覧を作りたい                       | 値一覧               | 値一覧の定義         |
| アカウントを管理したい、パスワード     | アカウントの管理     | アカウントとパスワードの管理 |
| 特権セットを変えたい                   | 特権セット           | 特権セットの編集     |
| ファイルを共有したい、ホスト           | ファイルの共有       | ネットワーク共有の設定 |
| 印刷したい、印刷設定                   | 印刷                 | 印刷の設定           |

対応表にない機能は menu_item を省略し、keyword だけ指定してください。
レスポンスの「候補:」一覧を確認して menu_item を決め、再度呼び出してください。
```

---

### テスト用プロンプト

```
# スキルなしで直接呼ぶ（候補確認モード）
navigate_to_feature ツールで keyword="レイアウト管理" を実行して

# keyword と menu_item を両方指定（確実ハイライト）
navigate_to_feature ツールで keyword="レイアウト管理" menu_item="レイアウトの管理" を実行して
```

## 注意点・既知の制約

- `ExprEnvUniquePtr` で生成した `ExprEnv` は現在のファイルコンテキストを持つため、`Get(FileName)` が空になるケースはファイルを開いていない状態のみ
- `get_tables` の応答タイムアウトは 5 秒 (macOS/Linux のみ。Windows は未設定)
- Windows: ソースファイルに日本語コメントを書く場合は UTF-8 BOM 付きで保存すること (MSVC の SJIS 誤読み対策)
- **IPC 認証なし**: 現在 UDS / Named Pipe に認証機構がないため、FileMaker Pro 起動中はローカルの同一ユーザー（macOS）または同一マシン上の誰でも接続可能。将来的にはソケットパーミッション制限やトークン検証の追加を検討すること

## デバッグ実行方法
cd mcp-server
cargo run -- rfp/operations/define_table.json





