use std::io::{self, BufRead, Write};
use serde_json::{json, Value};

mod ax_navigate;
mod clipboard;
mod ipc;
mod layout_gen;

fn main() {
    let args: Vec<String> = std::env::args().collect();

    // オーバーレイ表示サブプロセスモード: --show-overlay x,y,w,h
    if args.get(1).map(|s| s.as_str()) == Some("--show-overlay") {
        if let Some(coords) = args.get(2) {
            let nums: Vec<f64> = coords.split(',')
                .filter_map(|s| s.parse().ok())
                .collect();
            if nums.len() == 4 {
                ax_navigate::run_overlay_subprocess(nums[0], nums[1], nums[2], nums[3]);
            }
        }
        return;
    }

    // デバッグ CLI モード: cargo run -- <operations.json>
    if args.len() >= 2 {
        run_debug_operations(&args[1]);
        return;
    }

    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = stdout.lock();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(e) => {
                eprintln!("[filemaker-mcp] readline error: {e}");
                break;
            }
        };

        let line = line.trim().to_string();
        if line.is_empty() {
            continue;
        }

        eprintln!("[filemaker-mcp] recv: {line}");

        let req: Value = match serde_json::from_str(&line) {
            Ok(v) => v,
            Err(e) => {
                eprintln!("[filemaker-mcp] parse error: {e}");
                let resp = json!({
                    "jsonrpc": "2.0",
                    "id": null,
                    "error": { "code": -32700, "message": "Parse error" }
                });
                writeln!(out, "{}", resp).ok();
                continue;
            }
        };

        let id = req.get("id").cloned().unwrap_or(Value::Null);
        let method = req["method"].as_str().unwrap_or("");

        // notifications/initialized はレスポンス不要
        if method == "notifications/initialized" {
            eprintln!("[filemaker-mcp] initialized notification received");
            continue;
        }

        let resp = handle(&req, id, method);
        let resp_str = serde_json::to_string(&resp).unwrap();
        eprintln!("[filemaker-mcp] send: {resp_str}");
        writeln!(out, "{resp_str}").ok();
        out.flush().ok();
    }
}

