use std::io::{self, BufRead, Write};
use serde_json::{json, Value};

mod ipc;

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
                    "protocolVersion": "2024-11-05",
                    "capabilities": {
                        "tools": {}
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
                            "name": "create_layout",
                            "description": "フィールド情報とレイアウト設定から FileMaker レイアウト XML を生成してクリップボードに書き込む。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "table": {
                                        "type": "string",
                                        "description": "テーブル名"
                                    },
                                    "fields": {
                                        "type": "array",
                                        "description": "get_fields で取得したフィールド配列（name, id を含む）",
                                        "items": {"type": "object"}
                                    },
                                    "config": {
                                        "type": "object",
                                        "description": "layout_config.json の内容（省略時はデフォルト値を使用）"
                                    }
                                },
                                "required": ["table", "fields"]
                            }
                        },
                        {
                            "name": "set_clipboard",
                            "description": "FileMaker レイアウト XML をクリップボードに書き込む。FileMaker で Cmd+V でペースト可能になる。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "xml": {
                                        "type": "string",
                                        "description": "FileMaker レイアウト XML 文字列"
                                    }
                                },
                                "required": ["xml"]
                            }
                        },
                        {
                            "name": "get_fields",
                            "description": "指定テーブルのフィールド名・型一覧を取得する。",
                            "inputSchema": {
                                "type": "object",
                                "properties": {
                                    "table": {
                                        "type": "string",
                                        "description": "対象テーブル名"
                                    }
                                },
                                "required": ["table"]
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

        "create_layout" => {
            let table  = args["table"].as_str().unwrap_or("");
            let empty  = vec![];
            let fields: Vec<&Value> = args["fields"].as_array()
                .unwrap_or(&empty)
                .iter()
                .collect();
            let cfg = LayoutConfig::from_json(&args["config"]);
            let xml = build_layout_xml(table, &fields, &cfg);

            match ipc::send_to_plugin("set_clipboard", &json!({ "xml": xml })) {
                Ok(resp) => {
                    let text = if resp["status"] == "ok" {
                        "クリップボードにコピーしました。FileMaker で Cmd+V でペーストしてください。".to_string()
                    } else {
                        format!("Plugin エラー: {}", resp["message"].as_str().unwrap_or("unknown error"))
                    };
                    tool_result(id, json!([{ "type": "text", "text": text }]))
                }
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("Plugin 未接続: {e}") }])),
            }
        }

        "set_clipboard" => {
            let xml = args["xml"].as_str().unwrap_or("");
            match ipc::send_to_plugin("set_clipboard", &json!({ "xml": xml })) {
                Ok(resp) => {
                    let text = if resp["status"] == "ok" {
                        resp["message"].as_str().unwrap_or("クリップボードにコピーしました").to_string()
                    } else {
                        format!("Plugin エラー: {}", resp["message"].as_str().unwrap_or("unknown error"))
                    };
                    tool_result(id, json!([{ "type": "text", "text": text }]))
                }
                Err(e) => tool_result(id, json!([{ "type": "text", "text": format!("Plugin 未接続: {e}") }])),
            }
        }

        "get_fields" => {
            let table = args["table"].as_str().unwrap_or("");
            match ipc::send_to_plugin("get_fields", &json!({ "table": table })) {
                Ok(resp) => {
                    if resp["status"] == "ok" {
                        let fields = &resp["fields"];
                        let text = format!(
                            "テーブル {} のフィールド一覧:\n{}",
                            table,
                            serde_json::to_string_pretty(fields).unwrap()
                        );
                        tool_result(id, json!([{ "type": "text", "text": text }]))
                    } else {
                        let msg = resp["message"].as_str().unwrap_or("unknown error");
                        tool_result(id, json!([{ "type": "text", "text": format!("Plugin エラー: {msg}") }]))
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

fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
     .replace('<', "&lt;")
     .replace('>', "&gt;")
     .replace('"', "&quot;")
}

struct LayoutConfig {
    start_y: f64,
    row_stride: f64,
    field_height: f64,
    label_left: f64,
    label_right: f64,
    field_left: f64,
    field_right: f64,
    label_top_offset: f64,
    label_bottom_offset: f64,
}

impl LayoutConfig {
    fn from_json(v: &Value) -> Self {
        let g = |k: &str, d: f64| v[k].as_f64().unwrap_or(d);
        Self {
            start_y:             g("start_y",             50.0),
            row_stride:          g("row_stride",          47.0),
            field_height:        g("field_height",        37.0),
            label_left:          g("label_left",          50.0),
            label_right:         g("label_right",         92.0),
            field_left:          g("field_left",         103.0),
            field_right:         g("field_right",        356.0),
            label_top_offset:    g("label_top_offset",    6.0),
            label_bottom_offset: g("label_bottom_offset", 5.0),
        }
    }
}

fn build_layout_xml(table: &str, fields: &[&Value], cfg: &LayoutConfig) -> String {
    let n = fields.len();
    let last_bottom = if n == 0 {
        cfg.start_y + cfg.field_height
    } else {
        cfg.start_y + (n - 1) as f64 * cfg.row_stride + cfg.field_height
    };

    let mut objects = String::new();

    for (i, field) in fields.iter().enumerate() {
        let name     = field["name"].as_str().unwrap_or("");
        let id       = field["id"].as_i64().unwrap_or(0);
        let ft       = cfg.start_y + i as f64 * cfg.row_stride;  // field top
        let fb       = ft + cfg.field_height;                      // field bottom
        let lt       = ft + cfg.label_top_offset;                  // label top
        let lb       = fb - cfg.label_bottom_offset;               // label bottom
        let fkey     = i * 2 + 1;
        let lkey     = i * 2 + 2;
        let ename    = xml_escape(name);
        let etable   = xml_escape(table);

        objects.push_str(&format!(
r#"    <Object type="Field" key="{fkey}" LabelKey="{lkey}" flags="0" rotation="0">
      <Bounds top="{ft:.7}" left="{fl:.7}" bottom="{fb:.7}" right="{fr:.7}"/>
      <FieldObj numOfReps="1" flags="0" inputMode="0" keyboardType="1" displayType="0" quickFind="1" pictFormat="5">
        <Name>{etable}::{ename}</Name>
        <DDRInfo>
          <Field name="{ename}" id="{id}" repetition="1" maxRepetition="1" table="{etable}"/>
        </DDRInfo>
      </FieldObj>
    </Object>
    <Object type="Text" key="{lkey}" LabelKey="0" flags="0" rotation="0">
      <Bounds top="{lt:.7}" left="{ll:.7}" bottom="{lb:.7}" right="{lr:.7}"/>
      <TextObj flags="0">
        <CharacterStyleVector>
          <Style>
            <Data>{ename}</Data>
            <CharacterStyle mask="32695">
              <Font-family codeSet="Other" fontId="4" postScript="HiraKakuProN-W3">Hiragino Kaku Gothic ProN</Font-family>
              <Font-size>16</Font-size>
              <Face>0</Face>
              <Color>#282828</Color>
            </CharacterStyle>
          </Style>
        </CharacterStyleVector>
        <ParagraphStyleVector>
          <Style>
            <Data>{ename}</Data>
            <ParagraphStyle mask="0">
</ParagraphStyle>
          </Style>
        </ParagraphStyleVector>
      </TextObj>
    </Object>
"#,
            fkey = fkey, lkey = lkey,
            ft = ft, fl = cfg.field_left, fb = fb, fr = cfg.field_right,
            lt = lt, ll = cfg.label_left, lb = lb, lr = cfg.label_right,
            etable = etable, ename = ename, id = id,
        ));
    }

    format!(
r#"<?xml version="1.0" encoding="UTF-8"?>
<fmxmlsnippet type="LayoutObjectList">
  <Layout enclosingRectTop="{sy:.7}" enclosingRectLeft="{ll:.7}" enclosingRectBottom="{lb:.7}" enclosingRectRight="{fr:.7}">
{objects}  </Layout>
</fmxmlsnippet>"#,
        sy = cfg.start_y, ll = cfg.label_left,
        lb = last_bottom, fr = cfg.field_right,
        objects = objects,
    )
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
