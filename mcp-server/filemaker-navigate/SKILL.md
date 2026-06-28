---
name: filemaker-navigate
description: "FileMaker Pro の機能の場所をガイドするスキル。「レイアウトを管理したい」「スクリプトを書きたい」「テーブルを定義したい」などの発話に応じ、MCP ツールでメニュー項目のハイライト・ダイアログ要素の強調表示を行う。macOS のみ。"
compatibility: macOS only. Requires FileMaker Pro and filemaker MCP server with accessibility permissions.
---

## 手順

1. ユーザー発話を下記インデックスのトリガーと照合する
2. 対応する operations ファイルを Read ツールで読み込む（相対パスで指定）
3. 読み込んだ JSON の `steps` 配列を `filemaker:run_operations` に渡す（1 回だけ呼ぶ、宣言不要）
4. ツール完了後に結果を 1 回だけ簡潔に伝える

## 操作インデックス

| トリガー（部分一致） | operations ファイル |
|---|---|
| 高度なツール、詳細ツール、設定 | operations/navigate_advanced_tools.json |
| レイアウト管理、レイアウト一覧、レイアウトを管理 | operations/navigate_layout.json |
| スクリプトを書く、スクリプトワークスペース、スクリプト編集 | operations/navigate_script_workspace.json |
| スクリプト管理、スクリプト一覧 | operations/navigate_script_manage.json |
| テーブルを定義、テーブルを作る、テーブル追加、データベース管理 | operations/define_table.json |
| キャッシュを削除、テンポラリファイルを削除 | operations/clear_cache.json |
