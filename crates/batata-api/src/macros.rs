/// Macro to generate RequestTrait impl for derived types that override request_type().
/// All methods delegate to the specified inner field.
macro_rules! impl_request_trait {
    ($type:ty, $inner:ident) => {
        impl $crate::remote::model::RequestTrait for $type {
            fn headers(&self) -> std::collections::HashMap<String, String> {
                self.$inner.headers()
            }

            fn request_type(&self) -> &'static str {
                stringify!($type)
            }

            fn insert_headers(&mut self, headers: std::collections::HashMap<String, String>) {
                self.$inner.insert_headers(headers);
            }

            fn request_id(&self) -> String {
                self.$inner.request_id()
            }
        }
    };
    // Base type variant: does NOT override request_type() (uses default "")
    (base $type:ty, $inner:ident) => {
        impl $crate::remote::model::RequestTrait for $type {
            fn headers(&self) -> std::collections::HashMap<String, String> {
                self.$inner.headers()
            }

            fn insert_headers(&mut self, headers: std::collections::HashMap<String, String>) {
                self.$inner.insert_headers(headers);
            }

            fn request_id(&self) -> String {
                self.$inner.request_id()
            }
        }
    };
}

/// Macro to generate ResponseTrait impl for types with a `response: Response` field.
macro_rules! impl_response_trait {
    ($type:ty) => {
        impl $crate::remote::model::ResponseTrait for $type {
            fn response_type(&self) -> &'static str {
                stringify!($type)
            }

            fn request_id(&mut self, request_id: String) {
                self.response.request_id = request_id;
            }

            fn error_code(&self) -> i32 {
                self.response.error_code
            }

            fn result_code(&self) -> i32 {
                self.response.result_code
            }
        }
    };
}
