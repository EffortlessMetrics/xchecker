//! Budgeted backend wrapper for LLM call limiting
//!
//! This module provides a wrapper around any `LlmBackend` that enforces a budget
//! limit on the number of invocations. This is primarily used for cost control
//! with HTTP providers like OpenRouter.

use crate::LlmError;
use crate::types::{LlmBackend, LlmInvocation, LlmResult};
use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, warn};

/// Default budget limit for OpenRouter calls per process
pub(crate) const DEFAULT_BUDGET_LIMIT: u32 = 20;

/// Environment variable for overriding the budget limit
pub(crate) const BUDGET_ENV_VAR: &str = "XCHECKER_OPENROUTER_BUDGET";

/// A wrapper around an `LlmBackend` that enforces a budget limit on invocations.
///
/// The budget tracks attempted calls, not successful requests. This means that
/// even if the underlying backend fails, the budget slot is consumed. This prevents
/// retry loops from bypassing budget limits.
///
/// Budget tracking is per xchecker process lifetime. Each call to `invoke` counts
/// against the limit, regardless of provider success or failure.
pub struct BudgetedBackend {
    /// The wrapped backend
    inner: Box<dyn LlmBackend>,
    /// Thread-safe counter for tracking calls
    budget: Arc<AtomicU32>,
    /// Maximum number of calls allowed
    limit: u32,
}

impl BudgetedBackend {
    /// Create a new budgeted backend with the specified limit
    ///
    /// # Arguments
    ///
    /// * `inner` - The backend to wrap
    /// * `limit` - Maximum number of invocations allowed
    pub fn new(inner: Box<dyn LlmBackend>, limit: u32) -> Self {
        debug!(limit = limit, "Creating BudgetedBackend");
        Self {
            inner,
            budget: Arc::new(AtomicU32::new(0)),
            limit,
        }
    }

    /// Create a new budgeted backend with the default limit
    ///
    /// The default limit can be overridden via the `XCHECKER_OPENROUTER_BUDGET`
    /// environment variable.
    ///
    /// # Deprecated
    ///
    /// Use `with_limit_from_config` instead for proper precedence handling.
    #[deprecated(since = "0.1.0", note = "Use with_limit_from_config instead")]
    #[allow(dead_code)] // Deprecated but kept for backwards compatibility
    pub fn with_default_limit(inner: Box<dyn LlmBackend>) -> Self {
        let limit = std::env::var(BUDGET_ENV_VAR)
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .unwrap_or(DEFAULT_BUDGET_LIMIT);

        if limit != DEFAULT_BUDGET_LIMIT {
            debug!(
                limit = limit,
                default = DEFAULT_BUDGET_LIMIT,
                "Using custom budget limit from {}",
                BUDGET_ENV_VAR
            );
        }

        Self::new(inner, limit)
    }

