//! MCP 客户端：握手、列工具、调工具。
//!
//! 只实现 MCP 客户端侧的三个通用动作，不认得任何工具名——工具叫什么、参数是什么，由具体引擎后端
//! 决定并作为 `serde_json::Value` 透传。线上字段按 MCP 规范是 camelCase（`protocolVersion`、
//! `inputSchema`、`isError`），这里在解析时逐字段读出并落成本仓库统一的 snake_case 结构，
//! 让协议的命名习惯止步于本文件。
//!
//! 失败一律上抛（R7）：服务端错误对象、工具 `isError=true`、id 对不上、未握手就调用，
//! 都是 `Err` 且消息带工具名与对端原文。

use super::jsonrpc::{
    ERROR_METHOD_NOT_FOUND, IdSequence, IncomingMessage, JsonRpcNotification, JsonRpcRequest,
    JsonRpcResponse, decode_incoming, encode_frame,
};
use super::transport::McpTransport;
use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

/// 本客户端声明的 MCP 协议版本。
pub const MCP_PROTOCOL_VERSION: &str = "2025-06-18";

/// 握手时报给服务端的客户端身份。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpClientInfo {
    /// 客户端名，进服务端日志便于对端排障时认出是谁在调。
    pub name: String,
    /// 客户端版本。
    pub version: String,
}

/// 服务端在握手时报的身份。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpServerInfo {
    pub name: String,
    pub version: String,
}

/// 握手结果。`capabilities` 原样保留为 JSON：不同服务端能力表差异大，客户端不预设结构。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct InitializeResult {
    pub protocol_version: String,
    pub capabilities: Value,
    pub server_info: McpServerInfo,
}

/// 一个可调用的工具。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpToolInfo {
    pub name: String,
    pub description: String,
    /// 入参 JSON Schema，原样保留供后端/人核对。
    pub input_schema: Value,
}

/// 工具调用结果。`is_error=true` 的结果在 [`McpClient::call_tool`] 里已转成 `Err`，
/// 这里保留字段是为了序列化进日志时保持与线上一致。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct McpToolResult {
    pub content: Vec<Value>,
    pub is_error: bool,
}

/// MCP 客户端。泛型于传输，让同一份协议逻辑既能接子进程也能接脚本回放。
pub struct McpClient<T: McpTransport> {
    transport: T,
    ids: IdSequence,
    initialized: bool,
}

impl<T: McpTransport> McpClient<T> {
    /// 接管一条传输；此时尚未握手，任何工具调用都会被拒。
    pub fn new(transport: T) -> Self {
        Self {
            transport,
            ids: IdSequence::new(),
            initialized: false,
        }
    }

    /// 是否已完成握手（`initialize` 成功且 `notifications/initialized` 已发出）。
    pub fn is_initialized(&self) -> bool {
        self.initialized
    }

    /// 借出传输，测试用它读取脚本化传输记录的帧。
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// 可变借出传输，例如显式关闭子进程。
    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    /// 拆出传输并丢弃客户端状态；之后要再用必须重新握手。
    pub fn into_transport(self) -> T {
        self.transport
    }

    /// 握手：发 `initialize`，收结果，再发 `notifications/initialized`。
    pub fn initialize(&mut self, client_info: &McpClientInfo) -> Adm4Result<InitializeResult> {
        let params = json!({
            "protocolVersion": MCP_PROTOCOL_VERSION,
            "capabilities": {},
            "clientInfo": {
                "name": client_info.name,
                "version": client_info.version,
            },
        });
        let result = self.request("initialize", Some(params))?;
        let protocol_version = required_str(&result, "protocolVersion", "initialize")?;
        let server_info_value = result.get("serverInfo").ok_or_else(|| {
            Adm4Error::validation(format!("initialize 响应缺 serverInfo；原文：{result}"))
        })?;
        let server_info = McpServerInfo {
            name: required_str(server_info_value, "name", "initialize.serverInfo")?,
            version: optional_str(server_info_value, "version"),
        };
        let capabilities = result.get("capabilities").cloned().ok_or_else(|| {
            Adm4Error::validation(format!("initialize 响应缺 capabilities；原文：{result}"))
        })?;
        let notification = JsonRpcNotification::new("notifications/initialized", None);
        self.transport.send(&encode_frame(&notification)?)?;
        self.initialized = true;
        Ok(InitializeResult {
            protocol_version,
            capabilities,
            server_info,
        })
    }