fn handle(req: &Value, id: Value, method: &str) -> Value {
    match method {
        "initialize" => {
            let client = req["params"]["clientInfo"]["name"]
                .as_str()
                .unwrap_or("unknown");
            eprintln!("[filemaker-mcp] initialize from client: {client}");
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "protocolVersion": "2025-11-25",
                    "capabilities": {
                        "tools": {},
                        "extensions": {
                            "io.modelcontextprotocol/ui": {
                                "mimeTypes": ["text/html;profile=mcp-app"]
                            }
                        }
                    },
                    "serverInfo": {
                        "name": "filemaker-mcp",
                        "version": "0.1.0"
                    }
                }
            })
        }

        "tools/list" => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "result": {
                    "tools": [
                        {
                            "name": "hello_filemaker",
                            "description": "疎通確認用。名前を渡すと挨拶を返す。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "name": {
                                        "type": "string",
                                        "description": "挨拶する相手の名前"
                                    }
                                },
                                "required": ["name"]
                            }
                        },
                        {
                            "name": "get_records",
                            "description": "指定したテーブルのレコード一覧を返す。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "table": {
                                        "type": "string",
                                        "description": "テーブル名"
                                    },
                                    "limit": {
                                        "type": "number",
                                        "description": "最大取得件数（省略時 50）"
                                    }
                                },
                                "required": ["table"]
                            }
                        },
                        {
                            "name": "debug_eval",
                            "description": "任意の FileMaker 計算式を評価して結果を返す（デバッグ用）。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "expr": {
                                        "type": "string",
                                        "description": "評価する FileMaker 計算式"
                                    }
                                },
                                "required": ["expr"]
                            }
                        },
                        {
                            "name": "get_tables",
                            "description": "現在開いている FileMaker ファイルのテーブル一覧を返す。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {},
                                "required": []
                            }
                        },
                        {
                            "name": "set_clipboard",
                            "description": "FileMaker の XML をクリップボードに書き込み、FileMaker で Cmd+V でペーストできる形式にします。xml または file のいずれかを指定してください。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "xml": {
                                        "type": "string",
                                        "description": "FileMaker 用のレイアウト XML 文字列"
                                    },
                                    "file": {
                                        "type": "string",
                                        "description": "XML を読み込むファイルパス（xml の代わりに指定可）"
                                    }
                                },
                                "required": []
                            }
                        },
                        {
                            "name": "get_fields",
                            "description": "指定テーブルのフィールド名・型一覧を取得する。table を省略すると現在 FileMaker で開いているウィンドウのテーブルを対象にする。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "table": {
                                        "type": "string",
                                        "description": "対象テーブルオカレンス名。省略時は現在のレイアウトのテーブルを使用。"
                                    }
                                },
                                "required": []
                            }
                        },
                        {
                            "name": "generate_layout",
                            "description": "get_fields の結果をもとに FileMaker レイアウト XML（ラベル＋フィールドのペア）を生成しファイルに保存する。保存先パスを set_clipboard の file パラメータに渡すとクリップボードに送れる。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "table": {
                                        "type": "string",
                                        "description": "対象テーブルオカレンス名。省略時は現在のレイアウトのテーブルを使用。"
                                    },
                                    "output_file": {
                                        "type": "string",
                                        "description": "保存先ファイルパス。省略時は /tmp/filemaker_layout_<table>.xml に自動保存。"
                                    }
                                },
                                "required": []
                            }
                        },
                        {
                            "name": "run_operations",
                            "description": "operations JSON の steps 配列を受け取り、navigate_to_feature / await_for_user_interaction / highlight_to_feature を Rust が直接順番に実行する。Claude の推論を挟まないため高速。Claude が operations ファイルを Read して steps 配列をそのまま渡す。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "steps": {
                                        "type": "array",
                                        "description": "operations JSON の steps フィールドの配列をそのまま渡す",
                                        "items": { "type": "object" }
                                    }
                                },
                                "required": ["steps"]
                            }
                        },
                        {
                            "name": "navigate_to_feature",
                            "description": "FileMaker のヘルプメニューに keyword を入力して検索し、結果の中から menu_item に一致する行をハイライトして機能の場所をユーザーに示す。macOS のアクセシビリティ API を使用するため、システム設定でのアクセシビリティ権限が必要。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "keyword": {
                                        "type": "string",
                                        "description": "ヘルプ検索フィールドに入力するキーワード（例: 'レイアウト管理'）"
                                    },
                                    "menu_item": {
                                        "type": "string",
                                        "description": "検索結果の中からハイライトしたいメニュー項目のテキスト（例: 'レイアウトの管理'）。省略または空の場合は先頭の結果を選択する。"
                                    }
                                },
                                "required": ["keyword"]
                            }
                        },
                        {
                            "name": "await_for_user_interaction",
                            "description": "FileMaker の特定要素の出現またはダイアログの出現を待機する。element_name を指定すると全ウィンドウからその要素が見つかるまで待機（モード切り替え検出に使用）。element_name が空の場合は従来通りダイアログ出現を待機する。macOS のアクセシビリティ API を使用。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "element_name": {
                                        "type": "string",
                                        "description": "出現を待つ要素名（例: 'レイアウトの終了'）。指定するとダイアログでなく要素の出現を待機する。"
                                    },
                                    "dialog_title": {
                                        "type": "string",
                                        "description": "待機対象ダイアログのタイトル（部分一致）。element_name が空の場合に使用。空なら任意のダイアログを検出する。"
                                    },
                                    "timeout_sec": {
                                        "type": "number",
                                        "description": "タイムアウト秒数（デフォルト 30）"
                                    }
                                },
                                "required": []
                            }
                        },
                        {
                            "name": "click_element",
                            "description": "FileMaker から要素を名前で検索しクリックする。panel を指定するとそのパネル内のみ検索する（誤検出防止）。macOS のアクセシビリティ API を使用。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "element_name": {
                                        "type": "string",
                                        "description": "クリックする要素名（AXTitle / AXValue で部分一致）"
                                    },
                                    "element_type": {
                                        "type": "string",
                                        "description": "要素の種類（例: 'ボタン', 'テキストフィールド'）。省略時は種類を問わない。AXRole 文字列でも指定可。"
                                    },
                                    "panel": {
                                        "type": "string",
                                        "enum": ["object_panel", "inspector_panel"],
                                        "description": "検索対象パネル。object_panel=左フィールド/オブジェクト/アドオンパネル、inspector_panel=右インスペクタパネル。省略時は全ウィンドウを検索。"
                                    }
                                },
                                "required": ["element_name"]
                            }
                        },
                        {
                            "name": "highlight_to_feature",
                            "description": "FileMaker のウィンドウ（ダイアログ優先、なければ通常ウィンドウ）内の要素を Accessibility API で探し、その周囲を赤枠で 3 秒間強調表示する。tab_name を指定するとタブを切り替えてから検索する。macOS のアクセシビリティ API を使用。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "tab_name": {
                                        "type": "string",
                                        "description": "切り替えるタブ名（例: 'テーブル'）。省略時はタブ切り替えをしない。"
                                    },
                                    "element_name": {
                                        "type": "string",
                                        "description": "強調表示する要素名（AXTitle / AXDescription で部分一致）"
                                    },
                                    "element_type": {
                                        "type": "string",
                                        "description": "要素の種類（例: 'ボタン', 'テキストフィールド', 'チェックボックス'）。省略時は種類を問わず名前で検索。AXRole 文字列（'AXButton' など）でも指定可。"
                                    }
                                },
                                "required": ["element_name"]
                            }
                        }
                    ]
                }
            })
        }

        "tools/call" => {
            let tool_name = req["params"]["name"].as_str().unwrap_or("");
            let args = &req["params"]["arguments"];
            dispatch_tool(id, tool_name, args)
        }

        _ => {
            json!({
                "jsonrpc": "2.0",
                "id": id,
                "error": {
                    "code": -32601,
                    "message": format!("Method not found: {method}")
                }
            })
        }
    }
}

