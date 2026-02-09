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
}
