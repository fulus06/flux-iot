use tokio::signal;
use tokio::sync::broadcast;
use tracing::{info, warn};

/// 关闭信号类型
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// SIGTERM - 优雅关闭
    Term,
    
    /// SIGINT - Ctrl+C
    Interrupt,
    
    /// 手动触发
    Manual,
}

/// 信号处理器
pub struct SignalHandler {
    shutdown_tx: broadcast::Sender<ShutdownSignal>,
}

impl SignalHandler {
    pub fn new() -> (Self, broadcast::Receiver<ShutdownSignal>) {
        let (tx, rx) = broadcast::channel(16);
        (Self { shutdown_tx: tx }, rx)
    }

    /// 等待关闭信号（非阻塞版本，用于生产环境）
    pub async fn wait_for_signal(&self) -> ShutdownSignal {
        // 等待广播通道的信号
        let mut rx = self.shutdown_tx.subscribe();
        rx.recv().await.unwrap_or(ShutdownSignal::Manual)
    }
    
    /// 等待系统信号（阻塞版本，用于实际部署）
    #[cfg(unix)]
    pub async fn wait_for_system_signal(&self) -> ShutdownSignal {
        use signal::unix::{signal, SignalKind};
        
        let mut sigterm = signal(SignalKind::terminate())
            .expect("Failed to install SIGTERM handler");
        let mut sigint = signal(SignalKind::interrupt())
            .expect("Failed to install SIGINT handler");

        tokio::select! {
            _ = sigterm.recv() => {
                info!("Received SIGTERM");
                let _ = self.shutdown_tx.send(ShutdownSignal::Term);
                ShutdownSignal::Term
            }
            _ = sigint.recv() => {
                info!("Received SIGINT");
                let _ = self.shutdown_tx.send(ShutdownSignal::Interrupt);
                ShutdownSignal::Interrupt
            }
        }
    }
    
    /// 等待系统信号（Windows 版本）
    #[cfg(not(unix))]
    pub async fn wait_for_system_signal(&self) -> ShutdownSignal {
        signal::ctrl_c().await.expect("Failed to listen for Ctrl+C");
        info!("Received Ctrl+C");
        let _ = self.shutdown_tx.send(ShutdownSignal::Interrupt);
        ShutdownSignal::Interrupt
    }

    /// 手动触发关闭
    pub fn trigger_shutdown(&self) {
        info!("Manual shutdown triggered");
        let _ = self.shutdown_tx.send(ShutdownSignal::Manual);
    }

    /// 订阅关闭信号
    pub fn subscribe(&self) -> broadcast::Receiver<ShutdownSignal> {
        self.shutdown_tx.subscribe()
    }
}

impl Default for SignalHandler {
    fn default() -> Self {
        Self::new().0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_signal_handler_creation() {
        let (handler, mut rx) = SignalHandler::new();
        
        // 手动触发关闭
        handler.trigger_shutdown();
        
        let signal = rx.recv().await.unwrap();
        assert_eq!(signal, ShutdownSignal::Manual);
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let (handler, _rx1) = SignalHandler::new();
        let mut rx2 = handler.subscribe();
        let mut rx3 = handler.subscribe();
        
        handler.trigger_shutdown();
        
        assert_eq!(rx2.recv().await.unwrap(), ShutdownSignal::Manual);
        assert_eq!(rx3.recv().await.unwrap(), ShutdownSignal::Manual);
    }
}
