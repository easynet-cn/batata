// Console HTTP client module
// Provides HTTP client for remote console mode

pub mod api_client;
pub mod http_client;

pub use api_client::ConsoleApiClient;
pub use http_client::ConsoleHttpClient;
