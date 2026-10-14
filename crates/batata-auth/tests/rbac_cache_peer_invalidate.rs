//! Peer notification half of the RBAC cache invalidation.
//!
//! Lives in its own test binary because the peer notifier is a process-global
//! `OnceLock`, so it cannot be shared with tests that expect a different
//! notifier (or none at all).

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use batata_auth::service::rbac_cache::{self, RbacSource};
use batata_persistence::model::{PermissionInfo, RoleInfo};

/// Auth backend that counts role reads and always reports one role.
struct StubPersistence {
    role_reads: AtomicUsize,
}

#[async_trait::async_trait]
impl RbacSource for StubPersistence {
    async fn load_roles(&self, username: &str) -> anyhow::Result<Vec<RoleInfo>> {
        self.role_reads.fetch_add(1, Ordering::SeqCst);

        Ok(vec![RoleInfo {
            role: "ROLE_A".to_owned(),
            username: username.to_owned(),
        }])
    }

    async fn load_permissions(&self, roles: Vec<String>) -> anyhow::Result<Vec<PermissionInfo>> {
        Ok(roles
            .into_iter()
            .map(|role| PermissionInfo {
                role,
                resource: "public:*".to_owned(),
                action: "rw".to_owned(),
            })
            .collect())
    }
}

/// The public invalidators notify the peers, the `*_local` variants do not.
///
/// Sequential on purpose: the notifier is global, so two tests mutating it in
/// parallel would race.
#[tokio::test]
async fn public_invalidators_notify_peers_and_local_ones_do_not() {
    let seen: Arc<Mutex<Vec<(String, String)>>> = Arc::new(Mutex::new(Vec::new()));
    let sink = seen.clone();

    rbac_cache::set_peer_invalidator(Arc::new(move |kind: &str, target: &str| {
        sink.lock()
            .unwrap()
            .push((kind.to_owned(), target.to_owned()));
    }));

    // A peer-driven eviction must never bounce back to the cluster.
    rbac_cache::invalidate_user_local("alice");
    rbac_cache::invalidate_role_local("ROLE_A");
    rbac_cache::invalidate_all_users_local();
    assert!(
        seen.lock().unwrap().is_empty(),
        "local evictions must not notify peers"
    );

    // The public variants evict locally *and* notify.
    let backend = StubPersistence {
        role_reads: AtomicUsize::new(0),
    };

    rbac_cache::invalidate_user("alice");
    assert_eq!(rbac_cache::roles(&backend, "alice").await.len(), 1);
    assert_eq!(rbac_cache::roles(&backend, "alice").await.len(), 1);
    assert_eq!(backend.role_reads.load(Ordering::SeqCst), 1);

    rbac_cache::invalidate_user("alice");
    assert_eq!(rbac_cache::roles(&backend, "alice").await.len(), 1);
    assert_eq!(backend.role_reads.load(Ordering::SeqCst), 2);

    rbac_cache::invalidate_role("ROLE_A");
    rbac_cache::invalidate_all_users();

    let seen = seen.lock().unwrap();
    assert_eq!(
        *seen,
        vec![
            ("user".to_string(), "alice".to_string()),
            ("user".to_string(), "alice".to_string()),
            ("role".to_string(), "ROLE_A".to_string()),
            ("all".to_string(), String::new()),
        ]
    );
}
