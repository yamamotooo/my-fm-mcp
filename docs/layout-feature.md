# FileMaker Layout 自動生成機能 仕様書

## 概要

Claude からの指示でフィールド情報を取得し、FileMaker のレイアウト XML を生成、
クリップボードに書き込むことで、ユーザーが Cmd+V でそのままレイアウトに貼り付けできる機能。

**設計方針:**
- Rust は IPC の橋渡しに専念（XML の知識を持たない）
- XML テンプレートの管理・組み立ては Claude スキルが担当
- ユーザーがテンプレートを自由に追加・編集できる拡張性を重視

---

## ユーザーフロー

```
1. ユーザー → Claude「フィールド一覧を取得して、レイアウトにフィールドを配置して」
2. Claude → スキル発見（filemaker-layouts/SKILL.md を読み込む）
3. Claude → Rust MCP Server → C++ Plugin → FileMaker API（get_fields）
4. Plugin → Rust → Claude へ フィールド情報 JSON を返却
5. Claude → スキルの手順に従い、テンプレートファイルを読んで XML を組み立て
6. Claude → Rust MCP Server → C++ Plugin（set_clipboard）
7. Plugin → OS クリップボードに書き込み（macOS/Windows 各 API）
8. Claude →「クリップボードにコピーしました。FileMaker で Cmd+V してください。」
9. ユーザー → FileMaker で Cmd+V → レイアウトにペースト完了
```

---

## アーキテクチャ

```
Claude Desktop
  │  スキル読み込み（SKILL.md + templates/*.xml）← XML 知識はここに集約
  │  stdin/stdout (JSON-RPC 2.0)
  ▼
mcp-server (Rust)          ← IPC 橋渡しのみ。XML を知らない
  │  macOS: Unix Domain Socket /tmp/filemaker_mcp.sock
  │  Windows: Named Pipe \\.\pipe\filemaker_mcp
  ▼
fm-plugin (C++ .fmplugin / .fmx64)
  │  macOS: NSPasteboard
  │  Windows: RegisterClipboardFormat + SetClipboardData
  ▼
FileMaker Pro（Cmd+V でペースト）
```

---

## 役割分担

| 処理                      | 担当                  |
|--------------------------|---------------------|
| フィールド情報の取得         | C++ Plugin（FM API） |
| IPC 通信                  | Rust MCP Server      |
| **XML テンプレート管理**    | **Claude スキル**     |
| **XML 組み立て**           | **Claude スキル**     |
| クリップボード書き込み       | C++ Plugin（OS API） |

---

## スキル設計

### 配置場所

```
~/.claude/skills/filemaker-layouts/   ← ユーザーが管理
├── SKILL.md                          ← Claude が読む手順書
└── templates/
    ├── layout_root.xml               ← レイアウト全体のラッパー
    ├── field_text.xml                ← Text フィールド
    ├── field_number.xml              ← Number フィールド
    ├── field_date.xml                ← Date フィールド
    ├── field_time.xml                ← Time フィールド
    ├── field_timestamp.xml           ← Timestamp フィールド
    ├── field_container.xml           ← Container フィールド
    ├── field_calc.xml                ← Calculation フィールド
    ├── label.xml                     ← フィールド名ラベル
    └── (ユーザーが自由に追加可能)
        ├── portal.xml                ← ポータル
        ├── button.xml                ← ボタン
        └── ...
```

### SKILL.md の役割

Claude がスキルを見つけると SKILL.md を読み、以下を把握する:

1. テンプレートファイルの場所とそれぞれの用途
2. フィールド型 → テンプレートファイルの対応ルール
3. XML 組み立て手順（座標計算、KEY 採番など）
4. `set_clipboard` MCP ツールへの渡し方

### SKILL.md サンプル構成

