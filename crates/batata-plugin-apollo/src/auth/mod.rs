//! Authentication and authorization for the Apollo-compatible plugin.
//!
//! Upstream Apollo splits its security model across three mechanisms:
//! - portal session users (`@PreAuthorize` against `UserInfoHolder`),
//! - open-platform consumers signed with a token (`ConsumerAuthenticationFilter`),
//! - the admin-service access filter (`AdminServiceAuthenticationFilter`).
//!
//! batata implements all three faithfully:
//! - `AdminAuthMiddleware` enforces the admin-service access token on the
//!   adminservice route scope (`Authorization: <token>`), mirroring
//!   `AdminServiceAuthenticationFilter`.
//! - `OpenApiAuthMiddleware` enforces the consumer token on openapi data
//!   endpoints (`Authorization: Consumer <sha256(appId|timestamp|salt)>`,
//!   see `consumer_auth`) and the user-token on portal-management endpoints.
//! - `consumer_auth` provides the SHA-256 signing used both by the filter and
//!   by `ConsumerTokenService` when issuing a token to a new consumer.

pub mod consumer_auth;
pub mod permission;
pub mod role;
