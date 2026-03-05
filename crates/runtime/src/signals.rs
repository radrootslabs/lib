use core::future::Future;
use tokio::signal;
use tracing::info;

pub async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    wait_for_shutdown(ctrl_c, terminate).await;
}

async fn wait_for_shutdown<C, T>(ctrl_c: C, terminate: T)
where
    C: Future<Output = ()>,
    T: Future<Output = ()>,
{
    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    info!("Shutdown signal received, terminating...");
}

#[cfg(test)]
mod tests {
    use super::{shutdown_signal, wait_for_shutdown};
    use core::future::Future;
    use core::pin::Pin;
    use core::task::{Context, Poll};
    use std::process::Command;
    use std::time::Duration;

    struct TestFuture {
        ready: bool,
    }

    impl Future for TestFuture {
        type Output = ();

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            if self.ready {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }

    #[tokio::test]
    async fn wait_for_shutdown_returns_when_ctrl_completes() {
        wait_for_shutdown(TestFuture { ready: true }, TestFuture { ready: false }).await;
    }

    #[tokio::test]
    async fn wait_for_shutdown_returns_when_terminate_completes() {
        wait_for_shutdown(TestFuture { ready: false }, TestFuture { ready: true }).await;
    }

    #[tokio::test]
    async fn wait_for_shutdown_polls_pending_paths() {
        let handle = tokio::task::spawn(wait_for_shutdown(
            TestFuture { ready: false },
            TestFuture { ready: false },
        ));
        tokio::task::yield_now().await;
        handle.abort();
        let _ = handle.await;
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_signal_returns_on_sigterm() {
        let pid = std::process::id();
        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let status = Command::new("kill")
                .args(["-TERM", pid.to_string().as_str()])
                .status()
                .expect("run kill");
            assert!(status.success());
        });
        shutdown_signal().await;
        sender.join().expect("signal sender should join");
    }

    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn shutdown_signal_returns_on_sigint() {
        let pid = std::process::id();
        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            let status = Command::new("kill")
                .args(["-INT", pid.to_string().as_str()])
                .status()
                .expect("run kill");
            assert!(status.success());
        });
        shutdown_signal().await;
        sender.join().expect("signal sender should join");
    }
}
