//! Client error types for the Batata SDK

/// Error type for Batata gRPC client operations
#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("gRPC error: {0}")]
    Grpc(#[from] tonic::Status),

    #[error("transport error: {0}")]
    Transport(#[from] tonic::transport::Error),

    #[error("auth failed: {0}")]
    AuthFailed(String),

    #[error("server returned error: code={code}, message={message}")]
    ServerError { code: i32, message: String },

    #[error("connection not ready")]
    NotConnected,

    #[error("request timeout")]
    Timeout,

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

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
