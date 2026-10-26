//! Integration test for the console **independent (remote) deployment**.
//!
//! Nacos supports running the console as a separate process that proxies to a
//! core cluster via an HTTP `MaintainerClient` (deployment type `console`).
//! Batata mirrors this in `batata-console/src/datasource/remote.rs`.
//!
//! This test launches a *real* core server (standalone, embedded) and a *real*
//! console-remote server (deployment_type=console pointing at the core), then:
//!   1. asserts the console-remote process boots and serves the console API
//!      surface (`/v3/console/health/liveness` -> 200),
//!   2. asserts a guarded, proxied read
//!      (`/v3/console/core/namespace?namespaceId=<seeded>`) returns 200 when called
//!      with a valid core-issued token, and
//!   3. asserts the same guarded read returns 403 without a token, proving the
//!      remote console is *enforcing* auth (not merely bypassing it).
//!
//! Following Nacos, **authorization happens in the console**: the console runs
//! the real `DefaultAuthPlugin` in-process, so token validation and the RBAC
//! decision are made locally with the end user's identity. Only the
//! user/role/permission *data* it reads switches backend — here a
//! `RemoteAuthDataService` that fetches them from the core over HTTP, because an
//! independently deployed console must not depend on the database.
//!
//! So a successful guarded read here proves three things at once: the console
//! built a working auth plugin, it reached the core for the user's role/permission
//! data, and it validated the token locally.
//!
//! The two spawned servers run with authentication **enabled**
//! (`batata.core.auth.enabled=true` and `batata.core.auth.console.enabled=true`),
//! which is exactly the scenario the gap manifested in: with auth on, the
//! console's `secured!` macro must authenticate and authorize every guarded
//! request. The seeded `nacos` user is a global admin (created via the open
//! `/v3/auth/user/admin` bootstrap endpoint), so it passes both identity
//! validation and RBAC on the proxied read.
//!
//! Login is performed *against the console* (not the core) so the test covers the
//! remote credential lookup: the console asks the core for the `nacos` user
//! record and verifies the password locally.
//!
//! The two-process approach exercises the full independent-deployment path the
//! way a real operator would run it (no in-process shortcuts).

use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::Duration;

use reqwest::StatusCode;
use tokio::time::sleep;

const CORE_PORT: u16 = 8848;
const CONSOLE_PORT: u16 = 9090;
const CONSOLE_MAIN_PORT: u16 = 9091; // placeholder main port for the remote console
const USER: &str = "nacos";
const PASS: &str = "nacos";

struct ChildGuard {
    child: Child,
}

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn workspace_root() -> PathBuf {
    // CARGO_MANIFEST_DIR for this integration test == crates/batata-console
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../")
        .canonicalize()
        .expect("workspace root resolvable")
}

fn find_server_binary() -> Option<PathBuf> {
    let root = workspace_root();
    for profile in ["release", "debug"] {
        let cand = root.join("target").join(profile).join("batata-server");
        if cand.exists() {
            return Some(cand);
        }
    }
    None
}

async fn wait_for_http(base: &str, path: &str, timeout: Duration) -> bool {
    let url = format!("{base}{path}");
    let deadline = tokio::time::Instant::now() + timeout;
    loop {
        if let Ok(resp) = reqwest::get(&url).await {
            if resp.status().is_success() {
                return true;
            }
        }
        if tokio::time::Instant::now() >= deadline {
            return false;
        }
        sleep(Duration::from_millis(500)).await;
    }
}

async fn seed_admin(base: &str) {
    let client = reqwest::Client::new();
    let _ = client
        .post(format!("{base}/nacos/v3/auth/user/admin"))
        .form(&[("username", USER), ("password", PASS)])
        .send()
        .await;
}

async fn create_namespace(base: &str, token: &str, ns_id: &str) -> StatusCode {
    let client = reqwest::Client::new();
    let resp = client
        .post(format!("{base}/v3/console/core/namespace"))
        .bearer_auth(token)
        .form(&[
            ("customNamespaceId", ns_id),
            ("namespaceName", ns_id),
            ("namespaceDesc", "integration-test"),
        ])
        .send()
        .await
        .expect("create namespace request");
    resp.status()
}

