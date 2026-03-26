use std::future::Future;
use std::time::Duration;

use bashli_core::ExecError;

/// Run a future with a timeout. Returns `ExecError::Timeout` if the deadline is exceeded.
pub async fn with_timeout<F, T>(duration: Duration, future: F) -> Result<T, ExecError>
where
    F: Future<Output = Result<T, ExecError>>,
{
    match tokio::time::timeout(duration, future).await {
        Ok(result) => result,
        Err(_elapsed) => Err(ExecError::Timeout(duration.as_millis() as u64)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn timeout_fires_when_exceeded() {
        let result: Result<(), ExecError> = with_timeout(Duration::from_millis(10), async {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Ok(())
        })
        .await;

        match result {
            Err(ExecError::Timeout(_)) => {} // expected
            other => panic!("expected Timeout, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn timeout_passes_when_fast() {
        let result = with_timeout(Duration::from_secs(5), async { Ok(42) }).await;
        assert_eq!(result.unwrap(), 42);
    }
}
