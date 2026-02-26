//! gRPC Handler macros and utilities
//!
//! This module provides macros and helper functions to reduce boilerplate
//! in gRPC handler implementations.

use tonic::Status;

use crate::api::{grpc::Payload, remote::model::ResponseCode};

/// Helper trait for creating error responses
pub trait ErrorResponseBuilder {
    /// Build an error response with the given request_id and error message
    fn build_error_response(&mut self, code: i32, message: String);
}

/// Helper function to create a failure status from an error
pub fn error_to_status<E: std::fmt::Display>(e: E) -> Status {
    Status::internal(e.to_string())
}

/// Helper function to create failure response codes
pub fn fail_code() -> i32 {
    ResponseCode::Fail.code()
}

/// Macro for defining a simple gRPC handler struct with common fields
///
/// # Usage
/// ```ignore
/// define_handler!(ConfigQueryHandler {
///     app_state: Arc<AppState>,
/// });
/// ```
#[macro_export]
macro_rules! define_handler {
    ($name:ident { $($field:ident : $type:ty),* $(,)? }) => {
        #[derive(Clone)]
        pub struct $name {
            $(pub $field: $type),*
        }
    };
}

/// Macro for implementing can_handle method
///
/// # Usage
/// ```ignore
/// impl_can_handle!(ConfigQueryHandler, "ConfigQueryRequest");
/// ```
#[macro_export]
macro_rules! impl_can_handle {
    ($handler:ty, $message_type:expr) => {
        impl $handler {
            pub const MESSAGE_TYPE: &'static str = $message_type;
        }
    };
}

/// Macro for implementing a simple acknowledgement handler
///
/// Creates a handler that just acknowledges the request by returning
/// a success response with the same request_id.
///
/// # Usage
/// ```ignore
/// impl_ack_handler!(
///     ConfigChangeNotifyHandler,
///     "ConfigChangeNotifyRequest",
///     ConfigChangeNotifyRequest,
///     ConfigChangeNotifyResponse
/// );
/// ```
#[macro_export]
macro_rules! impl_ack_handler {
    (
        $handler:ty,
        $message_type:expr,
        $request_type:ty,
        $response_type:ty
    ) => {
        #[tonic::async_trait]
        impl $crate::service::rpc::PayloadHandler for $handler {
            async fn handle(
                &self,
                _connection: &batata_core::model::Connection,
                payload: &$crate::api::grpc::Payload,
            ) -> Result<$crate::api::grpc::Payload, tonic::Status> {
                use $crate::api::remote::model::{RequestTrait, ResponseTrait};

                let request = <$request_type>::from(payload);
                let request_id = request.request_id();

                let mut response = <$response_type>::new();
                response.response.request_id = request_id;

                Ok(response.build_payload())
            }

            fn can_handle(&self) -> &'static str {
                $message_type
            }
        }
    };

    // With auth requirement
    (
        $handler:ty,
        $message_type:expr,
        $request_type:ty,
        $response_type:ty,
        auth = $auth_req:expr
    ) => {
        #[tonic::async_trait]
        impl $crate::service::rpc::PayloadHandler for $handler {
            async fn handle(
                &self,
                _connection: &batata_core::model::Connection,
                payload: &$crate::api::grpc::Payload,
            ) -> Result<$crate::api::grpc::Payload, tonic::Status> {
                use $crate::api::remote::model::{RequestTrait, ResponseTrait};

                let request = <$request_type>::from(payload);
                let request_id = request.request_id();

                let mut response = <$response_type>::new();
                response.response.request_id = request_id;

                Ok(response.build_payload())
            }

            fn can_handle(&self) -> &'static str {
                $message_type
            }

            fn auth_requirement(&self) -> $crate::service::rpc::AuthRequirement {
                $auth_req
            }
        }
    };
}

/// Macro for creating a standard error payload response
///
/// # Usage
/// ```ignore
/// let response = error_response!(ConfigQueryResponse, request_id, "config not found");
/// ```
#[macro_export]
macro_rules! error_response {
    ($response_type:ty, $request_id:expr, $message:expr) => {{
        let mut response = <$response_type>::new();
        response.response.request_id = $request_id;
        response.response.result_code = $crate::api::remote::model::ResponseCode::Fail.code();
        response.response.error_code = $crate::api::remote::model::ResponseCode::Fail.code();
        response.response.success = false;
        response.response.message = $message.to_string();
        response
    }};
    ($response_type:ty, $request_id:expr, $error_code:expr, $message:expr) => {{
        let mut response = <$response_type>::new();
        response.response.request_id = $request_id;
        response.response.result_code = $crate::api::remote::model::ResponseCode::Fail.code();
        response.response.error_code = $error_code;
        response.response.success = false;
        response.response.message = $message.to_string();
        response
    }};
}

/// Macro for creating a success payload response
///
/// # Usage
/// ```ignore
/// let response = success_response!(ConfigQueryResponse, request_id);
/// ```
#[macro_export]
macro_rules! success_response {
    ($response_type:ty, $request_id:expr) => {{
        let mut response = <$response_type>::new();
        response.response.request_id = $request_id;
        response
    }};
}

/// Helper struct for building gRPC responses with consistent error handling
pub struct ResponseBuilder<T> {
    response: T,
    request_id: String,
}

impl<T> ResponseBuilder<T>
where
    T: Default,
{
    pub fn new(request_id: String) -> Self {
        Self {
            response: T::default(),
            request_id,
        }
    }

    pub fn with_response(response: T, request_id: String) -> Self {
        Self {
            response,
            request_id,
        }
    }

    pub fn request_id(&self) -> &str {
        &self.request_id
    }

    pub fn into_inner(self) -> T {
        self.response
    }

    pub fn response(&self) -> &T {
        &self.response
    }

    pub fn response_mut(&mut self) -> &mut T {
        &mut self.response
    }
}

/// Extension trait for Payload to extract common information
pub trait PayloadExt {
    fn message_type(&self) -> Option<&str>;
    fn client_ip(&self) -> Option<&str>;
    fn headers(&self) -> std::collections::HashMap<String, String>;
}

impl PayloadExt for Payload {
    fn message_type(&self) -> Option<&str> {
        self.metadata.as_ref().map(|m| m.r#type.as_str())
    }

    fn client_ip(&self) -> Option<&str> {
        self.metadata.as_ref().map(|m| m.client_ip.as_str())
    }

    fn headers(&self) -> std::collections::HashMap<String, String> {
        self.metadata
            .as_ref()
            .map(|m| m.headers.clone())
            .unwrap_or_default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fail_code() {
        assert_eq!(fail_code(), 500);
    }

    #[test]
    fn test_error_to_status() {
        let status = error_to_status("test error");
        assert_eq!(status.code(), tonic::Code::Internal);
        assert_eq!(status.message(), "test error");
    }
}
