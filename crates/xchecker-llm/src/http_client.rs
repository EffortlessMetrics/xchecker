//! Shared HTTP client infrastructure for HTTP-based LLM providers
//!
//! This module provides a shared `reqwest::Client` configured once per process,
//! with timeout and retry policies for reliable HTTP communication with LLM providers.

use crate::LlmError;
use reqwest::{Client, Response, StatusCode};
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

/// Default maximum HTTP timeout (5 minutes)
const DEFAULT_MAX_HTTP_TIMEOUT: Duration = Duration::from_secs(300);

/// Default connect timeout (30 seconds)
const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(30);

/// Maximum number of retry attempts for 5xx and network failures
const MAX_RETRIES: u32 = 2;

/// Initial backoff duration for retries (1 second)
const INITIAL_BACKOFF: Duration = Duration::from_secs(1);

/// Shared HTTP client for LLM providers
///
/// This client is configured once per process and reused across all HTTP-based
/// LLM backend invocations. It provides:
/// - Connection reuse
/// - Configurable timeouts
/// - Automatic retry with exponential backoff
/// - TLS support via rustls
#[derive(Clone)]
pub(crate) struct HttpClient {
    client: Arc<Client>,
    max_timeout: Duration,
}

impl HttpClient {
    /// Create a new HTTP client with default configuration
    ///
    /// # Errors
    ///
    /// Returns `LlmError::Misconfiguration` if the client cannot be constructed
    pub fn new() -> Result<Self, LlmError> {
        Self::with_max_timeout(DEFAULT_MAX_HTTP_TIMEOUT)
    }

    /// Create a new HTTP client with a custom maximum timeout
    ///
    /// # Errors
    ///
    /// Returns `LlmError::Misconfiguration` if the client cannot be constructed
    pub fn with_max_timeout(max_timeout: Duration) -> Result<Self, LlmError> {
        let client = Client::builder()
            .connect_timeout(DEFAULT_CONNECT_TIMEOUT)
            .pool_idle_timeout(Duration::from_secs(90))
            .pool_max_idle_per_host(10)
            .use_rustls_tls()
            .build()
            .map_err(|e| {
                LlmError::Misconfiguration(format!("Failed to build HTTP client: {}", e))
            })?;

        Ok(Self {
            client: Arc::new(client),
            max_timeout,
        })
    }

    /// Execute an HTTP request with timeout and retry policy
    ///
    /// This method implements:
    /// - Per-request timeout: `min(request_timeout, global_max_http_timeout)`
    /// - Retry policy: up to 2 retries for 5xx and network failures
    /// - Exponential backoff: 1s, 2s
    /// - No retries for 4xx errors
    ///
    /// # Errors
    ///
    /// Returns `LlmError` for various failure scenarios:
    /// - `LlmError::ProviderAuth` for 401/403 errors
    /// - `LlmError::ProviderQuota` for 429 errors
    /// - `LlmError::ProviderOutage` for 5xx errors (after retries)
    /// - `LlmError::Timeout` for timeouts
    /// - `LlmError::Transport` for network errors (after retries)
    pub async fn execute_with_retry(
        &self,
        request_builder: reqwest::RequestBuilder,
        request_timeout: Duration,
        provider_name: &str,
    ) -> Result<Response, LlmError> {
        // Calculate effective timeout
        let effective_timeout = request_timeout.min(self.max_timeout);

        let mut attempt = 0;

        loop {
            attempt += 1;

            // Clone the request for this attempt
            let request = request_builder
                .try_clone()
                .ok_or_else(|| {
                    LlmError::Transport("Failed to clone request for retry".to_string())
                })?
                .timeout(effective_timeout)
                .build()
                .map_err(|e| LlmError::Transport(format!("Failed to build request: {}", e)))?;

            debug!(
                provider = provider_name,
                attempt = attempt,
                timeout_secs = effective_timeout.as_secs(),
                "Executing HTTP request"
            );

            // Execute the request
            match self.client.execute(request).await {
                Ok(response) => {
                    let status = response.status();

                    // Check for error status codes
                    if status.is_client_error() {
                        return Err(map_client_error(status, provider_name));
                    }

                    if status.is_server_error() {
                        let error = LlmError::ProviderOutage(format!(
                            "{} returned server error: {}",
                            provider_name, status
                        ));

                        // Retry 5xx errors
                        if attempt <= MAX_RETRIES {
                            warn!(
                                provider = provider_name,
                                attempt = attempt,
                                status = status.as_u16(),
                                "Server error, will retry"
                            );
                            // Apply exponential backoff
                            let backoff = INITIAL_BACKOFF * attempt;
                            tokio::time::sleep(backoff).await;
                            continue;
                        }

                        return Err(error);
                    }

                    // Success
                    return Ok(response);
                }
                Err(e) => {
                    // Check if it's a timeout
                    if e.is_timeout() {
                        return Err(LlmError::Timeout {
                            duration: effective_timeout,
                        });
                    }

                    // Network/transport error - retry if we have attempts left
                    let error = LlmError::Transport(format!(
                        "{} request failed: {}",
                        provider_name,
                        xchecker_error_redaction::redact_error_message(&e.to_string())
                    ));

                    if attempt <= MAX_RETRIES {
                        warn!(
                            provider = provider_name,
                            attempt = attempt,
                            error = %e,
                            "Network error, will retry"
                        );
                        // Apply exponential backoff
                        let backoff = INITIAL_BACKOFF * attempt;
                        tokio::time::sleep(backoff).await;
                        continue;
                    }

                    return Err(error);
                }
            }
        }
    }
}