    /// 列出全部工具（跟随分页游标直到没有 `nextCursor`）。
    pub fn list_tools(&mut self) -> Adm4Result<Vec<McpToolInfo>> {
        self.ensure_initialized("tools/list")?;
        let mut tools = Vec::new();
        let mut cursor: Option<String> = None;
        loop {
            let params = cursor.as_ref().map(|cursor| json!({ "cursor": cursor }));
            let result = self.request("tools/list", params)?;
            let items = result
                .get("tools")
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    Adm4Error::validation(format!("tools/list 响应缺 tools 数组；原文：{result}"))
                })?;
            for item in items {
                tools.push(McpToolInfo {
                    name: required_str(item, "name", "tools/list.tools[]")?,
                    description: optional_str(item, "description"),
                    input_schema: item.get("inputSchema").cloned().ok_or_else(|| {
                        Adm4Error::validation(format!(
                            "tools/list 中工具缺 inputSchema；原文：{item}"
                        ))
                    })?,
                });
            }
            match result.get("nextCursor").and_then(Value::as_str) {
                Some(next) if !next.is_empty() => cursor = Some(next.to_string()),
                _ => return Ok(tools),
            }
        }
    }

    /// 调用工具。工具报错（`isError=true`）也是 `Err`：调用方不需要再检查一个布尔。
    pub fn call_tool(&mut self, name: &str, arguments: Value) -> Adm4Result<McpToolResult> {
        self.ensure_initialized(&format!("tools/call {name}"))?;
        let params = json!({ "name": name, "arguments": arguments });
        let result = self.request("tools/call", Some(params)).map_err(|error| {
            Adm4Error::new(
                error.kind,
                format!("工具 {name} 调用失败：{}", error.message),
            )
        })?;
        let content = result
            .get("content")
            .and_then(Value::as_array)
            .cloned()
            .ok_or_else(|| {
                Adm4Error::validation(format!("工具 {name} 的响应缺 content 数组；原文：{result}"))
            })?;
        let is_error = match result.get("isError") {
            None => false,
            Some(Value::Bool(flag)) => *flag,
            Some(other) => {
                return Err(Adm4Error::validation(format!(
                    "工具 {name} 的 isError 不是布尔：{other}"
                )));
            }
        };
        if is_error {
            let detail = serde_json::to_string(&content)
                .map_err(|error| Adm4Error::internal(format!("序列化工具错误内容失败：{error}")))?;
            return Err(Adm4Error::blocked(format!(
                "工具 {name} 返回 isError=true：{detail}"
            )));
        }
        Ok(McpToolResult { content, is_error })
    }

    fn ensure_initialized(&self, operation: &str) -> Adm4Result<()> {
        if self.initialized {
            Ok(())
        } else {
            Err(Adm4Error::validation(format!(
                "MCP 客户端尚未 initialize，不能执行 {operation}"
            )))
        }
    }

    /// 发一条请求并等它的响应。途中的通知跳过；服务端反向请求回以「方法不存在」后继续等。
    fn request(&mut self, method: &str, params: Option<Value>) -> Adm4Result<Value> {
        let id = self.ids.next_id();
        let request = JsonRpcRequest::new(id, method, params);
        self.transport.send(&encode_frame(&request)?)?;
        loop {
            let frame = self.transport.recv()?;
            match decode_incoming(&frame)? {
                IncomingMessage::Notification(_) => continue,
                IncomingMessage::Request {
                    id: server_id,
                    method: server_method,
                    ..
                } => {
                    let reply = JsonRpcResponse::error(
                        server_id,
                        ERROR_METHOD_NOT_FOUND,
                        &format!("客户端不处理服务端请求 {server_method}"),
                    );
                    self.transport.send(&encode_frame(&reply)?)?;
                    continue;
                }
                IncomingMessage::Response(response) => {
                    if response.id != id {
                        return Err(Adm4Error::validation(format!(
                            "{method} 收到 id 不匹配的响应：期待 {id}，实际 {}；原文：{frame}",
                            response.id
                        )));
                    }
                    if let Some(error) = response.error {
                        return Err(Adm4Error::blocked(format!(
                            "{method} 被服务端拒绝：{error}"
                        )));
                    }
                    return response.result.ok_or_else(|| {
                        Adm4Error::validation(format!(
                            "{method} 的响应既无 result 也无 error；原文：{frame}"
                        ))
                    });
                }
            }
        }
    }
}