fn dispatch_tool(id: Value, name: &str, args: &Value) -> Value {
    match name {
        "hello_filemaker" => {
            let who = args["name"].as_str().unwrap_or("world");
            let msg = match ipc::send_to_plugin("ping", &json!({})) {
                Ok(resp) => {
                    let status = resp["status"].as_str().unwrap_or("?");
                    let message = resp["message"].as_str().unwrap_or("?");
                    format!("Hello, {who}! Plugin 応答: status={status}, message={message}")
                }
                Err(e) => format!("Hello, {who}! (Plugin 未接続: {e})"),
            };
            tool_result(id, json!([{ "type": "text", "text": msg }]))
        }

        "get_records" => {
            let table = args["table"].as_str().unwrap_or("");
            let limit = args["limit"].as_u64().unwrap_or(50);
            match ipc::send_to_plugin("get_records", &json!({ "table": table, "limit": limit.to_string() })) {
                Ok(resp) => {
                    if resp["status"] == "ok" {
                        let records = resp["records"].as_array().cloned().unwrap_or_default();
                        let fields = resp["fields"].as_array().cloned().unwrap_or_default();
                        let header: Vec<&str> = fields.iter()
                            .filter_map(|f| f.as_str())
                            .collect();
                        let body = if header.is_empty() {
                            serde_json::to_string_pretty(&records).unwrap()
                        } else {
                            let rows: Vec<String> = records.iter().map(|row| {
                                let vals: Vec<String> = row.as_array()
                                    .map(|cols| cols.iter().enumerate().map(|(i, v)| {
                                        let k = header.get(i).copied().unwrap_or("?");
                                        format!("{}: {}", k, v.as_str().unwrap_or(""))
                                    }).collect())
                                    .unwrap_or_default();
                                format!("{{ {} }}", vals.join(", "))
                            }).collect();
                            rows.join("\n")
                        };
                        let text = format!("テーブル: {} ({} 件)\n{}", table, records.len(), body);
                        tool_result(id, json!([{ "type": "text", "text": text }]))
                    } else {
                        let msg = resp["message"].as_str().unwrap_or("unknown error");
                        let code = resp["code"].as_i64().map(|c| format!(" (code={c})")).unwrap_or_default();
                        tool_result(id, json!([{ "type": "text", "text": format!("Plugin エラー: {msg}{code}") }]))
                    }
                }
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("Plugin 未接続: {e}") }])),
            }
        }

        "debug_eval" => {
            let expr = args["expr"].as_str().unwrap_or("");
            match ipc::send_to_plugin("evaluate", &json!({ "expr": expr })) {
                Ok(resp) => {
                    let text = if resp["status"] == "ok" {
                        format!("式: {expr}\n結果: {}", resp["result"].as_str().unwrap_or("(empty)"))
                    } else {
                        format!("エラー: code={}, message={}",
                            resp["code"].as_i64().unwrap_or(-1),
                            resp["message"].as_str().unwrap_or("?"))
                    };
                    tool_result(id, json!([{ "type": "text", "text": text }]))
                }
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("Plugin 未接続: {e}") }])),
            }
        }

        "get_tables" => {
            match ipc::send_to_plugin("get_tables", &json!({})) {
                Ok(resp) => {
                    let status = resp["status"].as_str().unwrap_or("error");
                    if status == "ok" {
                        let tables = &resp["tables"];
                        let text = format!(
                            "FileMaker tables:\n{}",
                            serde_json::to_string_pretty(tables).unwrap()
                        );
                        tool_result(id, json!([{ "type": "text", "text": text }]))
                    } else {
                        let msg = resp["message"].as_str().unwrap_or("unknown error");
                        tool_result(id, json!([{ "type": "text", "text": format!("Plugin エラー: {msg}") }]))
                    }
                }
                Err(e) => {
                    tool_result(id, json!([{ "type": "text", "text": format!("Plugin 未接続: {e}") }]))
                }
            }
        }

        "set_clipboard" => {
            let xml = if let Some(path) = args.get("file").and_then(|v| v.as_str()) {
                match std::fs::read_to_string(path) {
                    Ok(s) => s,
                    Err(e) => return tool_result(id, json!([{ "type": "text", "text": format!("ファイル読み込み失敗: {e}") }])),
                }
            } else {
                args["xml"].as_str().unwrap_or("").to_string()
            };
            match clipboard::set_layout_xml(&xml) {
                Ok(()) => tool_result(id, json!([{ "type": "text", "text": "クリップボードにコピーしました。FileMaker で Cmd+V してください。" }])),
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("クリップボードへの書き込み失敗: {e}") }])),
            }
        }

        "get_fields" => {
            let table = args["table"].as_str().unwrap_or("");
            match ipc::send_to_plugin("get_fields", &json!({ "table": table })) {
                Ok(resp) => {
                    if resp["status"] == "ok" {
                        tool_result(id, json!([{ "type": "text", "text": serde_json::to_string_pretty(&resp).unwrap_or_default() }]))
                    } else {
                        let msg = resp["message"].as_str().unwrap_or("unknown error");
                        let code = resp["code"].as_i64().map(|c| format!(" (code={c})")).unwrap_or_default();
                        let tbl = resp["table"].as_str().map(|t| format!("\ntable: {t}")).unwrap_or_default();
                        let base_tbl = resp["baseTable"].as_str().map(|t| format!("\nbaseTable: {t}")).unwrap_or_default();
                        let file = resp["fileName"].as_str().map(|f| format!("\nfile: {f}")).unwrap_or_default();
                        let sql = resp["sql"].as_str().map(|s| format!("\nsql: {s}")).unwrap_or_default();
                        tool_result(id, json!([{ "type": "text", "text": format!("Plugin エラー: {msg}{code}{tbl}{base_tbl}{file}{sql}") }]))
                    }
                }
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("Plugin 未接続: {e}") }])),
            }
        }

        "generate_layout" => {
            let table = args["table"].as_str().unwrap_or("");
            match ipc::send_to_plugin("get_fields", &json!({ "table": table })) {
                Ok(resp) => {
                    if resp["status"] != "ok" {
                        let msg = resp["message"].as_str().unwrap_or("unknown error");
                        return tool_result(id, json!([{ "type": "text", "text": format!("Plugin エラー: {msg}") }]));
                    }
                    let table_name = resp["table"].as_str().unwrap_or(table);
                    let fields: Vec<layout_gen::FieldInfo> = resp["fields"]
                        .as_array()
                        .unwrap_or(&vec![])
                        .iter()
                        .filter_map(|f| {
                            let name = f["name"].as_str()?;
                            Some(layout_gen::FieldInfo {
                                name: name.to_string(),
                                id: f["id"].as_u64().unwrap_or(0),
                                repetitions: f["repetitions"].as_u64().unwrap_or(1) as u32,
                            })
                        })
                        .collect();
                    if fields.is_empty() {
                        return tool_result(id, json!([{ "type": "text", "text": "フィールドが見つかりませんでした。" }]));
                    }
                    let xml = layout_gen::generate(table_name, &fields);
                    let path = args.get("output_file")
                        .and_then(|v| v.as_str())
                        .filter(|s| !s.is_empty())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("/tmp/filemaker_layout_{table_name}.xml"));
                    match std::fs::write(&path, &xml) {
                        Ok(()) => tool_result(id, json!([{ "type": "text", "text": format!("{table_name} の {} フィールドを生成しました。\nファイル: {path}", fields.len()) }])),
                        Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("ファイル書き込み失敗: {e}") }])),
                    }
                }
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("Plugin 未接続: {e}") }])),
            }
        }

        "click_element" => {
            let element_name = args["element_name"].as_str().unwrap_or("");
            if element_name.is_empty() {
                return tool_result(id, json!([{ "type": "text", "text": "element_name が空です" }]));
            }
            let element_type = args["element_type"].as_str().filter(|s| !s.is_empty());
            let panel = args["panel"].as_str().filter(|s| !s.is_empty());
            match ax_navigate::click_element(element_name, element_type, panel) {
                Ok(msg) => tool_result(id, json!([{ "type": "text", "text": msg }])),
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("クリック失敗: {e}") }])),
            }
        }

        "navigate_to_feature" => {
            let keyword = args["keyword"].as_str().unwrap_or("");
            if keyword.is_empty() {
                return tool_result(id, json!([{ "type": "text", "text": "keyword が空です" }]));
            }
            let menu_item = args["menu_item"].as_str().unwrap_or("");
            match ax_navigate::navigate_to_feature(keyword, menu_item) {
                Ok(msg) => tool_result(id, json!([{ "type": "text", "text": msg }])),
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("ナビゲーション失敗: {e}") }])),
            }
        }

        "await_for_user_interaction" => {
            let dialog_title = args["dialog_title"].as_str().unwrap_or("");
            let element_name = args["element_name"].as_str().unwrap_or("");
            let timeout_sec = args["timeout_sec"].as_u64().unwrap_or(30);
            match ax_navigate::await_for_user_interaction(dialog_title, element_name, timeout_sec) {
                Ok(msg) => tool_result(id, json!([{ "type": "text", "text": msg }])),
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("待機タイムアウト: {e}") }])),
            }
        }

        "highlight_to_feature" => {
            let tab_name = args["tab_name"].as_str().filter(|s| !s.is_empty());
            let element_name = args["element_name"].as_str().unwrap_or("");
            let element_type = args["element_type"].as_str().filter(|s| !s.is_empty());
            if element_name.is_empty() {
                return tool_result(id, json!([{ "type": "text", "text": "element_name が空です" }]));
            }
            match ax_navigate::highlight_to_feature(tab_name, element_name, element_type) {
                Ok(msg) => tool_result(id, json!([{ "type": "text", "text": msg }])),
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("強調表示失敗: {e}") }])),
            }
        }

        "run_operations" => {
            let steps = match args["steps"].as_array() {
                Some(s) => s.clone(),
                None => return tool_result(id, json!([{ "type": "text", "text": "steps が配列ではありません" }])),
            };
            match execute_operations_steps(&steps) {
                Ok(msg) => tool_result(id, json!([{ "type": "text", "text": msg }])),
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("実行エラー: {e}") }])),
            }
        }

        _ => json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32601,
                "message": format!("Unknown tool: {name}")
            }
        }),
    }
}