```markdown
---
name: filemaker-layouts
description: FileMaker レイアウト XML を生成してクリップボードに格納するスキル。
  「レイアウトを作って」「フィールドを配置して」などの指示があれば必ずこのスキルを使うこと。
---

## テンプレート一覧
| ファイル                | 対応フィールド型          |
|----------------------|------------------------|
| field_text.xml       | Text, Calculation      |
| field_number.xml     | Number, Summary        |
| field_date.xml       | Date                   |
| field_time.xml       | Time                   |
| field_timestamp.xml  | Timestamp              |
| field_container.xml  | Container              |

## 組み立て手順
1. get_fields で取得した JSON を確認
2. 各フィールドの type に応じてテンプレートを選択
3. 座標を計算（デフォルトルール参照）
4. label.xml + field_xxx.xml のペアを繰り返し生成
5. layout_root.xml の {{OBJECTS}} に展開
6. 完成した XML を set_clipboard に渡す

## デフォルト座標ルール
- 開始Y: 50 / フィールド高さ: 22 / ラベル高さ: 14 / 行間隔: 8
- フィールド左端: 120 / フィールド幅: 200 / ラベル左端: 10
```

### テンプレートの拡張方法

ユーザーは `templates/` に XML ファイルを追加し、SKILL.md の対応表を更新するだけで
新しいオブジェクト型（ポータル、ボタンバーなど）に対応できる。
Claude は SKILL.md を読んで自動的に新しいテンプレートを利用する。

---

## 新規実装コンポーネント（Rust / C++）

### 1. IPC コマンド: `get_fields`

#### リクエスト (Rust → Plugin)
```json
{"command": "get_fields", "args": {"table": "Contacts"}}
```
- `table`: 対象テーブル名（省略時は現在アクティブなテーブル）

#### レスポンス (Plugin → Rust → Claude)
```json
{
  "status": "ok",
  "fields": [
    {"name": "FirstName", "type": "Text",      "repetitions": 1},
    {"name": "Age",       "type": "Number",    "repetitions": 1},
    {"name": "BirthDate", "type": "Date",      "repetitions": 1},
    {"name": "Photo",     "type": "Container", "repetitions": 1}
  ]
}
```

#### C++ 実装方針
- `FieldNames(Get(FileName))` で全フィールド名を取得（CR 区切り）
- 各フィールドに対し `FieldType(Get(FileName); フィールド名)` で型を取得
- `kFMXT_Idle` 内で処理（FM 主スレッドのみ API 呼び出し可）
- タイムアウト: 5 秒（macOS/Linux）

#### FileMaker フィールド型の対応表
| FM 型文字列    | 意味           |
|--------------|---------------|
| `Text`       | テキスト        |
| `Number`     | 数字           |
| `Date`       | 日付           |
| `Time`       | 時刻           |
| `Timestamp`  | タイムスタンプ   |
| `Container`  | コンテナ        |
| `Calculation`| 計算           |
| `Summary`    | 集計           |

---

### 2. IPC コマンド: `set_clipboard`

#### リクエスト (Rust → Plugin)
```json
{"command": "set_clipboard", "args": {"xml": "<FMObjectTransfer>...</FMObjectTransfer>"}}
```
- XML の組み立ては Claude スキルが済ませてから渡す
- Rust は XML の内容を解釈しない

#### レスポンス (Plugin → Rust)
```json
{"status": "ok",    "message": "クリップボードにコピーしました"}
{"status": "error", "message": "エラー内容"}
```

#### C++ 実装方針（macOS）
```objc
NSPasteboard* pb = [NSPasteboard generalPasteboard];
[pb clearContents];
NSData* data = [xml dataUsingEncoding:NSUTF8StringEncoding];
[pb setData:data forType:@"dyn.ah62d4rv4gk8zuxnqgk"];
// dyn.ah62d4rv4gk8zuxnqgk = Layout Object (.fmp12)
```

#### C++ 実装方針（Windows）
```cpp
UINT fmt = RegisterClipboardFormat(L"Mac-XML2");
// "Mac-XML2" = Layout Object (.fmp12) の Windows クリップボード形式名

OpenClipboard(NULL);
EmptyClipboard();

// データ先頭に 4 バイトのヘッダが必要（xmlpaste_windows.go の start := 4 より）
// ヘッダの内容: 現時点では 0x00000000 で試験予定
size_t dataSize = 4 + xmlBytes.size();
HGLOBAL hMem = GlobalAlloc(GMEM_MOVEABLE, dataSize);
LPVOID ptr = GlobalLock(hMem);
memset(ptr, 0, 4);
memcpy((char*)ptr + 4, xmlBytes.data(), xmlBytes.size());
GlobalUnlock(hMem);

SetClipboardData(fmt, hMem);
CloseClipboard();
```

