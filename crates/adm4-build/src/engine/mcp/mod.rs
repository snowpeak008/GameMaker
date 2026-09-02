//! 通用 MCP（Model Context Protocol）stdio 客户端协议层。
//!
//! 分三层，每层只做自己的事，**零引擎语义**：
//! - [`jsonrpc`]：JSON-RPC 2.0 消息类型与帧编解码、递增 id；
//! - [`transport`]：一帧一行的收发抽象——子进程 stdio 与脚本回放两种实现；
//! - [`client`]：MCP 的 initialize / tools/list / tools/call 三个客户端动作。
//!
//! 任何走 MCP 的引擎后端都复用本层：后端只决定「拉起哪个服务端、调哪些工具、怎么解释结果」。
//! 把协议从后端里剥出来，是为了协议层能在没有任何真实服务端的情况下被确定性测试。

pub mod client;
pub mod jsonrpc;
pub mod transport;

pub use client::{
    InitializeResult, MCP_PROTOCOL_VERSION, McpClient, McpClientInfo, McpServerInfo, McpToolInfo,
    McpToolResult,
};
pub use jsonrpc::{
    IdSequence, IncomingMessage, JSONRPC_VERSION, JsonRpcError, JsonRpcNotification,
    JsonRpcRequest, JsonRpcResponse, decode_incoming, encode_frame,
};
pub use transport::{McpTransport, ScriptedTransport, StdioTransport};