fn tool_result(id: Value, content: Value) -> Value {
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "result": {
            "content": content
        }
    })
}


/// operations JSON の steps を順番に実行し、各ステップの結果を改行結合して返す。
fn execute_operations_steps(steps: &[Value]) -> Result<String, String> {
    let mut results = Vec::new();
    for step in steps {
        let tool = step["tool"].as_str().unwrap_or("(unknown)");
        let args = &step["args"];
        let result = match tool {
            "click_element" => {
                let element_name = args["element_name"].as_str().unwrap_or("");
                let element_type = args["element_type"].as_str().filter(|s| !s.is_empty());
                let panel = args["panel"].as_str().filter(|s| !s.is_empty());
                ax_navigate::click_element(element_name, element_type, panel)
            }
            "navigate_to_feature" => {
                let keyword = args["keyword"].as_str().unwrap_or("");
                let menu_item = args["menu_item"].as_str().unwrap_or("");
                ax_navigate::navigate_to_feature(keyword, menu_item)
            }
            "await_for_user_interaction" => {
                let dialog_title = args["dialog_title"].as_str().unwrap_or("");
                let element_name = args["element_name"].as_str().unwrap_or("");
                let timeout_sec = args["timeout_sec"].as_u64().unwrap_or(30);
                ax_navigate::await_for_user_interaction(dialog_title, element_name, timeout_sec)
            }
            "highlight_to_feature" => {
                let tab_name = args["tab_name"].as_str().filter(|s| !s.is_empty());
                let element_name = args["element_name"].as_str().unwrap_or("");
                let element_type = args["element_type"].as_str().filter(|s| !s.is_empty());
                ax_navigate::highlight_to_feature(tab_name, element_name, element_type)
            }
            unknown => Err(format!("未知のツール: {unknown}")),
        }?;
        results.push(result);
    }
    Ok(results.join("\n"))
}