> **⚠️ 要検証**: Windows の 4 バイトヘッダの正確な内容は
> FileMaker で実際にコピーしたデータを xmlpaste で取り出して確認すること。

---

### 3. MCP ツール定義（Rust 側）

Rust は XML を解釈しない。`get_fields` と `set_clipboard` の橋渡しに徹する。

#### `get_fields` ツール定義
```json
{
  "name": "get_fields",
  "description": "指定テーブルのフィールド名・型一覧を取得する",
  "inputSchema": {
    "type": "object",
    "properties": {
      "table": {"type": "string", "description": "対象テーブル名"}
    },
    "required": ["table"]
  }
}
```

#### `set_clipboard` ツール定義
```json
{
  "name": "set_clipboard",
  "description": "FileMaker レイアウト XML をクリップボードに書き込む。FileMaker で Cmd+V でペースト可能になる。",
  "inputSchema": {
    "type": "object",
    "properties": {
      "xml": {"type": "string", "description": "FileMaker レイアウト XML 文字列"}
    },
    "required": ["xml"]
  }
}
```

---

## クリップボード識別コード（参考: xmlpaste より）

### macOS（NSPasteboard の UTI）
| オブジェクト               | UTI                          |
|--------------------------|------------------------------|
| Layout Object (.fmp12)   | `dyn.ah62d4rv4gk8zuxnqgk`   |
| Table                    | `dyn.ah62d4rv4gk8zuxnykk`   |
| Field                    | `dyn.ah62d4rv4gk8zuxngku`   |
| Script                   | `dyn.ah62d4rv4gk8zuxnxkq`   |
| Script Step              | `dyn.ah62d4rv4gk8zuxnxnq`   |
| Custom Function          | `dyn.ah62d4rv4gk8zuxngm2`   |
| Value List               | `dyn.ah62d4rv4gk8zuxn0mu`   |

### Windows（RegisterClipboardFormat の形式名）
| オブジェクト               | 形式名        |
|--------------------------|-------------|
| Layout Object (.fmp12)   | `Mac-XML2`  |
| Table                    | `Mac-XMTB`  |
| Field                    | `Mac-XMFD`  |
| Script                   | `Mac-XMSC`  |
| Script Step              | `Mac-XMSS`  |
| Custom Function          | `Mac-XMFN`  |
| Value List               | `Mac-XMVL`  |

> Windows は `GetClipboardData` 取得時に先頭 4 バイトをスキップして本体 XML を読む。
> 書き込み時も先頭 4 バイトのヘッダが必要（内容は要検証）。

---

## 実装優先順位

| フェーズ | 内容                                       |
|--------|------------------------------------------|
| 1      | `get_fields` IPC コマンド（C++ + Rust）     |
| 2      | `set_clipboard` IPC コマンド（C++ + Rust）  |
| 3      | Layout XML テンプレート作成（実物確認後）      |
| 4      | スキル SKILL.md 作成・動作確認              |
| 5      | Windows 動作検証（4 バイトヘッダ確認）        |

> フェーズ 3 は FileMaker で手動コピー → xmlpaste で取り出した実物 XML をベースにすること。

---

## 注意事項

- **FM API は主スレッドのみ**: `kFMXT_Idle` 以外での API 呼び出し禁止
- **Rust は XML を解釈しない**: テンプレート・座標計算・組み立てはすべてスキル側
- **Layout XML の正確な構造は実物確認必須**: FileMaker で手動コピー → xmlpaste で確認
- **Windows の 4 バイトヘッダ**: 内容未確定。実物データで検証すること
- **文字コード**: macOS は UTF-8、Windows の Custom Menu のみ UTF-16LE
