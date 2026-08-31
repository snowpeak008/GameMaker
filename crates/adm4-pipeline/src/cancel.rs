//! 协作式取消信号：流水线「停止运行」的最小共享原语。

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// 段边界粒度的协作式取消信号。
///
/// 为什么是协作式而不是强杀：C1-C5 段内含 AI 调用与产物落盘，中途强杀会留下半写产物和
/// 无主的网络请求。运行器只在**每段开始前**读一次信号，因此被取消时磁盘上要么是上一段
/// 的完整产物、要么这一段什么都没写——不存在「半个阶段」的中间态。
///
/// 克隆共享同一份标志位（`Arc<AtomicBool>`），因此可以「GUI 主线程持一份负责 `cancel()`、
/// 工作线程持一份传给运行器」，两侧不需要额外加锁。
#[derive(Debug, Clone, Default)]
pub struct CancelSignal {
    flag: Arc<AtomicBool>,
}

impl CancelSignal {
    /// 新建未取消的信号。
    pub fn new() -> Self {
        Self::default()
    }

    /// 永不取消的信号：给不提供「停止」入口的调用方（CLI 单次运行、既有同步方法）用。
    /// 语义等价于 `new()`，独立命名是为了让调用点自解释「这里故意不接取消」。
    pub fn never() -> Self {
        Self::default()
    }

    /// 请求取消（幂等）：运行器会在下一个段边界停止推进。
    pub fn cancel(&self) {
        self.flag.store(true, Ordering::SeqCst);
    }

    pub fn is_cancelled(&self) -> bool {
        self.flag.load(Ordering::SeqCst)
    }

    /// 复位为未取消。同一个信号对象要复用于下一次运行时**必须**先复位，
    /// 否则新运行会在第一个段边界立刻停下。
    pub fn reset(&self) {
        self.flag.store(false, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_and_never_signals_are_not_cancelled() {
        assert!(!CancelSignal::new().is_cancelled());
        assert!(!CancelSignal::never().is_cancelled());
        assert!(!CancelSignal::default().is_cancelled());
    }

    #[test]
    fn clone_shares_the_same_flag_across_holders() {
        let owner = CancelSignal::new();
        let worker = owner.clone();
        assert!(!worker.is_cancelled());
        owner.cancel();
        assert!(worker.is_cancelled(), "克隆必须看到主线程发出的取消");
        // 幂等：重复取消不改变结果。
        owner.cancel();
        assert!(worker.is_cancelled());
    }

    #[test]
    fn reset_allows_reusing_one_signal_for_the_next_run() {
        let signal = CancelSignal::new();
        signal.cancel();
        assert!(signal.is_cancelled());
        signal.reset();
        assert!(!signal.is_cancelled(), "复位后新运行不应被上一次的取消波及");
    }

    #[test]
    fn cancel_crosses_thread_boundary() {
        let owner = CancelSignal::new();
        let worker = owner.clone();
        // 工作线程等到看见取消为止（带上限，避免实现回退成非共享时测试挂死）。
        let handle = std::thread::spawn(move || {
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
            while std::time::Instant::now() < deadline {
                if worker.is_cancelled() {
                    return true;
                }
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            false
        });
        owner.cancel();
        assert!(
            handle.join().expect("worker thread joins"),
            "工作线程应观察到主线程的取消（GUI 线程模型的最小验证）"
        );
    }
}
