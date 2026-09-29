//! Shared helpers for the live-database tests.

use std::time::Duration;

use batata_persistence::sea_orm::{ConnectOptions, Database, DatabaseConnection};

/// How long to wait before declaring the test database unreachable.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(3);

/// Connect to the test database, failing fast with an actionable message.
///
/// A stopped Podman container leaves the forwarded port *accepting* TCP while
/// nothing completes the handshake, so a plain `Database::connect` blocks for
/// the default ~30s and only then reports a pool timeout. Across a file with
/// fifteen tests that is several minutes of silence before anything fails, and
/// the resulting message reads like a code bug rather than a missing database.
///
/// A short timeout surfaces the same situation in seconds, and the panic names
/// the likely cause.
pub async fn connect_database(url: &str) -> DatabaseConnection {
    let mut options = ConnectOptions::new(url.to_string());
    options
        .connect_timeout(CONNECT_TIMEOUT)
        .acquire_timeout(CONNECT_TIMEOUT);

    match Database::connect(options).await {
        Ok(connection) => connection,
        Err(error) => panic!(
            "cannot reach the test database at {url}: {error}\n\
             hint: containers do not come back on their own after a machine \
             restart or a podman machine stop — try `podman start mysql postgres`"
        ),
    }
}
