//! `SeaORM` Entity for Apollo plugin.

/// Entity prelude re-exports.
pub mod prelude;

/// App entity.
pub mod apollo_app;
/// App namespace entity.
pub mod apollo_app_namespace;
/// Cluster entity.
pub mod apollo_cluster;
/// Namespace entity.
pub mod apollo_namespace;
/// Config item entity.
pub mod apollo_item;
/// Release entity.
pub mod apollo_release;
/// Commit entity.
pub mod apollo_commit;
/// Release message entity.
pub mod apollo_release_message;
/// Gray release rule entity.
pub mod apollo_gray_release_rule;
/// Instance entity.
pub mod apollo_instance;
/// Instance config entity.
pub mod apollo_instance_config;
/// Namespace lock entity.
pub mod apollo_namespace_lock;
/// Audit entity.
pub mod apollo_audit;
/// Server config entity.
pub mod apollo_server_config;
/// Service registry entity.
pub mod apollo_service_registry;
/// Consumer entity.
pub mod apollo_consumer;
/// Consumer audit entity.
pub mod apollo_consumer_audit;
/// Consumer role entity.
pub mod apollo_consumer_role;
/// Consumer token entity.
pub mod apollo_consumer_token;
/// Favorite entity.
pub mod apollo_favorite;
/// Permission entity.
pub mod apollo_permission;
/// Role entity.
pub mod apollo_role;
/// Role permission entity.
pub mod apollo_role_permission;
/// User role entity.
pub mod apollo_user_role;
/// Users entity.
pub mod apollo_users;
/// Access key entity.
pub mod apollo_access_key;
/// Release history entity.
pub mod apollo_release_history;
/// User token entity (portal session / openapi user token).
pub mod apollo_user_token;
