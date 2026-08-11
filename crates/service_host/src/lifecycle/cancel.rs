//! Composable cooperative cancellation without process-global signal handling.

use core::fmt;

/// Cloneable cooperative cancellation shared by supervisor-owned tasks.
///
/// Child cancellation propagates to descendants but never back to its parent
/// or sideways to siblings. This type does not install or interpret operating
/// system signals.
#[derive(Clone, Default)]
pub struct CancellationToken {
    inner: tokio_util::sync::CancellationToken,
}

impl CancellationToken {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a child that is cancelled with this token while retaining its own authority.
    #[must_use]
    pub fn child_token(&self) -> Self {
        Self {
            inner: self.inner.child_token(),
        }
    }

    /// Requests cancellation. Repeated requests have no additional effect.
    pub fn cancel(&self) {
        self.inner.cancel();
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.inner.is_cancelled()
    }

    /// Completes after cancellation and remains immediately ready thereafter.
    pub async fn cancelled(&self) {
        self.inner.cancelled().await;
    }
}

impl fmt::Debug for CancellationToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CancellationToken")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn parent_child_propagation_is_directional() {
        let parent = CancellationToken::new();
        let child = parent.child_token();
        let grandchild = child.child_token();
        let sibling = parent.child_token();

        child.cancel();
        child.cancelled().await;
        grandchild.cancelled().await;
        assert!(!parent.is_cancelled());
        assert!(!sibling.is_cancelled());

        parent.cancel();
        parent.cancelled().await;
        sibling.cancelled().await;
        assert!(parent.child_token().is_cancelled());
    }

    #[tokio::test]
    async fn cancellation_is_idempotent_and_observable_after_the_fact() {
        let token = CancellationToken::new();
        token.cancel();
        token.cancel();
        assert!(token.is_cancelled());
        token.cancelled().await;
        assert_eq!(
            format!("{token:?}"),
            "CancellationToken { cancelled: true }"
        );
    }

    #[tokio::test]
    async fn dropping_a_waiter_does_not_consume_cancellation() {
        let token = CancellationToken::new();
        let abandoned = token.cancelled();
        drop(abandoned);

        token.cancel();
        token.cancelled().await;
        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn concurrent_waiter_registration_and_cancel_has_no_lost_wakeup() {
        for round in 0..64 {
            let token = CancellationToken::new();
            let waiter_token = token.clone();
            let waiter = tokio::spawn(async move {
                if round % 2 == 0 {
                    tokio::task::yield_now().await;
                }
                waiter_token.cancelled().await;
                waiter_token.is_cancelled()
            });
            if round % 2 == 1 {
                tokio::task::yield_now().await;
            }
            token.cancel();
            assert!(waiter.await.unwrap());
        }
    }
}