    /// Create a new budgeted backend with limit resolved from configuration
    ///
    /// Budget limit precedence (highest to lowest):
    /// 1. Environment variable (`XCHECKER_OPENROUTER_BUDGET`)
    /// 2. Config file (`[llm.openrouter] budget`)
    /// 3. Default (20 calls per process)
    ///
    /// # Arguments
    ///
    /// * `inner` - The backend to wrap
    /// * `config_budget` - Optional budget from config file
    ///
    /// # Examples
    ///
    /// ```ignore
    /// use async_trait::async_trait;
    /// use std::time::Duration;
    /// use xchecker_llm::{BudgetedBackend, LlmBackend, LlmError, LlmInvocation, LlmResult, Message, Role};
    ///
    /// // Define a simple mock backend for demonstration
    /// struct MockBackend;
    ///
    /// #[async_trait]
    /// impl LlmBackend for MockBackend {
    ///     async fn invoke(&self, _inv: LlmInvocation) -> Result<LlmResult, LlmError> {
    ///         Ok(LlmResult::new("mock response", "mock", "mock-model"))
    ///     }
    /// }
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     // Create a budgeted backend with a limit of 2 calls
    ///     let backend = BudgetedBackend::with_limit_from_config(
    ///         Box::new(MockBackend),
    ///         Some(2), // Config budget of 2 calls
    ///     );
    ///
    ///     let inv = LlmInvocation::new(
    ///         "test-spec",
    ///         "test-phase",
    ///         "test-model",
    ///         Duration::from_secs(60),
    ///         vec![Message::new(Role::User, "Hello")],
    ///     );
    ///
    ///     // First two calls succeed
    ///     assert!(backend.invoke(inv.clone()).await.is_ok());
    ///     assert!(backend.invoke(inv.clone()).await.is_ok());
    ///
    ///     // Third call fails with BudgetExceeded
    ///     match backend.invoke(inv).await {
    ///         Err(LlmError::BudgetExceeded { limit, attempted }) => {
    ///             assert_eq!(limit, 2);
    ///             assert_eq!(attempted, 3);
    ///         }
    ///         _ => panic!("Expected BudgetExceeded error"),
    ///     }
    /// }
    /// ```
    pub fn with_limit_from_config(inner: Box<dyn LlmBackend>, config_budget: Option<u32>) -> Self {
        // Precedence: env var > config file > default
        let limit = std::env::var(BUDGET_ENV_VAR)
            .ok()
            .and_then(|s| s.parse::<u32>().ok())
            .or(config_budget)
            .unwrap_or(DEFAULT_BUDGET_LIMIT);

        // Log the source of the budget limit
        if let Ok(env_val) = std::env::var(BUDGET_ENV_VAR) {
            if let Ok(env_limit) = env_val.parse::<u32>() {
                debug!(
                    limit = env_limit,
                    default = DEFAULT_BUDGET_LIMIT,
                    "Using budget limit from environment variable {}",
                    BUDGET_ENV_VAR
                );
            }
        } else if let Some(config_limit) = config_budget {
            debug!(
                limit = config_limit,
                default = DEFAULT_BUDGET_LIMIT,
                "Using budget limit from config file"
            );
        } else {
            debug!(limit = DEFAULT_BUDGET_LIMIT, "Using default budget limit");
        }

        Self::new(inner, limit)
    }

    /// Get the current call count
    #[cfg(test)]
    pub fn call_count(&self) -> u32 {
        self.budget.load(Ordering::SeqCst)
    }

    /// Get the budget limit
    #[cfg(test)]
    pub fn limit(&self) -> u32 {
        self.limit
    }
}

