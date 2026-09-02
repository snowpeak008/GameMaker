//! MCP 传输层：一帧一行的收发抽象，以及两个实现。
//!
//! - [`StdioTransport`]：按 MCP stdio 规范工作——每帧一个 JSON 对象，以 `\n` 分隔，帧内无裸换行。
//!   它包装的是**任意** `Read`/`Write`，子进程只是其中一种来源：这样帧分割逻辑能用内存缓冲区
//!   确定性地测，而不必在测试里拉起真实进程。
//! - [`ScriptedTransport`]：测试替身。预置应答、记录发出的每一帧；应答用完再收就报错，
//!   而不是静默返回空帧让上层误以为「服务端没话说」。
//!
//! 传输层不解析 JSON、不认得任何方法名：那是 [`super::jsonrpc`] 与 [`super::client`] 的事。

use adm4_foundation::{Adm4Error, Adm4Result};
use std::collections::VecDeque;
use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};

/// 帧级收发接口。`send` 的入参与 `recv` 的返回都是**不含**结尾换行的单帧文本。
pub trait McpTransport {
    /// 发出一帧；帧内含裸换行必须拒绝，否则对端会把它切成两帧。
    fn send(&mut self, frame: &str) -> Adm4Result<()>;
    /// 阻塞收下一帧；对端关闭/应答耗尽是 `Err`，不得返回空串让上层误判。
    fn recv(&mut self) -> Adm4Result<String>;
}

/// stdio 传输：一行一帧。
///
/// 构造本身不拉起任何进程；要驱动子进程请显式调用 [`StdioTransport::spawn`]。
/// 这样「谁在什么时候起了什么进程」在调用方代码里一目了然，也让测试能只用内存缓冲区。
pub struct StdioTransport {
    reader: Box<dyn BufRead>,
    writer: Box<dyn Write>,
    child: Option<Child>,
}

impl StdioTransport {
    /// 用任意读写端构造（测试用 `Cursor`；生产由 [`Self::spawn`] 内部调用）。
    pub fn from_io<R: Read + 'static, W: Write + 'static>(reader: R, writer: W) -> Self {
        Self {
            reader: Box::new(BufReader::new(reader)),
            writer: Box::new(writer),
            child: None,
        }
    }

    /// 以 `cwd` 为工作目录拉起 `program args`，接管其 stdin/stdout。
    ///
    /// stderr 继承给父进程：MCP 服务端的日志走 stderr，吞掉它就丢了排障现场。
    pub fn spawn(program: &str, args: &[String], cwd: &Path) -> Adm4Result<Self> {
        let mut child = Command::new(program)
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .map_err(|error| {
                Adm4Error::io(format!(
                    "拉起 MCP 服务端失败：program={program} args={args:?} cwd={}：{error}",
                    cwd.display()
                ))
            })?;
        let stdin = child.stdin.take().ok_or_else(|| {
            Adm4Error::internal(format!("MCP 服务端 {program} 的 stdin 未按 piped 打开"))
        })?;
        let stdout = child.stdout.take().ok_or_else(|| {
            Adm4Error::internal(format!("MCP 服务端 {program} 的 stdout 未按 piped 打开"))
        })?;
        Ok(Self {
            reader: Box::new(BufReader::new(stdout)),
            writer: Box::new(stdin),
            child: Some(child),
        })
    }