/// Log in against an absolute login URL.
///
/// The console context path is empty while the core's is `/nacos`, so callers
/// pass the full path.
async fn login(url: &str) -> String {
    let client = reqwest::Client::new();
    let resp = client
        .post(url)
        .form(&[("username", USER), ("password", PASS)])
        .send()
        .await
        .expect("login request");
    let body: serde_json::Value = resp.json().await.expect("login json");
    // Response shape: {"accessToken":"..."} (top level)
    body.get("accessToken")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string()
}

#[tokio::test]
async fn console_remote_deployment_proxies_to_core() {
    let bin = match find_server_binary() {
        Some(b) => b,
        None => {
            eprintln!(
                "SKIP: batata-server binary not found (build it first: cargo build --release -p batata-server)"
            );
            return;
        }
    };
    let root = workspace_root();

    let core_store = std::env::temp_dir().join(format!("batata-it-core-{}", std::process::id()));
    let console_store =
        std::env::temp_dir().join(format!("batata-it-console-remote-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&core_store);
    let _ = std::fs::remove_dir_all(&console_store);

    // --- Core server (standalone, embedded) ---
    let core = Command::new(&bin)
        .current_dir(&root)
        .args([
            "-m",
            "standalone",
            "-d",
            "server",
            &format!("--batata.server.main.port={CORE_PORT}"),
            // Enable authentication so `login` issues a real (validatable) JWT
            // instead of the `mock-token-auth-disabled` placeholder returned when
            // auth is off. The console must validate that JWT locally.
            "--batata.core.auth.enabled=true",
            &format!(
                "--batata.persistence.embedded.data_dir={}",
                core_store.display()
            ),
        ])
        .stdout(std::fs::File::create(root.join("/tmp/it-core.log")).unwrap())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn core");
    let _core_guard = ChildGuard { child: core };

    let core_base = format!("http://127.0.0.1:{CORE_PORT}");
    assert!(
        wait_for_http(
            &core_base,
            "/nacos/v3/admin/core/state/liveness",
            Duration::from_secs(90)
        )
        .await,
        "core server did not become ready"
    );
    seed_admin(&core_base).await;

    // --- Console-remote server (deployment_type=console -> proxies to core) ---
    let console = Command::new(&bin)
        .current_dir(&root)
        .args([
            "-m",
            "standalone",
            "-d",
            "server",
            "--batata.deployment.type=console",
            &format!("--batata.console.remote.server_addr={core_base}"),
            // `batata.member.list` outranks both `conf/cluster.conf` (present in
            // the workspace root, so it would win over `server_addr`) and
            // `server_addr` itself. Pin it so the console really targets *this*
            // core instead of whichever address the checkout happens to list.
            "--batata.member.list=127.0.0.1:8848",
            &format!("--batata.console.port={CONSOLE_PORT}"),
            &format!("--batata.server.main.port={CONSOLE_MAIN_PORT}"),
            // Enable core + console auth so the console's `secured!` macro actually
            // enforces on `/v3/console/...` and consults the wired auth plugin.
            "--batata.core.auth.enabled=true",
            "--batata.core.auth.console.enabled=true",
            &format!(
                "--batata.persistence.embedded.data_dir={}",
                console_store.display()
            ),
        ])
        .stdout(std::fs::File::create(root.join("/tmp/it-console-remote.log")).unwrap())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn console-remote");
    let _console_guard = ChildGuard { child: console };

    let console_base = format!("http://127.0.0.1:{CONSOLE_PORT}");
    // The console-remote server mounts its API under `/v3/console/...` (no /nacos
    // prefix, since it is a separate HTTP server with an empty context path).
    assert!(
        wait_for_http(
            &console_base,
            "/v3/console/health/liveness",
            Duration::from_secs(90)
        )
        .await,
        "console-remote did not become ready / did not serve the console API"
    );

    // --- Guard 1: independent console deployment boots and serves its API ---
    let liveness = reqwest::get(format!("{console_base}/v3/console/health/liveness"))
        .await
        .expect("liveness request")
        .status();
    assert_eq!(liveness, StatusCode::OK);

    // --- Guard 2: a guarded, proxied read succeeds with a valid token ---
    // The console builds the real DefaultAuthPlugin locally and reads the user
    // record plus role/permission bindings from the core over HTTP. Logging in
    // *through the console* exercises that remote credential lookup; the
    // resulting token must then authenticate and authorize locally (the seeded
    // `nacos` user is a global admin), so the guarded read returns 200.
    let token = login(&format!("{console_base}/v3/auth/user/login")).await;
    assert!(
        !token.is_empty(),
        "console login returned an empty token; the remote auth data service could not \
         reach the core (check batata.console.remote.server_addr and server identity)"
    );

    // A fresh embedded store has no persisted namespaces and `public` is not
    // stored as a detail row, so seed a namespace through the console (write
    // proxy) and read it back (read proxy). This exercises the exact detail-read
    // path that failed before the auth bridge was wired.
    let ns_id = "it-console-ns";
    let create_status = create_namespace(&console_base, &token, ns_id).await;
    assert_eq!(
        create_status,
        StatusCode::OK,
        "remote console failed to create namespace via the core proxy"
    );
    let guarded_resp = reqwest::Client::new()
        .get(format!(
            "{console_base}/v3/console/core/namespace?namespaceId={ns_id}"
        ))
        .bearer_auth(&token)
        .send()
        .await
        .expect("guarded request");
    let guarded_status = guarded_resp.status();
    let guarded_body = guarded_resp.text().await.unwrap_or_default();
    assert_eq!(
        guarded_status,
        StatusCode::OK,
        "expected the remote console guarded read to 200 after the auth plugin was wired in; body={guarded_body}"
    );

    // --- Guard 3: a guarded, proxied read without a token is rejected (403) ---
    // This proves the console is *enforcing* auth, not merely bypassing it: with no
    // bearer token the locally-run auth plugin reports an unauthenticated identity
    // and the `secured!` macro refuses the request.
    let unauth_resp = reqwest::Client::new()
        .get(format!(
            "{console_base}/v3/console/core/namespace?namespaceId=public"
        ))
        .send()
        .await
        .expect("unauthenticated request");
    let unauth_status = unauth_resp.status();
    let unauth_body = unauth_resp.text().await.unwrap_or_default();
    assert_eq!(
        unauth_status,
        StatusCode::FORBIDDEN,
        "expected the remote console guarded read without a token to 403 (auth enforced); body={unauth_body}"
    );

    // --- Guard 4: user/role/permission management works on the console ---
    //
    // `/v3/auth/**` is mounted in the console process too, so a separated
    // console serves its own management UI from the core's data. Before the
    // remote auth data backend existed these handlers went straight to
    // `AppState::persistence()`, which is `None` in console mode — the request
    // panicked instead of being answered.
    let managed_user = "it-console-user";
    let managed_role = "IT_CONSOLE_ROLE";

    let created = auth_form(
        &format!("{console_base}/v3/auth/user"),
        &token,
        &[("username", managed_user), ("password", "it-password")],
    )
    .await;
    assert_eq!(
        created,
        StatusCode::OK,
        "console could not create a user through the remote auth data backend"
    );

    let listed = auth_get(
        &format!(
            "{console_base}/v3/auth/user/list?username={managed_user}&pageNo=1&pageSize=10&search=accurate"
        ),
        &token,
    )
    .await;
    assert!(
        listed.contains(managed_user),
        "user list served by the console does not contain the created user: {listed}"
    );

    // The console holds no database of its own, so the user can only be visible
    // if the write crossed to the core. Reading it back straight from the core
    // proves the console's management handlers really used the remote backend.
    let core_token = login(&format!("{core_base}/nacos/v3/auth/user/login")).await;
    let on_core = auth_get(
        &format!(
            "{core_base}/nacos/v3/auth/user/list?username={managed_user}&pageNo=1&pageSize=10&search=accurate"
        ),
        &core_token,
    )
    .await;
    assert!(
        on_core.contains(managed_user),
        "user created through the console is not present on the core; the console did not \
         proxy the write: {on_core}"
    );

    let role_added = auth_form(
        &format!("{console_base}/v3/auth/role"),
        &token,
        &[("role", managed_role), ("username", managed_user)],
    )
    .await;
    assert_eq!(
        role_added,
        StatusCode::OK,
        "console could not assign a role"
    );

    let roles = auth_get(
        &format!("{console_base}/v3/auth/role/list?username={managed_user}&pageNo=1&pageSize=10"),
        &token,
    )
    .await;
    assert!(
        roles.contains(managed_role),
        "role list served by the console does not contain the assigned role: {roles}"
    );

    let permission_added = auth_form(
        &format!("{console_base}/v3/auth/permission"),
        &token,
        &[
            ("role", managed_role),
            ("resource", "it-resource"),
            ("action", "rw"),
        ],
    )
    .await;
    assert_eq!(
        permission_added,
        StatusCode::OK,
        "console could not grant a permission"
    );

    let permissions = auth_get(
        &format!("{console_base}/v3/auth/permission/list?role={managed_role}&pageNo=1&pageSize=10"),
        &token,
    )
    .await;
    assert!(
        permissions.contains("it-resource"),
        "permission list served by the console does not contain the grant: {permissions}"
    );

    // --- Guard 5: visibility grants work on the separated console ---
    //
    // `/v3/auth/visibility` is plugin-owned: the dedicated role and permission
    // row are built by whoever owns the data. In console mode that is the core,
    // so the console forwards the grant instead of replaying it as a role +
    // permission pair — the split Nacos makes in `RemoteVisibilityGrantService`.
    let visibility_granted = auth_form(
        &format!("{console_base}/v3/auth/visibility"),
        &token,
        &[
            ("namespaceId", ns_id),
            ("resourceType", "config"),
            ("resourceName", "it-app.yml"),
            ("username", managed_user),
            ("action", "r"),
        ],
    )
    .await;
    assert_eq!(
        visibility_granted,
        StatusCode::OK,
        "console could not grant visibility through the remote auth data backend"
    );

    // The console has no database of its own, so the dedicated visibility role
    // can only exist on the core if the grant really crossed the process line.
    let visibility_on_core = auth_get(
        &format!(
            "{core_base}/nacos/v3/auth/role/list?username={managed_user}&pageNo=1&pageSize=10"
        ),
        &core_token,
    )
    .await;
    assert!(
        visibility_on_core.contains("__nacos_vis__.u."),
        "visibility grant did not reach the core: {visibility_on_core}"
    );

    let visibility_revoked = auth_delete(
        &format!(
            "{console_base}/v3/auth/visibility?namespaceId={ns_id}&resourceType=config\
             &resourceName=it-app.yml&username={managed_user}&action=r"
        ),
        &token,
    )
    .await;
    assert_eq!(
        visibility_revoked,
        StatusCode::OK,
        "console could not revoke visibility"
    );

    // Tear the management objects back down, reusing the same remote paths.
    let revoked = auth_delete(
        &format!(
            "{console_base}/v3/auth/permission?role={managed_role}&resource=it-resource&action=rw"
        ),
        &token,
    )
    .await;
    assert_eq!(
        revoked,
        StatusCode::OK,
        "console could not revoke permission"
    );

    let role_removed = auth_delete(
        &format!("{console_base}/v3/auth/role?role={managed_role}&username={managed_user}"),
        &token,
    )
    .await;
    assert_eq!(
        role_removed,
        StatusCode::OK,
        "console could not remove role"
    );

    let removed = auth_delete(
        &format!("{console_base}/v3/auth/user?username={managed_user}"),
        &token,
    )
    .await;
    assert_eq!(removed, StatusCode::OK, "console could not delete user");

    // --- Guard 6: admin bootstrap is answered, not panicked on ---
    //
    // `/v3/auth/user/admin` is the first-run bootstrap the console UI calls. It
    // has no `secured!` guard, but it still reads role data, so before the
    // deployment-aware backend existed it reached for `AppState::persistence()`
    // — `None` in console mode — and answered 500 instead of a real result.
    //
    // An admin already exists at this point (the core was seeded), so the console
    // must report 409 "already initialised". Reaching that verdict requires the
    // remote backend's global-admin lookup to have crossed to the core.
    let bootstrap = auth_form(
        &format!("{console_base}/v3/auth/user/admin"),
        &token,
        &[("username", "it-bootstrap"), ("password", "it-password")],
    )
    .await;
    assert_eq!(
        bootstrap,
        StatusCode::CONFLICT,
        "console admin bootstrap did not detect the existing admin through the remote backend"
    );

    let _ = std::fs::remove_dir_all(&core_store);
    let _ = std::fs::remove_dir_all(&console_store);
    // Guards drop here and kill the child processes.
}

/// First-run bootstrap performed *through* an independently deployed console.
///
/// `POST /v3/auth/user/admin` is the endpoint the console UI calls on a brand new
/// cluster. The separated console has no database, so it forwards the bootstrap
/// to the core — and it must forward the **plaintext** password, because the core
/// is the side that hashes. Hashing in the console and again in the core would
/// create an administrator nobody can ever log in as.
///
/// So the real proof here is not the 200: it is that the very next login, which
/// runs entirely in the console against the hash the core stored, succeeds.
#[tokio::test]
async fn console_remote_deployment_bootstraps_first_admin() {
    let bin = match find_server_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: batata-server binary not found");
            return;
        }
    };
    let root = workspace_root();

    let core_port = 9948u16;
    let console_port = 9290u16;
    let console_main_port = 9291u16;

    let core_store =
        std::env::temp_dir().join(format!("batata-it-boot-core-{}", std::process::id()));
    let console_store =
        std::env::temp_dir().join(format!("batata-it-boot-console-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&core_store);
    let _ = std::fs::remove_dir_all(&console_store);

    // A core with authentication on and **no administrator yet**.
    let core = Command::new(&bin)
        .current_dir(&root)
        .args([
            "-m",
            "standalone",
            "-d",
            "server",
            &format!("--batata.server.main.port={core_port}"),
            "--batata.core.auth.enabled=true",
            &format!(
                "--batata.persistence.embedded.data_dir={}",
                core_store.display()
            ),
        ])
        .stdout(std::fs::File::create(root.join("/tmp/it-boot-core.log")).unwrap())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn core");
    let _core_guard = ChildGuard { child: core };

    let core_base = format!("http://127.0.0.1:{core_port}");
    assert!(
        wait_for_http(
            &core_base,
            "/nacos/v3/admin/core/state/liveness",
            Duration::from_secs(90)
        )
        .await,
        "core server did not become ready"
    );
    // Deliberately no `seed_admin` here: the cluster must start uninitialised.

    let console = Command::new(&bin)
        .current_dir(&root)
        .args([
            "-m",
            "standalone",
            "-d",
            "server",
            "--batata.deployment.type=console",
            &format!("--batata.console.remote.server_addr={core_base}"),
            // See the note in the sibling test: without this the console would
            // resolve `conf/cluster.conf` (8848) instead of this core.
            &format!("--batata.member.list=127.0.0.1:{core_port}"),
            &format!("--batata.console.port={console_port}"),
            &format!("--batata.server.main.port={console_main_port}"),
            "--batata.core.auth.enabled=true",
            "--batata.core.auth.console.enabled=true",
            &format!(
                "--batata.persistence.embedded.data_dir={}",
                console_store.display()
            ),
        ])
        .stdout(std::fs::File::create(root.join("/tmp/it-boot-console.log")).unwrap())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn console-remote");
    let _console_guard = ChildGuard { child: console };

    let console_base = format!("http://127.0.0.1:{console_port}");
    assert!(
        wait_for_http(
            &console_base,
            "/v3/console/health/liveness",
            Duration::from_secs(90)
        )
        .await,
        "console-remote did not become ready"
    );

    let bootstrap = reqwest::Client::new()
        .post(format!("{console_base}/v3/auth/user/admin"))
        .form(&[("username", USER), ("password", PASS)])
        .send()
        .await
        .expect("bootstrap request");
    let bootstrap_status = bootstrap.status();
    let bootstrap_body = bootstrap.text().await.unwrap_or_default();
    assert_eq!(
        bootstrap_status,
        StatusCode::OK,
        "separated console could not bootstrap the first admin through the core: {bootstrap_body}"
    );

    // The administrator now has to live on the core, not in the console.
    let core_token = login(&format!("{core_base}/nacos/v3/auth/user/login")).await;
    let users = auth_get(
        &format!("{core_base}/nacos/v3/auth/user/list?username={USER}&pageNo=1&pageSize=10&search=accurate"),
        &core_token,
    )
    .await;
    assert!(
        users.contains(USER),
        "admin bootstrap did not reach the core: {users}"
    );

    // And the stored hash must verify exactly once: the console looks the user up
    // over the remote backend and checks the password locally.
    let token = login(&format!("{console_base}/v3/auth/user/login")).await;
    assert!(
        !token.is_empty(),
        "login with the bootstrapped admin failed; the password was hashed twice \
         (once in the console, once in the core)"
    );

    let _ = std::fs::remove_dir_all(&core_store);
    let _ = std::fs::remove_dir_all(&console_store);
}

/// The console's **data plane** in independent deployment.
///
/// The guards above prove a separated console can authenticate, authorize and
/// manage users. What they do not touch is the part an operator actually uses:
/// browsing and editing configuration and services. Those go through
/// `ConsoleDataSource`, whose remote implementation talks to the core with the
/// maintainer client.
///
/// Because the console holds no database of its own, every byte it renders has
/// to have crossed the process boundary. So each write performed here *through
/// the console* is read back straight *from the core*, and each read through the
/// console is checked against what the core stores. A console that silently fell
/// back to a local empty store would fail every assertion.
#[tokio::test]
async fn console_remote_deployment_serves_config_and_naming_data_plane() {
    let bin = match find_server_binary() {
        Some(b) => b,
        None => {
            eprintln!("SKIP: batata-server binary not found");
            return;
        }
    };
    let root = workspace_root();

    let core_port = 9648u16;
    let console_port = 9490u16;
    let console_main_port = 9491u16;

    let core_store = std::env::temp_dir().join(format!("batata-it-dp-core-{}", std::process::id()));
    let console_store =
        std::env::temp_dir().join(format!("batata-it-dp-console-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&core_store);
    let _ = std::fs::remove_dir_all(&console_store);

    let core = Command::new(&bin)
        .current_dir(&root)
        .args([
            "-m",
            "standalone",
            "-d",
            "server",
            &format!("--batata.server.main.port={core_port}"),
            "--batata.core.auth.enabled=true",
            &format!(
                "--batata.persistence.embedded.data_dir={}",
                core_store.display()
            ),
        ])
        .stdout(std::fs::File::create("/tmp/it-dp-core.log").unwrap())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn core");
    let _core_guard = ChildGuard { child: core };

    let core_base = format!("http://127.0.0.1:{core_port}");
    assert!(
        wait_for_http(
            &core_base,
            "/nacos/v3/admin/core/state/liveness",
            Duration::from_secs(90)
        )
        .await,
        "core server did not become ready"
    );
    seed_admin(&core_base).await;

    let console = Command::new(&bin)
        .current_dir(&root)
        .args([
            "-m",
            "standalone",
            "-d",
            "server",
            "--batata.deployment.type=console",
            &format!("--batata.console.remote.server_addr={core_base}"),
            // `conf/cluster.conf` in the workspace root would otherwise win and
            // send every maintainer call to 8848.
            &format!("--batata.member.list=127.0.0.1:{core_port}"),
            &format!("--batata.console.port={console_port}"),
            &format!("--batata.server.main.port={console_main_port}"),
            "--batata.core.auth.enabled=true",
            "--batata.core.auth.console.enabled=true",
            &format!(
                "--batata.persistence.embedded.data_dir={}",
                console_store.display()
            ),
        ])
        .stdout(std::fs::File::create("/tmp/it-dp-console.log").unwrap())
        .stderr(std::process::Stdio::null())
        .spawn()
        .expect("spawn console-remote");
    let _console_guard = ChildGuard { child: console };

    let console_base = format!("http://127.0.0.1:{console_port}");
    assert!(
        wait_for_http(
            &console_base,
            "/v3/console/health/liveness",
            Duration::from_secs(90)
        )
        .await,
        "console-remote did not become ready"
    );

    let token = login(&format!("{console_base}/v3/auth/user/login")).await;
    assert!(!token.is_empty(), "console login returned an empty token");
    let core_token = login(&format!("{core_base}/nacos/v3/auth/user/login")).await;
    assert!(!core_token.is_empty(), "core login returned an empty token");

    let ns = "it-dp-ns";
    let data_id = "it-dp-app.yaml";
    let group = "DEFAULT_GROUP";
    let config_qs = format!("dataId={data_id}&groupName={group}&namespaceId={ns}");

    // --- Namespace: written through the console ---
    let (status, body) = post_form(
        &format!("{console_base}/v3/console/core/namespace"),
        &token,
        &[
            ("customNamespaceId", ns),
            ("namespaceName", ns),
            ("namespaceDesc", "data-plane"),
        ],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "console could not create a namespace through the core: {body}"
    );

    // --- Config publish: written through the console, read from the core ---
    let (status, body) = post_form(
        &format!("{console_base}/v3/console/cs/config"),
        &token,
        &[
            ("dataId", data_id),
            ("groupName", group),
            ("namespaceId", ns),
            ("content", "batata=v1"),
            ("type", "yaml"),
            ("appName", ""),
            ("configTags", ""),
            ("desc", "data-plane"),
        ],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "console could not publish a config through the core: {body}"
    );

    let on_core = get_with(
        &format!("{core_base}/nacos/v3/admin/cs/config?{config_qs}"),
        &core_token,
    )
    .await;
    assert!(
        on_core.1.contains("batata=v1"),
        "config published through the console is not stored on the core: {:?}",
        on_core
    );

    // --- Config read: through the console, against what the core stores ---
    let via_console = get_with(
        &format!("{console_base}/v3/console/cs/config?{config_qs}"),
        &token,
    )
    .await;
    assert_eq!(
        via_console.0,
        StatusCode::OK,
        "console could not read the config back: {:?}",
        via_console
    );
    assert!(
        via_console.1.contains("batata=v1") && via_console.1.contains("\"code\":0"),
        "console read did not return the core's content: {:?}",
        via_console
    );

    // --- Config search through the console ---
    let search = get_with(
        &format!("{console_base}/v3/console/cs/config/list?{config_qs}&pageNo=1&pageSize=10"),
        &token,
    )
    .await;
    assert_eq!(
        search.0,
        StatusCode::OK,
        "console config search failed: {:?}",
        search
    );
    assert!(
        search.1.contains(data_id) && search.1.contains("\"code\":0"),
        "console config search did not return the published data id: {:?}",
        search
    );

    // --- Config update through the console lands on the core ---
    let (status, body) = post_form(
        &format!("{console_base}/v3/console/cs/config"),
        &token,
        &[
            ("dataId", data_id),
            ("groupName", group),
            ("namespaceId", ns),
            ("content", "batata=v2"),
            ("type", "yaml"),
            ("appName", ""),
            ("configTags", ""),
            ("desc", "data-plane"),
        ],
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "console could not update the config: {body}"
    );

    let updated = get_with(
        &format!("{core_base}/nacos/v3/admin/cs/config?{config_qs}"),
        &core_token,
    )
    .await;
    assert!(
        updated.1.contains("batata=v2"),
        "config update did not reach the core: {:?}",
        updated
    );

    // --- Config history through the console ---
    let history = get_with(
        &format!("{console_base}/v3/console/cs/history/list?{config_qs}&pageNo=1&pageSize=10"),
        &token,
    )
    .await;
    assert_eq!(
        history.0,
        StatusCode::OK,
        "console config history failed: {:?}",
        history
    );
    assert!(
        history.1.contains("\"code\":0") && history.1.contains(data_id),
        "console config history did not return the core's history rows: {:?}",
        history
    );

    // --- Naming: service created through the console, verified on the core ---
    let service = "it-dp-service";
    let (status, body) = post_json(
        &format!("{console_base}/v3/console/ns/service"),
        &token,
        serde_json::json!({
            "serviceName": service,
            "namespaceId": "public",
            "groupName": group,
        }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "console could not create a service through the core: {body}"
    );

    let service_on_core = get_with(
        &format!(
            "{core_base}/nacos/v3/admin/ns/service/list?serviceName={service}\
             &namespaceId=public&pageNo=1&pageSize=10"
        ),
        &core_token,
    )
    .await;
    assert!(
        service_on_core.1.contains(service),
        "service created through the console is not present on the core: {:?}",
        service_on_core
    );

    let service_via_console = get_with(
        &format!(
            "{console_base}/v3/console/ns/service/list?namespaceId=public&pageNo=1&pageSize=10"
        ),
        &token,
    )
    .await;
    assert!(
        service_via_console.1.contains(service),
        "console service list did not show the service stored on the core: {:?}",
        service_via_console
    );

    // --- Deletes travel the same path ---
    let (status, body) = delete_with(
        &format!("{console_base}/v3/console/cs/config?{config_qs}"),
        &token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "console could not delete the config: {body}"
    );
    let deleted = get_with(
        &format!("{core_base}/nacos/v3/admin/cs/config?{config_qs}"),
        &core_token,
    )
    .await;
    assert!(
        !deleted.1.contains("batata=v2"),
        "config deleted through the console is still on the core: {:?}",
        deleted
    );

    let (status, body) = delete_with(
        &format!(
            "{console_base}/v3/console/ns/service?serviceName={service}&namespaceId=public&groupName={group}"
        ),
        &token,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "console could not delete the service: {body}"
    );

    let _ = std::fs::remove_dir_all(&core_store);
    let _ = std::fs::remove_dir_all(&console_store);
}

/// GET a console endpoint and return the raw status plus body.
async fn get_with(url: &str, token: &str) -> (StatusCode, String) {
    let resp = reqwest::Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .expect("console GET request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// POST a form-encoded console call and return the raw status plus body.
async fn post_form(url: &str, token: &str, form: &[(&str, &str)]) -> (StatusCode, String) {
    let resp = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .form(form)
        .send()
        .await
        .expect("console POST request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// POST a JSON console call and return the raw status plus body.
async fn post_json(url: &str, token: &str, body: serde_json::Value) -> (StatusCode, String) {
    let resp = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .expect("console JSON POST request");
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    (status, text)
}

/// DELETE a console resource and return the raw status plus body.
async fn delete_with(url: &str, token: &str) -> (StatusCode, String) {
    let resp = reqwest::Client::new()
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .expect("console DELETE request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();
    (status, body)
}

/// POST a form-encoded management call with the end user's token.
async fn auth_form(url: &str, token: &str, form: &[(&str, &str)]) -> StatusCode {
    let resp = reqwest::Client::new()
        .post(url)
        .bearer_auth(token)
        .form(form)
        .send()
        .await
        .expect("auth management request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if status != StatusCode::OK {
        eprintln!("auth management POST {url} -> {status}: {body}");
    }

    status
}

/// GET a management resource with the end user's token.
async fn auth_get(url: &str, token: &str) -> String {
    let resp = reqwest::Client::new()
        .get(url)
        .bearer_auth(token)
        .send()
        .await
        .expect("auth management request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    assert_eq!(
        status,
        StatusCode::OK,
        "auth management GET {url} -> {body}"
    );
    assert!(
        body.contains("\"code\":0"),
        "auth management GET {url} returned a non-zero code: {body}"
    );

    body
}

/// DELETE a management resource with the end user's token.
async fn auth_delete(url: &str, token: &str) -> StatusCode {
    let resp = reqwest::Client::new()
        .delete(url)
        .bearer_auth(token)
        .send()
        .await
        .expect("auth management request");
    let status = resp.status();
    let body = resp.text().await.unwrap_or_default();

    if status != StatusCode::OK {
        eprintln!("auth management DELETE {url} -> {status}: {body}");
    }

    status
}