/// operations/*.json を直接実行するデバッグ CLI モード。
/// cargo run -- rfp/operations/define_table.json
fn run_debug_operations(json_path: &str) {
    let content = match std::fs::read_to_string(json_path) {
        Ok(s) => s,
        Err(e) => { eprintln!("ファイル読み込み失敗: {json_path}: {e}"); std::process::exit(1); }
    };
    let ops: Value = match serde_json::from_str(&content) {
        Ok(v) => v,
        Err(e) => { eprintln!("JSON パースエラー: {e}"); std::process::exit(1); }
    };
    let steps = match ops["steps"].as_array() {
        Some(s) => s.clone(),
        None => { eprintln!("steps フィールドがありません"); std::process::exit(1); }
    };

    println!("[debug] {} ステップを実行: {json_path}", steps.len());

    // デバッグ時は after メッセージも表示しながらステップごとに進捗を出す
    for (i, step) in steps.iter().enumerate() {
        let tool = step["tool"].as_str().unwrap_or("(unknown)");
        let args = &step["args"];
        let after = step["after"].as_str();
        println!("\n[{}/{}] {tool}  args: {args}", i + 1, steps.len());
        let single = std::slice::from_ref(step);
        match execute_operations_steps(single) {
            Ok(msg) => println!("  OK: {msg}"),
            Err(e) => { eprintln!("  ERROR: {e}"); std::process::exit(1); }
        }
        if let Some(msg) = after {
            println!("  → {msg}");
        }
    }

    println!("\n[debug] 完了");
}
