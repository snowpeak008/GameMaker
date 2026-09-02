//! JSON-RPC 2.0 消息类型与帧编解码（MCP 的线级基础）。
//!
//! 只做协议层的事：把请求/通知编成一帧、把一帧解成响应/通知/请求。这里不认得任何 MCP 方法名，
//! 更不认得任何引擎——方法语义在 [`super::client`]，引擎语义在具体后端。
//!
//! 帧内不得含裸换行：stdio 传输以 `\n` 分帧，一个带裸换行的帧会被对端切成两段垃圾。
//! 编码时用 `serde_json` 紧凑输出（字符串内的换行会被转义），并在出口再校验一次，
//! 让「帧合法」成为编码函数的保证而不是调用方的自觉。

use adm4_foundation::{Adm4Error, Adm4Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// 协议版本字面量，每一帧都要带。
pub const JSONRPC_VERSION: &str = "2.0";

/// 标准错误码：方法不存在。客户端收到服务端反向请求而无法处理时用它回应。
pub const ERROR_METHOD_NOT_FOUND: i64 = -32601;

/// 客户端发出的请求（有 id，期待响应）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: u64,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcRequest {
    /// 构造一条带版本字面量的请求；`id` 由 [`IdSequence`] 分配，调用方不得自造。
    pub fn new(id: u64, method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            method: method.to_string(),
            params,
        }
    }
}

/// 通知（无 id，不期待响应）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JsonRpcNotification {
    pub jsonrpc: String,
    pub method: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub params: Option<Value>,
}

impl JsonRpcNotification {
    /// 构造一条带版本字面量的通知（无 id，对端不会回应）。
    pub fn new(method: &str, params: Option<Value>) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params,
        }
    }
}

/// 服务端错误对象。`data` 原样保留：那是对端给的现场，吞掉就没法定位（R7）。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl std::fmt::Display for JsonRpcError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.data {
            Some(data) => write!(
                formatter,
                "code={} message={} data={data}",
                self.code, self.message
            ),
            None => write!(formatter, "code={} message={}", self.code, self.message),
        }
    }
}

/// 响应。`id` 保留为原始 JSON 值：对端可能用 null 回应无法解析的请求，硬转成整数会丢掉这一事实。
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    /// 构造一条错误响应（客户端回应服务端反向请求时用）。
    pub fn error(id: Value, code: i64, message: &str) -> Self {
        Self {
            jsonrpc: JSONRPC_VERSION.to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.to_string(),
                data: None,
            }),
        }
    }
}

/// 服务端发来的一帧可能是三种之一；由 `id`/`method` 是否存在区分。
#[derive(Debug, Clone, PartialEq)]
pub enum IncomingMessage {
    Response(JsonRpcResponse),
    Notification(JsonRpcNotification),
    /// 服务端反向请求（如 ping、采样）。带 id，需要回应。
    Request {
        id: Value,
        method: String,
        params: Option<Value>,
    },
}

/// 递增 id 发生器：每个请求一个唯一 id，响应按 id 配对。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdSequence {
    last: u64,
}

impl IdSequence {
    /// 新序列，尚未发出任何 id。
    pub fn new() -> Self {
        Self::default()
    }

    /// 从 1 起递增。0 留空，避免与「默认值」混淆。
    pub fn next_id(&mut self) -> u64 {
        self.last += 1;
        self.last
    }
}

/// 把一条消息编成一帧（不含结尾 `\n`）。
pub fn encode_frame<T: Serialize>(message: &T) -> Adm4Result<String> {
    let frame = serde_json::to_string(message)
        .map_err(|error| Adm4Error::internal(format!("JSON-RPC 帧序列化失败：{error}")))?;
    if frame.contains('\n') {
        return Err(Adm4Error::internal(
            "JSON-RPC 帧含裸换行，无法按行传输".to_string(),
        ));
    }
    Ok(frame)
}

