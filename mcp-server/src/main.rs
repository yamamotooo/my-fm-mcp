use std::io::{self, BufRead, Write};
use serde_json::{json, Value};

mod clipboard;
mod ipc;
mod layout_gen;

fn main() {
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
                        let text = format!(
                            "テーブル: {} ({} 件)\n{}",
                            table,
                            records.len(),
                            serde_json::to_string_pretty(&records).unwrap()
                        );
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