#[async_trait]
impl LlmBackend for BudgetedBackend {
    async fn invoke(&self, inv: LlmInvocation) -> Result<LlmResult, LlmError> {
        // Increment counter BEFORE calling inner backend
        // This ensures we track attempted calls, not successful requests
        let current = self.budget.fetch_add(1, Ordering::SeqCst);

        // Check if we've exceeded the limit
        if current >= self.limit {
            let attempted = current + 1;
            warn!(
                limit = self.limit,
                attempted = attempted,
                "Budget limit exceeded"
            );
            return Err(LlmError::BudgetExceeded {
                limit: self.limit,
                attempted,
            });
        }

        debug!(
            call_count = current + 1,
            limit = self.limit,
            "Budget check passed, invoking inner backend"
        );

        // Call the inner backend
        let result = self.inner.invoke(inv).await;

        // Log the result (success or failure)
        match &result {
            Ok(_) => {
                debug!(
                    call_count = current + 1,
                    limit = self.limit,
                    "Inner backend invocation succeeded"
                );
            }
            Err(e) => {
                debug!(
                    call_count = current + 1,
                    limit = self.limit,
                    error = %e,
                    "Inner backend invocation failed (budget slot still consumed)"
                );
            }
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{LlmResult, Message, Role};
    use std::sync::{Mutex, OnceLock};
    use std::time::Duration;

    // Single global lock for all tests that touch environment variables.
    // This ensures env-mutating tests don't run concurrently with each other.
    static ENV_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

    fn env_guard() -> std::sync::MutexGuard<'static, ()> {
        ENV_LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    /// Mock backend that always succeeds
    struct MockSuccessBackend;

    #[async_trait]
    impl LlmBackend for MockSuccessBackend {
        async fn invoke(&self, _inv: LlmInvocation) -> Result<LlmResult, LlmError> {
            Ok(LlmResult::new("test response", "mock", "mock-model"))
        }
    }

    /// Mock backend that always fails
    struct MockFailureBackend;

    #[async_trait]
    impl LlmBackend for MockFailureBackend {
        async fn invoke(&self, _inv: LlmInvocation) -> Result<LlmResult, LlmError> {
            Err(LlmError::Transport("mock failure".to_string()))
        }
    }

    fn create_test_invocation() -> LlmInvocation {
        LlmInvocation::new(
            "test-spec",
            "test-phase",
            "test-model",
            Duration::from_secs(60),
            vec![Message::new(Role::User, "test message")],
        )
    }

    #[tokio::test]
    async fn test_budget_allows_calls_under_limit() {
        let backend = BudgetedBackend::new(Box::new(MockSuccessBackend), 3);

        // First call should succeed
        let result = backend.invoke(create_test_invocation()).await;
        assert!(result.is_ok());
        assert_eq!(backend.call_count(), 1);

        // Second call should succeed
        let result = backend.invoke(create_test_invocation()).await;
        assert!(result.is_ok());
        assert_eq!(backend.call_count(), 2);

        // Third call should succeed
        let result = backend.invoke(create_test_invocation()).await;
        assert!(result.is_ok());
        assert_eq!(backend.call_count(), 3);
    }

    #[tokio::test]
    async fn test_budget_fails_at_limit() {
        let backend = BudgetedBackend::new(Box::new(MockSuccessBackend), 2);

        // First two calls should succeed
        backend.invoke(create_test_invocation()).await.unwrap();
        backend.invoke(create_test_invocation()).await.unwrap();

        // Third call should fail with BudgetExceeded
        let result = backend.invoke(create_test_invocation()).await;
        match result {
            Err(LlmError::BudgetExceeded { limit, attempted }) => {
                assert_eq!(limit, 2);
                assert_eq!(attempted, 3);
            }
            _ => panic!("Expected BudgetExceeded error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_budget_tracks_failed_calls() {
        let backend = BudgetedBackend::new(Box::new(MockFailureBackend), 2);

        // First call fails but consumes budget
        let result = backend.invoke(create_test_invocation()).await;
        assert!(result.is_err());
        assert_eq!(backend.call_count(), 1);

        // Second call fails but consumes budget
        let result = backend.invoke(create_test_invocation()).await;
        assert!(result.is_err());
        assert_eq!(backend.call_count(), 2);

        // Third call should fail with BudgetExceeded (not the mock failure)
        let result = backend.invoke(create_test_invocation()).await;
        match result {
            Err(LlmError::BudgetExceeded { limit, attempted }) => {
                assert_eq!(limit, 2);
                assert_eq!(attempted, 3);
            }
            _ => panic!("Expected BudgetExceeded error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_budget_limit_zero() {
        let backend = BudgetedBackend::new(Box::new(MockSuccessBackend), 0);

        // First call should immediately fail
        let result = backend.invoke(create_test_invocation()).await;
        match result {
            Err(LlmError::BudgetExceeded { limit, attempted }) => {
                assert_eq!(limit, 0);
                assert_eq!(attempted, 1);
            }
            _ => panic!("Expected BudgetExceeded error, got {:?}", result),
        }
    }

    #[tokio::test]
    async fn test_with_default_limit() {
        // Test with explicit limit instead of relying on env var
        let backend = BudgetedBackend::new(Box::new(MockSuccessBackend), DEFAULT_BUDGET_LIMIT);
        assert_eq!(backend.limit(), DEFAULT_BUDGET_LIMIT);
    }

    #[tokio::test]
    async fn test_with_custom_limit() {
        // Test with explicit custom limit
        let backend = BudgetedBackend::new(Box::new(MockSuccessBackend), 5);
        assert_eq!(backend.limit(), 5);
    }

    #[tokio::test]
    async fn test_budget_limit_one() {
        // Test with limit of 1
        let backend = BudgetedBackend::new(Box::new(MockSuccessBackend), 1);

        // First call should succeed
        let result = backend.invoke(create_test_invocation()).await;
        assert!(result.is_ok());

        // Second call should fail
        let result = backend.invoke(create_test_invocation()).await;
        match result {
            Err(LlmError::BudgetExceeded { limit, attempted }) => {
                assert_eq!(limit, 1);
                assert_eq!(attempted, 2);
            }
            _ => panic!("Expected BudgetExceeded error, got {:?}", result),
        }
    }

    #[test]
    fn test_budget_precedence_env_over_config() {
        let _guard = env_guard();

        // Clean up first in case previous test left it set
        // SAFETY: This is a test function that runs in isolation. We set and clean up
        // environment variables within the same test scope.
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }

        // Set environment variable
        unsafe {
            std::env::set_var(BUDGET_ENV_VAR, "15");
        }

        // Config budget is 30, but env should win
        let backend =
            BudgetedBackend::with_limit_from_config(Box::new(MockSuccessBackend), Some(30));

        assert_eq!(backend.limit(), 15);

        // Clean up
        // SAFETY: Cleaning up the environment variable we set above
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }
    }

    #[test]
    fn test_budget_precedence_config_over_default() {
        let _guard = env_guard();

        // Ensure no env var is set (clean up first in case previous test left it set)
        // SAFETY: This is a test function that runs in isolation
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }

        // Config budget should be used
        let backend =
            BudgetedBackend::with_limit_from_config(Box::new(MockSuccessBackend), Some(25));

        assert_eq!(backend.limit(), 25);

        // Clean up at end as well
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }
    }

    #[test]
    fn test_budget_precedence_default_when_none() {
        let _guard = env_guard();

        // Ensure no env var is set (clean up first in case previous test left it set)
        // SAFETY: This is a test function that runs in isolation
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }

        // No config budget, should use default
        let backend = BudgetedBackend::with_limit_from_config(Box::new(MockSuccessBackend), None);

        assert_eq!(backend.limit(), DEFAULT_BUDGET_LIMIT);

        // Clean up at end as well
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }
    }

