//! Client error types for the Batata SDK

/// Error type for Batata gRPC client operations
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    /// Error originating from the gRPC status/transport layer.
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    /// Error establishing or using the underlying transport channel.
    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    /// Authentication with the server failed.
    #[error("auth failed: {0}")]
    AuthFailed(String),

    /// Server returned an explicit application-level error.
    #[error("server returned error: code={code}, message={message}")]
    ServerError {
        #[doc = "Application error code returned by the server."]
        code: i32,
        #[doc = "Human-readable error message returned by the server."]
        message: String,
    },

    /// Connection is not established or has been torn down.
    #[error("connection not ready")]
    NotConnected,

    /// The request exceeded its configured timeout.
    #[error("request timeout")]
    Timeout,

    /// Any other error not covered by the variants above.
    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

/// Convenience alias for [`std::result::Result`] using [`ClientError`].
pub type Result<T> = std::result::Result<T, ClientError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        let err = ClientError::NotConnected;
        assert_eq!(err.to_string(), "connection not ready");

        let err = ClientError::AuthFailed("bad credentials".to_string());
        assert_eq!(err.to_string(), "auth failed: bad credentials");

        let err = ClientError::ServerError {
            code: 500,
            message: "internal error".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "server returned error: code=500, message=internal error"
        );

        let err = ClientError::Timeout;
        assert_eq!(err.to_string(), "request timeout");
    }

    #[test]
    fn test_from_tonic_status() {
        let status = tonic::Status::unavailable("server down");
        let err: ClientError = status.into();
        assert!(matches!(err, ClientError::Grpc(_)));
    }

    #[test]
    fn test_from_anyhow_error() {
        let anyhow_err = anyhow::anyhow!("something went wrong");
        let err: ClientError = anyhow_err.into();
        assert!(matches!(err, ClientError::Other(_)));
        assert!(err.to_string().contains("something went wrong"));
    }

    #[test]
    fn test_server_error_display() {
        let err = ClientError::ServerError {
            code: 403,
            message: "forbidden".to_string(),
        };
        let display = err.to_string();
        assert!(display.contains("403"));
        assert!(display.contains("forbidden"));
    }

    #[test]
    fn test_error_debug() {
        let err = ClientError::NotConnected;
        let debug = format!("{:?}", err);
        assert!(debug.contains("NotConnected"));
    }

    #[test]
    fn test_result_type_alias() {
        fn test_fn() -> Result<i32> {
            Ok(42)
        }
        assert_eq!(test_fn().unwrap(), 42);
    }

    #[test]
    fn test_result_type_error() {
        fn test_fn() -> Result<()> {
            Err(ClientError::Timeout)
        }
        assert!(test_fn().is_err());
    }
}
