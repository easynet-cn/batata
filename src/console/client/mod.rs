// Console HTTP client module
// Provides HTTP client for remote console mode

pub mod http_client;
pub mod api_client;

pub use http_client::ConsoleHttpClient;
pub use api_client::ConsoleApiClient;