fn required_str(value: &Value, key: &str, where_: &str) -> Adm4Result<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            Adm4Error::validation(format!("{where_} 响应缺字符串字段 {key}；原文：{value}"))
        })
}

/// 规范里标为可选的字符串字段（如工具 `description`、服务端 `version`）：缺即空串，
/// 这不是兜底——规范允许对端不给，客户端没有理由据此拒绝。
fn optional_str(value: &Value, key: &str) -> String {
    match value.get(key).and_then(Value::as_str) {
        Some(text) => text.to_string(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::mcp::transport::ScriptedTransport;

    fn init_reply(id: u64) -> String {
        json!({
            "jsonrpc": "2.0",
            "id": id,
            "result": {
                "protocolVersion": MCP_PROTOCOL_VERSION,
                "capabilities": {"tools": {}},
                "serverInfo": {"name": "scripted", "version": "0.0.1"},
            },
        })
        .to_string()
    }

    fn client_info() -> McpClientInfo {
        McpClientInfo {
            name: "adm4".into(),
            version: "4.0.0".into(),
        }
    }

    fn parse(frame: &str) -> Value {
        serde_json::from_str(frame).expect("发出的帧应是 JSON")
    }

    #[test]
    fn full_chain_initialize_list_tools_call_tool_emits_valid_jsonrpc_with_increasing_ids() {
        let transport = ScriptedTransport::new(vec![
            init_reply(1),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {"tools": [
                    {"name": "create_thing", "description": "造一个东西", "inputSchema": {"type": "object"}},
                    {"name": "run_thing", "inputSchema": {"type": "object", "properties": {}}},
                ]},
            })
            .to_string(),
            json!({
                "jsonrpc": "2.0",
                "id": 3,
                "result": {"content": [{"type": "text", "text": "done"}]},
            })
            .to_string(),
        ]);
        let mut client = McpClient::new(transport);
        assert!(!client.is_initialized());

        let init = client.initialize(&client_info()).expect("握手");
        assert_eq!(
            init,
            InitializeResult {
                protocol_version: MCP_PROTOCOL_VERSION.into(),
                capabilities: json!({"tools": {}}),
                server_info: McpServerInfo {
                    name: "scripted".into(),
                    version: "0.0.1".into(),
                },
            }
        );
        assert!(client.is_initialized());

        let tools = client.list_tools().expect("列工具");
        assert_eq!(tools.len(), 2);
        assert_eq!(tools[0].name, "create_thing");
        assert_eq!(tools[0].description, "造一个东西");
        assert_eq!(tools[1].description, "", "description 可选");
        assert_eq!(tools[1].input_schema["type"], "object");

        let result = client
            .call_tool("create_thing", json!({"kind": "cube"}))
            .expect("调工具");
        assert_eq!(
            result,
            McpToolResult {
                content: vec![json!({"type": "text", "text": "done"})],
                is_error: false,
            }
        );

        let sent = client.transport().sent();
        assert_eq!(
            sent.len(),
            4,
            "initialize / initialized 通知 / tools/list / tools/call"
        );
        for frame in sent {
            assert!(!frame.contains('\n'));
            assert_eq!(parse(frame)["jsonrpc"], "2.0");
        }
        let initialize = parse(&sent[0]);
        assert_eq!(initialize["id"], 1);
        assert_eq!(initialize["method"], "initialize");
        assert_eq!(
            initialize["params"]["protocolVersion"],
            MCP_PROTOCOL_VERSION
        );
        assert_eq!(initialize["params"]["clientInfo"]["name"], "adm4");
        assert_eq!(initialize["params"]["clientInfo"]["version"], "4.0.0");
        assert!(initialize["params"]["capabilities"].is_object());

        let initialized = parse(&sent[1]);
        assert_eq!(initialized["method"], "notifications/initialized");
        assert!(initialized.get("id").is_none(), "通知无 id");

        let list = parse(&sent[2]);
        assert_eq!(list["id"], 2);
        assert_eq!(list["method"], "tools/list");

        let call = parse(&sent[3]);
        assert_eq!(call["id"], 3);
        assert_eq!(call["method"], "tools/call");
        assert_eq!(call["params"]["name"], "create_thing");
        assert_eq!(call["params"]["arguments"]["kind"], "cube");
        assert_eq!(client.transport().remaining_replies(), 0);
    }

    #[test]
    fn server_error_object_surfaces_with_tool_name_and_original_error() {
        let transport = ScriptedTransport::new(vec![
            init_reply(1),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "error": {"code": -32602, "message": "bad params", "data": {"field": "kind"}},
            })
            .to_string(),
        ]);
        let mut client = McpClient::new(transport);
        client.initialize(&client_info()).expect("握手");
        let error = client
            .call_tool("create_thing", json!({}))
            .expect_err("服务端 error 应 Err");
        assert!(error.message.contains("create_thing"), "{}", error.message);
        assert!(error.message.contains("-32602"), "{}", error.message);
        assert!(error.message.contains("bad params"), "{}", error.message);
        assert!(
            error.message.contains("kind"),
            "data 也要带上：{}",
            error.message
        );
    }

    #[test]
    fn tool_is_error_true_surfaces_with_tool_name_and_content() {
        let transport = ScriptedTransport::new(vec![
            init_reply(1),
            json!({
                "jsonrpc": "2.0",
                "id": 2,
                "result": {"content": [{"type": "text", "text": "编译失败：缺引用"}], "isError": true},
            })
            .to_string(),
        ]);
        let mut client = McpClient::new(transport);
        client.initialize(&client_info()).expect("握手");
        let error = client
            .call_tool("run_thing", json!({}))
            .expect_err("isError=true 应 Err");
        assert!(error.message.contains("run_thing"), "{}", error.message);
        assert!(error.message.contains("isError=true"), "{}", error.message);
        assert!(
            error.message.contains("编译失败：缺引用"),
            "{}",
            error.message
        );
    }

    #[test]
    fn calling_before_initialize_is_rejected_without_touching_transport() {
        let mut client = McpClient::new(ScriptedTransport::new(vec![]));
        let error = client
            .call_tool("anything", json!({}))
            .expect_err("未握手应 Err");
        assert!(error.message.contains("initialize"), "{}", error.message);
        assert!(error.message.contains("anything"), "{}", error.message);
        let error = client.list_tools().expect_err("未握手 list 也应 Err");
        assert!(error.message.contains("tools/list"), "{}", error.message);
        assert!(client.transport().sent().is_empty(), "未握手不得发任何帧");
    }

    #[test]
    fn request_skips_notifications_answers_server_requests_and_rejects_mismatched_ids() {
        let transport = ScriptedTransport::new(vec![
            json!({"jsonrpc": "2.0", "method": "notifications/message", "params": {"level": "info"}})
                .to_string(),
            json!({"jsonrpc": "2.0", "id": "srv-9", "method": "ping"}).to_string(),
            init_reply(1),
            json!({"jsonrpc": "2.0", "id": 99, "result": {"tools": []}}).to_string(),
        ]);
        let mut client = McpClient::new(transport);
        client
            .initialize(&client_info())
            .expect("握手应跨过通知与反向请求");
        let sent = client.transport().sent();
        assert_eq!(
            sent.len(),
            3,
            "initialize / 对 ping 的错误回应 / initialized 通知"
        );
        let ping_reply = parse(&sent[1]);
        assert_eq!(ping_reply["id"], "srv-9");
        assert_eq!(ping_reply["error"]["code"], ERROR_METHOD_NOT_FOUND);
        assert!(ping_reply.get("result").is_none());

        let error = client.list_tools().expect_err("id 不匹配应 Err");
        assert!(error.message.contains("期待 2"), "{}", error.message);
        assert!(error.message.contains("99"), "{}", error.message);
    }

    #[test]
    fn initialize_requires_protocol_version_and_server_info() {
        let transport = ScriptedTransport::new(vec![
            json!({"jsonrpc": "2.0", "id": 1, "result": {"capabilities": {}}}).to_string(),
        ]);
        let mut client = McpClient::new(transport);
        let error = client
            .initialize(&client_info())
            .expect_err("缺 protocolVersion 应 Err");
        assert!(
            error.message.contains("protocolVersion"),
            "{}",
            error.message
        );
        assert!(!client.is_initialized(), "握手失败不得标记为已初始化");
        assert_eq!(
            client.transport().sent().len(),
            1,
            "握手失败不得发 initialized 通知"
        );

        let transport = ScriptedTransport::new(vec![
            json!({"jsonrpc": "2.0", "id": 1, "result": {"protocolVersion": "x", "capabilities": {}}})
                .to_string(),
        ]);
        let mut client = McpClient::new(transport);
        let error = client
            .initialize(&client_info())
            .expect_err("缺 serverInfo 应 Err");
        assert!(error.message.contains("serverInfo"), "{}", error.message);
    }

    #[test]
    fn list_tools_follows_next_cursor_and_requires_input_schema() {
        let transport = ScriptedTransport::new(vec![
            init_reply(1),
            json!({"jsonrpc": "2.0", "id": 2, "result": {
                "tools": [{"name": "a", "inputSchema": {}}], "nextCursor": "page2"
            }})
            .to_string(),
            json!({"jsonrpc": "2.0", "id": 3, "result": {
                "tools": [{"name": "b", "inputSchema": {}}]
            }})
            .to_string(),
            json!({"jsonrpc": "2.0", "id": 4, "result": {"tools": [{"name": "no_schema"}]}})
                .to_string(),
        ]);
        let mut client = McpClient::new(transport);
        client.initialize(&client_info()).expect("握手");
        let tools = client.list_tools().expect("两页");
        assert_eq!(
            tools
                .iter()
                .map(|tool| tool.name.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "b"]
        );
        let second_page = parse(&client.transport().sent()[3]);
        assert_eq!(second_page["params"]["cursor"], "page2");

        let error = client.list_tools().expect_err("缺 inputSchema 应 Err");
        assert!(error.message.contains("inputSchema"), "{}", error.message);
        assert!(error.message.contains("no_schema"), "{}", error.message);
    }

    #[test]
    fn client_runs_over_stdio_transport_backed_by_in_memory_cursor() {
        // 用真实的 StdioTransport（内存读写端，不拉进程）跑握手 + 调工具，
        // 证明客户端与按行分帧的传输能拼在一起，而不只是各自在替身上通过。
        let server_output = format!(
            "{}\n\n{}\r\n",
            init_reply(1),
            json!({"jsonrpc": "2.0", "id": 2, "result": {"content": []}})
        );
        let transport = crate::engine::mcp::transport::StdioTransport::from_io(
            std::io::Cursor::new(server_output.into_bytes()),
            std::io::sink(),
        );
        let mut client = McpClient::new(transport);
        client.initialize(&client_info()).expect("握手");
        let result = client.call_tool("noop", json!({})).expect("调工具");
        assert!(result.content.is_empty());
        assert!(!result.is_error);
        let eof = client
            .list_tools()
            .expect_err("服务端输出耗尽应 Err 而非空帧");
        assert!(eof.message.contains("末尾"), "{}", eof.message);
    }

    #[test]
    fn wire_types_round_trip_and_read_legacy() {
        let info = McpToolInfo {
            name: "t".into(),
            description: "d".into(),
            input_schema: json!({"type": "object"}),
        };
        let json = serde_json::to_string(&info).expect("序列化");
        assert!(json.contains("input_schema"), "落盘用 snake_case：{json}");
        assert_eq!(
            serde_json::from_str::<McpToolInfo>(&json).expect("反序列化"),
            info
        );
        let legacy: McpToolResult = serde_json::from_str("{}").expect("空对象");
        assert_eq!(legacy, McpToolResult::default());
        let legacy: InitializeResult =
            serde_json::from_str(r#"{"protocol_version":"v"}"#).expect("旧档");
        assert_eq!(legacy.protocol_version, "v");
        assert_eq!(legacy.server_info, McpServerInfo::default());
    }
}
