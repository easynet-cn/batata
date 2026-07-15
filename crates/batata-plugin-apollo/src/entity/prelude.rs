//! `SeaORM` Entity Prelude for Apollo plugin.

pub use super::apollo_app::Entity as ApolloApp;
pub use super::apollo_app_namespace::Entity as ApolloAppNamespace;
pub use super::apollo_cluster::Entity as ApolloCluster;
pub use super::apollo_namespace::Entity as ApolloNamespace;
pub use super::apollo_item::Entity as ApolloItem;
pub use super::apollo_release::Entity as ApolloRelease;
pub use super::apollo_commit::Entity as ApolloCommit;
pub use super::apollo_release_message::Entity as ApolloReleaseMessage;
pub use super::apollo_gray_release_rule::Entity as ApolloGrayReleaseRule;
pub use super::apollo_instance::Entity as ApolloInstance;
pub use super::apollo_instance_config::Entity as ApolloInstanceConfig;
pub use super::apollo_namespace_lock::Entity as ApolloNamespaceLock;
pub use super::apollo_audit::Entity as ApolloAudit;
pub use super::apollo_server_config::Entity as ApolloServerConfig;
pub use super::apollo_consumer::Entity as ApolloConsumer;
pub use super::apollo_consumer_audit::Entity as ApolloConsumerAudit;
pub use super::apollo_consumer_role::Entity as ApolloConsumerRole;
pub use super::apollo_consumer_token::Entity as ApolloConsumerToken;
pub use super::apollo_favorite::Entity as ApolloFavorite;
pub use super::apollo_permission::Entity as ApolloPermission;
pub use super::apollo_role::Entity as ApolloRole;
pub use super::apollo_role_permission::Entity as ApolloRolePermission;
pub use super::apollo_user_role::Entity as ApolloUserRole;
pub use super::apollo_users::Entity as ApolloUsers;
pub use super::apollo_access_key::Entity as ApolloAccessKey;
pub use super::apollo_release_history::Entity as ApolloReleaseHistory;
