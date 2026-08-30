/// Embedded (in-memory) persistence backend.
pub mod embedded;
/// Shared storage types.
pub mod shared;
/// SQL persistence backend.
pub mod sql;
/// Trait implementations binding storage backends to the persistence traits.
pub mod trait_impl;
/// Persistence trait definitions.
pub mod traits;

pub use embedded::EmbeddedApolloPersistence;
pub use shared::*;
pub use sql::SqlApolloPersistence;
pub use trait_impl::{
    ApolloPersistence, ExternalDbApolloPersistence,
};
pub use traits::{
    AccessKeyPersistence, AppPersistence, CommitPersistence, GrayReleasePersistence,
    InstancePersistence, ItemPersistence, NamespaceLockPersistence, NamespacePersistence,
    ReleaseMessagePersistence, ReleasePersistence, ApolloPersistenceService,
};