/// 把一帧解成 [`IncomingMessage`]；不是 JSON-RPC 2.0 对象就报错并带上原文。
pub fn decode_incoming(frame: &str) -> Adm4Result<IncomingMessage> {
    let value: Value = serde_json::from_str(frame).map_err(|error| {
        Adm4Error::validation(format!("JSON-RPC 帧不是合法 JSON：{error}；原文：{frame}"))
    })?;
    let object = value
        .as_object()
        .ok_or_else(|| Adm4Error::validation(format!("JSON-RPC 帧不是对象；原文：{frame}")))?;
    match object.get("jsonrpc").and_then(Value::as_str) {
        Some(JSONRPC_VERSION) => {}
        other => {
            return Err(Adm4Error::validation(format!(
                "JSON-RPC 帧版本字段应为 \"{JSONRPC_VERSION}\"，实际 {other:?}；原文：{frame}"
            )));
        }
    }
    let method = object.get("method").and_then(Value::as_str);
    let id = object.get("id").cloned();
    match (method, id) {
        (Some(method), Some(id)) => Ok(IncomingMessage::Request {
            id,
            method: method.to_string(),
            params: object.get("params").cloned(),
        }),
        (Some(method), None) => Ok(IncomingMessage::Notification(JsonRpcNotification {
            jsonrpc: JSONRPC_VERSION.to_string(),
            method: method.to_string(),
            params: object.get("params").cloned(),
        })),
        (None, _) => {
            if !object.contains_key("result") && !object.contains_key("error") {
                return Err(Adm4Error::validation(format!(
                    "JSON-RPC 帧既无 method 也无 result/error；原文：{frame}"
                )));
            }
            let response: JsonRpcResponse = serde_json::from_value(value).map_err(|error| {
                Adm4Error::validation(format!("JSON-RPC 响应结构不合法：{error}；原文：{frame}"))
            })?;
            Ok(IncomingMessage::Response(response))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn id_sequence_starts_at_one_and_increments() {
        let mut ids = IdSequence::new();
        assert_eq!(ids.next_id(), 1);
        assert_eq!(ids.next_id(), 2);
        assert_eq!(ids.next_id(), 3);
    }

    #[test]
    fn request_and_notification_encode_as_single_line_jsonrpc_2() {
        let request = JsonRpcRequest::new(7, "tools/list", Some(json!({"cursor": "a\nb"})));
        let frame = encode_frame(&request).expect("编码");
        assert!(!frame.contains('\n'), "字符串内换行必须被转义");
        let value: Value = serde_json::from_str(&frame).expect("帧是 JSON");
        assert_eq!(value["jsonrpc"], "2.0");
        assert_eq!(value["id"], 7);
        assert_eq!(value["method"], "tools/list");
        assert_eq!(value["params"]["cursor"], "a\nb");

        let notification = JsonRpcNotification::new("notifications/initialized", None);
        let frame = encode_frame(&notification).expect("编码");
        let value: Value = serde_json::from_str(&frame).expect("帧是 JSON");
        assert!(value.get("id").is_none(), "通知不得带 id");
        assert!(value.get("params").is_none(), "无参数时不输出 params 键");
    }

    #[test]
    fn decode_distinguishes_response_notification_and_request() {
        let response =
            decode_incoming(r#"{"jsonrpc":"2.0","id":1,"result":{"ok":true}}"#).expect("解码响应");
        match response {
            IncomingMessage::Response(response) => {
                assert_eq!(response.id, json!(1));
                assert_eq!(response.result, Some(json!({"ok": true})));
                assert!(response.error.is_none());
            }
            other => panic!("应为响应：{other:?}"),
        }

        let error = decode_incoming(
            r#"{"jsonrpc":"2.0","id":null,"error":{"code":-32700,"message":"parse","data":"x"}}"#,
        )
        .expect("解码错误响应");
        match error {
            IncomingMessage::Response(response) => {
                assert_eq!(response.id, Value::Null);
                let error = response.error.expect("有 error");
                assert_eq!(error.code, -32700);
                assert_eq!(error.to_string(), "code=-32700 message=parse data=\"x\"");
            }
            other => panic!("应为响应：{other:?}"),
        }

        let notification =
            decode_incoming(r#"{"jsonrpc":"2.0","method":"notifications/message","params":{}}"#)
                .expect("解码通知");
        assert_eq!(
            notification,
            IncomingMessage::Notification(JsonRpcNotification {
                jsonrpc: "2.0".into(),
                method: "notifications/message".into(),
                params: Some(json!({})),
            })
        );

        let request = decode_incoming(r#"{"jsonrpc":"2.0","id":"srv-1","method":"ping"}"#)
            .expect("解码反向请求");
        assert_eq!(
            request,
            IncomingMessage::Request {
                id: json!("srv-1"),
                method: "ping".into(),
                params: None,
            }
        );
    }

    #[test]
    fn decode_rejects_non_jsonrpc_frames_with_original_text() {
        let bad_json = decode_incoming("not json").expect_err("应 Err");
        assert!(bad_json.message.contains("not json"));

        let wrong_version =
            decode_incoming(r#"{"jsonrpc":"1.0","id":1,"result":1}"#).expect_err("版本不对应 Err");
        assert!(wrong_version.message.contains("1.0"));

        let no_shape = decode_incoming(r#"{"jsonrpc":"2.0","id":1}"#)
            .expect_err("既无 method 也无 result/error 应 Err");
        assert!(no_shape.message.contains("result/error"));

        let not_object = decode_incoming("[1,2]").expect_err("数组应 Err");
        assert!(not_object.message.contains("不是对象"));
    }
}