    /// 子进程 id；内存构造的传输没有子进程。
    pub fn child_id(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    /// 结束子进程（若有）。显式调用能拿到错误；`Drop` 里的兜底结束拿不到。
    pub fn close(&mut self) -> Adm4Result<()> {
        let Some(mut child) = self.child.take() else {
            return Ok(());
        };
        child
            .kill()
            .map_err(|error| Adm4Error::io(format!("结束 MCP 服务端进程失败：{error}")))?;
        child
            .wait()
            .map_err(|error| Adm4Error::io(format!("等待 MCP 服务端进程退出失败：{error}")))?;
        Ok(())
    }
}

impl Drop for StdioTransport {
    fn drop(&mut self) {
        // 传输被丢弃时不能把子进程留成孤儿；这里拿不到错误出口，所以只做尽力清理。
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

impl McpTransport for StdioTransport {
    fn send(&mut self, frame: &str) -> Adm4Result<()> {
        if frame.contains('\n') {
            return Err(Adm4Error::invalid_input(format!(
                "MCP 帧含裸换行，会被对端切成多帧：{frame:?}"
            )));
        }
        self.writer
            .write_all(frame.as_bytes())
            .and_then(|_| self.writer.write_all(b"\n"))
            .and_then(|_| self.writer.flush())
            .map_err(|error| Adm4Error::io(format!("向 MCP 服务端写帧失败：{error}")))
    }

    fn recv(&mut self) -> Adm4Result<String> {
        loop {
            let mut line = String::new();
            let read = self
                .reader
                .read_line(&mut line)
                .map_err(|error| Adm4Error::io(format!("从 MCP 服务端读帧失败：{error}")))?;
            if read == 0 {
                return Err(Adm4Error::io(
                    "MCP 服务端输出已到末尾（进程退出或关闭了 stdout），没有更多帧".to_string(),
                ));
            }
            let frame = line.trim_end_matches(['\n', '\r']);
            // 空行不是帧；有些服务端在启动时会多打一个换行，跳过它而不是交给上层去解析空串。
            if frame.is_empty() {
                continue;
            }
            return Ok(frame.to_string());
        }
    }
}

/// 脚本化传输：预置应答序列，记录发出的帧。
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ScriptedTransport {
    replies: VecDeque<String>,
    sent: Vec<String>,
}

impl ScriptedTransport {
    /// 预置应答序列：每次 `recv` 按顺序弹出一条，用完即 `Err`。
    pub fn new(replies: Vec<String>) -> Self {
        Self {
            replies: replies.into_iter().collect(),
            sent: Vec::new(),
        }
    }

    /// 迄今发出的全部帧（按顺序）。
    pub fn sent(&self) -> &[String] {
        &self.sent
    }

    /// 还没被消费的应答数；测试用它断言「脚本全部走完」。
    pub fn remaining_replies(&self) -> usize {
        self.replies.len()
    }
}

impl McpTransport for ScriptedTransport {
    fn send(&mut self, frame: &str) -> Adm4Result<()> {
        if frame.contains('\n') {
            return Err(Adm4Error::invalid_input(format!(
                "MCP 帧含裸换行，会被对端切成多帧：{frame:?}"
            )));
        }
        self.sent.push(frame.to_string());
        Ok(())
    }

    fn recv(&mut self) -> Adm4Result<String> {
        self.replies.pop_front().ok_or_else(|| {
            Adm4Error::io(format!(
                "脚本化传输的应答已用完（已发 {} 帧），没有更多帧可收",
                self.sent.len()
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::{Arc, Mutex};

    /// 测试用共享写端：传输持有一份，测试再持一份用来核对写出的字节。
    #[derive(Clone, Default)]
    struct SharedWriter(Arc<Mutex<Vec<u8>>>);

    impl SharedWriter {
        fn contents(&self) -> String {
            String::from_utf8(self.0.lock().expect("锁").clone()).expect("utf8")
        }
    }

    impl Write for SharedWriter {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0.lock().expect("锁").extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn stdio_transport_splits_frames_by_newline_and_tolerates_crlf_and_blank_lines() {
        let input = Cursor::new(b"{\"a\":1}\n{\"b\":2}\r\n\n{\"c\":3}".to_vec());
        let mut transport = StdioTransport::from_io(input, SharedWriter::default());
        assert_eq!(transport.child_id(), None, "内存构造没有子进程");
        assert_eq!(transport.recv().expect("帧 1"), "{\"a\":1}");
        assert_eq!(
            transport.recv().expect("帧 2"),
            "{\"b\":2}",
            "CRLF 应被剥掉"
        );
        assert_eq!(
            transport.recv().expect("帧 3"),
            "{\"c\":3}",
            "空行跳过；最后一帧无换行也应返回"
        );
        let eof = transport.recv().expect_err("EOF 应 Err");
        assert!(eof.message.contains("末尾"), "{}", eof.message);
    }

    #[test]
    fn stdio_transport_sends_frame_with_trailing_newline_and_rejects_bare_newline() {
        let writer = SharedWriter::default();
        let mut transport = StdioTransport::from_io(Cursor::new(Vec::new()), writer.clone());
        transport.send("{\"x\":1}").expect("发帧");
        transport.send("{\"y\":2}").expect("发帧");
        assert_eq!(writer.contents(), "{\"x\":1}\n{\"y\":2}\n");

        let error = transport.send("{\"z\":\n1}").expect_err("裸换行应拒");
        assert!(error.message.contains("裸换行"));
        assert_eq!(
            writer.contents(),
            "{\"x\":1}\n{\"y\":2}\n",
            "被拒的帧不得写出"
        );
        transport.close().expect("无子进程时 close 应为 Ok");
    }

    #[test]
    fn scripted_transport_replays_in_order_records_sent_and_errors_when_exhausted() {
        let mut transport = ScriptedTransport::new(vec!["r1".into(), "r2".into()]);
        transport.send("s1").expect("发帧");
        assert_eq!(transport.recv().expect("应答 1"), "r1");
        transport.send("s2").expect("发帧");
        assert_eq!(transport.recv().expect("应答 2"), "r2");
        assert_eq!(transport.sent(), &["s1".to_string(), "s2".to_string()]);
        assert_eq!(transport.remaining_replies(), 0);
        let error = transport.recv().expect_err("用完应 Err");
        assert!(error.message.contains("用完"), "{}", error.message);
        assert!(
            transport.send("a\nb").is_err(),
            "脚本化传输同样拒绝裸换行，保证测试与真实传输口径一致"
        );
    }
}