/// Map HTTP client error status codes to LlmError variants
///
/// This function maps 4xx errors to appropriate LlmError types:
/// - 401/403 → `LlmError::ProviderAuth`
/// - 429 → `LlmError::ProviderQuota`
/// - Other 4xx → `LlmError::Transport`
fn map_client_error(status: StatusCode, provider_name: &str) -> LlmError {
    match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => LlmError::ProviderAuth(format!(
            "{} authentication failed: {}",
            provider_name, status
        )),
        StatusCode::TOO_MANY_REQUESTS => {
            LlmError::ProviderQuota(format!("{} rate limit exceeded: {}", provider_name, status))
        }
        _ => LlmError::Transport(format!(
            "{} returned client error: {}",
            provider_name, status
        )),
    }
}

/// Expose redaction function for testing (both unit and integration tests).
///
/// This function is a test seam that allows property-based tests to verify
/// that error message redaction correctly removes sensitive information.
///
/// Test seam; not part of public API stability guarantees.
#[doc(hidden)]
#[allow(dead_code)] // Used by integration tests via pub use in mod.rs
pub fn redact_error_message_for_testing(message: &str) -> String {
    xchecker_error_redaction::redact_error_message(message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_http_client_construction() {
        let client = HttpClient::new();
        assert!(client.is_ok(), "Should construct HTTP client successfully");
    }

    #[test]
    fn test_http_client_with_custom_timeout() {
        let custom_timeout = Duration::from_secs(60);
        let client = HttpClient::with_max_timeout(custom_timeout);
        assert!(
            client.is_ok(),
            "Should construct HTTP client with custom timeout"
        );

        let client = client.unwrap();
        assert_eq!(
            client.max_timeout, custom_timeout,
            "Should use custom timeout"
        );
    }

    /// Test 401 → LlmError::ProviderAuth
    /// **Property: HTTP errors map to correct LlmError variants**
    /// **Validates: Requirements 3.5.5**
    #[test]
    fn test_map_401_to_provider_auth() {
        let error = map_client_error(StatusCode::UNAUTHORIZED, "test-provider");
        match error {
            LlmError::ProviderAuth(msg) => {
                assert!(msg.contains("test-provider"));
                assert!(msg.contains("401"));
                assert!(msg.contains("authentication failed"));
            }
            _ => panic!("Expected ProviderAuth error for 401, got {:?}", error),
        }
    }

    /// Test 403 → LlmError::ProviderAuth
    /// **Property: HTTP errors map to correct LlmError variants**
    /// **Validates: Requirements 3.5.5**
    #[test]
    fn test_map_403_to_provider_auth() {
        let error = map_client_error(StatusCode::FORBIDDEN, "test-provider");
        match error {
            LlmError::ProviderAuth(msg) => {
                assert!(msg.contains("test-provider"));
                assert!(msg.contains("403"));
                assert!(msg.contains("authentication failed"));
            }
            _ => panic!("Expected ProviderAuth error for 403, got {:?}", error),
        }
    }

    /// Test 429 → LlmError::ProviderQuota
    /// **Property: HTTP errors map to correct LlmError variants**
    /// **Validates: Requirements 3.5.5**
    #[test]
    fn test_map_429_to_provider_quota() {
        let error = map_client_error(StatusCode::TOO_MANY_REQUESTS, "test-provider");
        match error {
            LlmError::ProviderQuota(msg) => {
                assert!(msg.contains("test-provider"));
                assert!(msg.contains("429"));
                assert!(msg.contains("rate limit"));
            }
            _ => panic!("Expected ProviderQuota error for 429, got {:?}", error),
        }
    }

    /// Test other 4xx → LlmError::Transport
    /// **Property: HTTP errors map to correct LlmError variants**
    /// **Validates: Requirements 3.5.5**
    #[test]
    fn test_map_other_4xx_to_transport() {
        // Test 400 Bad Request
        let error = map_client_error(StatusCode::BAD_REQUEST, "test-provider");
        match error {
            LlmError::Transport(msg) => {
                assert!(msg.contains("test-provider"));
                assert!(msg.contains("400"));
                assert!(msg.contains("client error"));
            }
            _ => panic!("Expected Transport error for 400, got {:?}", error),
        }

        // Test 404 Not Found
        let error = map_client_error(StatusCode::NOT_FOUND, "test-provider");
        match error {
            LlmError::Transport(msg) => {
                assert!(msg.contains("test-provider"));
                assert!(msg.contains("404"));
            }
            _ => panic!("Expected Transport error for 404, got {:?}", error),
        }

        // Test 422 Unprocessable Entity
        let error = map_client_error(StatusCode::UNPROCESSABLE_ENTITY, "test-provider");
        match error {
            LlmError::Transport(msg) => {
                assert!(msg.contains("test-provider"));
                assert!(msg.contains("422"));
            }
            _ => panic!("Expected Transport error for 422, got {:?}", error),
        }
    }

    /// Test that redaction preserves safe error messages
    #[test]
    fn test_redact_error_message_safe() {
        let message = "Connection failed: timeout";
        let redacted = xchecker_error_redaction::redact_error_message(message);
        assert_eq!(redacted, message, "Should preserve safe error message");
    }

    /// Test that redaction removes URLs with credentials
    /// **Property: HTTP logging never exposes secrets**
    /// **Validates: Requirements 3.5.6**
    #[test]
    fn test_redact_url_with_credentials() {
        let message = "Failed to connect to http://user:password@api.example.com/endpoint";
        let redacted = xchecker_error_redaction::redact_error_message(message);
        assert!(
            !redacted.contains("user:password"),
            "Should redact credentials from URL"
        );
        assert!(
            redacted.contains("[REDACTED]@"),
            "Should replace credentials with [REDACTED]"
        );
        assert!(redacted.contains("api.example.com"), "Should preserve host");

        // Test HTTPS as well
        let message = "Error: https://token123:secret456@openrouter.ai/api/v1";
        let redacted = xchecker_error_redaction::redact_error_message(message);
        assert!(
            !redacted.contains("token123"),
            "Should redact token from HTTPS URL"
        );
        assert!(
            !redacted.contains("secret456"),
            "Should redact secret from HTTPS URL"
        );
    }

    /// Test that redaction removes potential API keys
    /// **Property: HTTP logging never exposes secrets**
    /// **Validates: Requirements 3.5.6**
    #[test]
    fn test_redact_api_keys() {
        let message = "Authentication failed with key sk-1234567890abcdefghijklmnopqrstuvwxyz";
        let redacted = xchecker_error_redaction::redact_error_message(message);
        assert!(
            !redacted.contains("sk-1234567890abcdefghijklmnopqrstuvwxyz"),
            "Should redact long alphanumeric strings that look like keys"
        );
        assert!(
            redacted.contains("[REDACTED_KEY]"),
            "Should replace key with [REDACTED_KEY]"
        );
        assert!(
            redacted.contains("Authentication failed"),
            "Should preserve error context"
        );
    }

    /// Test that redaction handles multiple secrets in one message
    /// **Property: HTTP logging never exposes secrets**
    /// **Validates: Requirements 3.5.6**
    #[test]
    fn test_redact_multiple_secrets() {
        let message = "Failed to connect to https://user:pass@api.com with key abcdefghijklmnopqrstuvwxyz123456";
        let redacted = xchecker_error_redaction::redact_error_message(message);
        assert!(
            !redacted.contains("user:pass"),
            "Should redact URL credentials"
        );
        assert!(
            !redacted.contains("abcdefghijklmnopqrstuvwxyz123456"),
            "Should redact API key"
        );
        assert!(
            redacted.contains("Failed to connect"),
            "Should preserve error context"
        );
    }

    /// Test that error messages preserve context for debugging
    /// **Property: HTTP errors map to correct LlmError variants**
    /// **Validates: Requirements 3.5.5**
    #[test]
    fn test_error_messages_preserve_context() {
        // Auth errors should mention authentication
        let error = map_client_error(StatusCode::UNAUTHORIZED, "openrouter");
        assert!(
            error.to_string().contains("openrouter"),
            "Error should mention provider name"
        );
        assert!(
            error.to_string().contains("authentication"),
            "Error should mention authentication"
        );

        // Quota errors should mention rate limit
        let error = map_client_error(StatusCode::TOO_MANY_REQUESTS, "anthropic");
        assert!(
            error.to_string().contains("anthropic"),
            "Error should mention provider name"
        );
        assert!(
            error.to_string().contains("rate limit"),
            "Error should mention rate limit"
        );
    }
}
