# FileMaker MCP プロジェクト (仮)

## ディレクトリ構成

```
filemaker-mcp-workspace/
├── CLAUDE.md
├── docs/
│   └── layout-feature.md        ← レイアウト自動生成機能の仕様
├── mcp-server/                  ← Rust (MCP Server)
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs              ← JSON-RPC 2.0 ハンドラ、ツール定義
│       └── ipc.rs               ← IPC クライアント (Unix: UDS / Windows: Named Pipe)
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

| ツール名           | 説明                                       |
|--------------------|--------------------------------------------|
| `hello_filemaker`  | Plugin に ping を送り疎通状態を返す        |
| `get_tables`       | 現在開いている FM ファイルのテーブル一覧   |
| `get_fields`       | 指定テーブルのフィールド名・型一覧を取得   |
| `set_clipboard`    | FileMaker レイアウト XML をクリップボードに書き込む |

## 実装済みコマンド

| command        | 説明                                              |
|----------------|---------------------------------------------------|
| `ping`         | 疎通確認。`{"status":"ok","message":"pong"}` を返す |
| `get_tables`   | `TableNames(Get(FileName))` を評価してテーブル一覧を返す |

## 実装予定コマンド

| command        | 説明                          |
|----------------|-------------------------------|
| `get_fields`   | 指定テーブルのフィールド情報を返す |
| `set_clipboard`| XML をクリップボードに書き込む   |

詳細仕様: @docs/layout-feature.md

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

## 注意点・既知の制約

- `ExprEnvUniquePtr` で生成した `ExprEnv` は現在のファイルコンテキストを持つため、`Get(FileName)` が空になるケースはファイルを開いていない状態のみ
- `get_tables` の応答タイムアウトは 5 秒 (macOS/Linux のみ。Windows は未設定)
- Windows: ソースファイルに日本語コメントを書く場合は UTF-8 BOM 付きで保存すること (MSVC の SJIS 誤読み対策)
- **IPC 認証なし**: 現在 UDS / Named Pipe に認証機構がないため、FileMaker Pro 起動中はローカルの同一ユーザー（macOS）または同一マシン上の誰でも接続可能。将来的にはソケットパーミッション制限やトークン検証の追加を検討すること