    #[test]
    fn test_budget_precedence_env_invalid_falls_back_to_config() {
        let _guard = env_guard();

        // Clean up first in case previous test left it set
        // SAFETY: This is a test function that runs in isolation. We set and clean up
        // environment variables within the same test scope.
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }

        // Set invalid environment variable
        unsafe {
            std::env::set_var(BUDGET_ENV_VAR, "not-a-number");
        }

        // Config budget should be used when env is invalid
        let backend =
            BudgetedBackend::with_limit_from_config(Box::new(MockSuccessBackend), Some(35));

        assert_eq!(backend.limit(), 35);

        // Clean up
        // SAFETY: Cleaning up the environment variable we set above
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }
    }

    #[test]
    fn test_budget_precedence_env_empty_falls_back_to_config() {
        let _guard = env_guard();

        // Clean up first in case previous test left it set
        // SAFETY: This is a test function that runs in isolation. We set and clean up
        // environment variables within the same test scope.
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }

        // Set empty environment variable
        unsafe {
            std::env::set_var(BUDGET_ENV_VAR, "");
        }

        // Config budget should be used when env is empty
        let backend =
            BudgetedBackend::with_limit_from_config(Box::new(MockSuccessBackend), Some(40));

        assert_eq!(backend.limit(), 40);

        // Clean up
        // SAFETY: Cleaning up the environment variable we set above
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }
    }

    #[test]
    fn test_budget_zero_from_config() {
        let _guard = env_guard();

        // Ensure no env var is set (clean up first in case previous test left it set)
        // SAFETY: This is a test function that runs in isolation
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }

        // Config budget of 0 should be respected
        let backend =
            BudgetedBackend::with_limit_from_config(Box::new(MockSuccessBackend), Some(0));

        assert_eq!(backend.limit(), 0);

        // Clean up at end as well
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }
    }

    #[test]
    fn test_budget_zero_from_env() {
        let _guard = env_guard();

        // Clean up first in case previous test left it set
        // SAFETY: This is a test function that runs in isolation. We set and clean up
        // environment variables within the same test scope.
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }

        // Set environment variable to 0
        unsafe {
            std::env::set_var(BUDGET_ENV_VAR, "0");
        }

        // Env budget of 0 should be respected
        let backend =
            BudgetedBackend::with_limit_from_config(Box::new(MockSuccessBackend), Some(50));

        assert_eq!(backend.limit(), 0);

        // Clean up
        // SAFETY: Cleaning up the environment variable we set above
        unsafe {
            std::env::remove_var(BUDGET_ENV_VAR);
        }
    }
}
