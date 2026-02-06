//! Database test utilities
//!
//! Provides utilities for database integration testing with MySQL and PostgreSQL.

use sea_orm::{Database, DatabaseConnection, DbErr};
use std::env;

/// Configuration for test database
#[derive(Clone, Debug)]
pub struct TestDatabaseConfig {
    /// Database URL
    pub url: String,
    /// Whether to run migrations
    pub run_migrations: bool,
    /// Whether to clean up after tests
    pub cleanup: bool,
}

impl TestDatabaseConfig {
    /// Create config from environment variable
    pub fn from_env() -> Option<Self> {
        env::var("TEST_DATABASE_URL").ok().map(|url| Self {
            url,
            run_migrations: true,
            cleanup: true,
        })
    }

    /// Create MySQL config
    pub fn mysql(host: &str, port: u16, database: &str, user: &str, password: &str) -> Self {
        Self {
            url: format!(
                "mysql://{}:{}@{}:{}/{}",
                user, password, host, port, database
            ),
            run_migrations: true,
            cleanup: true,
        }
    }

    /// Create PostgreSQL config
    pub fn postgres(host: &str, port: u16, database: &str, user: &str, password: &str) -> Self {
        Self {
            url: format!(
                "postgres://{}:{}@{}:{}/{}",
                user, password, host, port, database
            ),
            run_migrations: true,
            cleanup: true,
        }
    }

    /// Create MySQL config for Docker test environment
    pub fn mysql_docker() -> Self {
        Self::mysql("127.0.0.1", 3307, "batata_test", "batata", "batata")
    }

    /// Create PostgreSQL config for Docker test environment
    pub fn postgres_docker() -> Self {
        Self::postgres("127.0.0.1", 5433, "batata_test", "batata", "batata")
    }

    /// Set whether to run migrations
    pub fn with_migrations(mut self, run: bool) -> Self {
        self.run_migrations = run;
        self
    }

    /// Set whether to cleanup after tests
    pub fn with_cleanup(mut self, cleanup: bool) -> Self {
        self.cleanup = cleanup;
        self
    }
}

/// Test database wrapper
pub struct TestDatabase {
    /// Database connection
    pub connection: DatabaseConnection,
    /// Configuration
    config: TestDatabaseConfig,
}

impl TestDatabase {
    /// Connect to test database
    pub async fn connect(config: TestDatabaseConfig) -> Result<Self, TestDatabaseError> {
        let connection = Database::connect(&config.url)
            .await
            .map_err(|e| TestDatabaseError::ConnectionFailed(e.to_string()))?;

        let db = Self { connection, config };

        if db.config.run_migrations {
            db.run_migrations().await?;
        }

        Ok(db)
    }

    /// Connect to MySQL test database (Docker environment)
    pub async fn mysql() -> Result<Self, TestDatabaseError> {
        Self::connect(TestDatabaseConfig::mysql_docker()).await
    }

    /// Connect to PostgreSQL test database (Docker environment)
    pub async fn postgres() -> Result<Self, TestDatabaseError> {
        Self::connect(TestDatabaseConfig::postgres_docker()).await
    }

    /// Connect using environment variable
    pub async fn from_env() -> Result<Self, TestDatabaseError> {
        let config =
            TestDatabaseConfig::from_env().ok_or(TestDatabaseError::NoEnvironmentVariable)?;
        Self::connect(config).await
    }

    /// Get database connection
    pub fn conn(&self) -> &DatabaseConnection {
        &self.connection
    }

    /// Get database URL
    pub fn url(&self) -> &str {
        &self.config.url
    }

    /// Check if using MySQL
    pub fn is_mysql(&self) -> bool {
        self.config.url.starts_with("mysql://")
    }

    /// Check if using PostgreSQL
    pub fn is_postgres(&self) -> bool {
        self.config.url.starts_with("postgres://")
    }

    /// Run database migrations (execute schema SQL)
    pub async fn run_migrations(&self) -> Result<(), TestDatabaseError> {
        // Load schema based on database type
        let schema_sql = if self.is_mysql() {
            include_str!("../../../../conf/mysql-schema.sql")
        } else if self.is_postgres() {
            include_str!("../../../../conf/postgresql-schema.sql")
        } else {
            return Err(TestDatabaseError::UnsupportedDatabase);
        };

        // Split by semicolon and execute each statement
        for statement in schema_sql.split(';') {
            let statement = statement.trim();
            if statement.is_empty()
                || statement.starts_with("--")
                || statement.starts_with("/*")
            {
                continue;
            }

            // Skip DROP statements in tests to avoid accidental data loss
            if statement.to_uppercase().starts_with("DROP") {
                continue;
            }

            // Use CREATE TABLE IF NOT EXISTS for safety
            let safe_statement = if statement.to_uppercase().contains("CREATE TABLE")
                && !statement.to_uppercase().contains("IF NOT EXISTS")
            {
                statement.replacen("CREATE TABLE", "CREATE TABLE IF NOT EXISTS", 1)
            } else {
                statement.to_string()
            };

            if !safe_statement.is_empty() {
                use sea_orm::{ConnectionTrait, Statement};
                let db_backend = self.connection.get_database_backend();
                self.connection
                    .execute(Statement::from_string(db_backend, safe_statement))
                    .await
                    .map_err(|e| TestDatabaseError::MigrationFailed(e.to_string()))?;
            }
        }

        Ok(())
    }

    /// Truncate all tables (for cleanup between tests)
    pub async fn truncate_all(&self) -> Result<(), TestDatabaseError> {
        let tables = vec![
            "config_info",
            "config_info_gray",
            "config_info_aggr",
            "config_tags_relation",
            "his_config_info",
            "service_info",
            "cluster_info",
            "instance_info",
            "tenant_info",
            "users",
            "roles",
            "permissions",
            "group_capacity",
            "tenant_capacity",
            "consul_acl_tokens",
            "consul_acl_policies",
            "operation_log",
        ];

        use sea_orm::{ConnectionTrait, Statement};
        let db_backend = self.connection.get_database_backend();

        // Disable foreign key checks
        if self.is_mysql() {
            self.connection
                .execute(Statement::from_string(
                    db_backend,
                    "SET FOREIGN_KEY_CHECKS = 0".to_string(),
                ))
                .await
                .ok();
        } else if self.is_postgres() {
            // PostgreSQL uses TRUNCATE ... CASCADE
        }

        for table in tables {
            let sql = if self.is_mysql() {
                format!("TRUNCATE TABLE {}", table)
            } else {
                format!("TRUNCATE TABLE {} CASCADE", table)
            };

            // Ignore errors for tables that might not exist
            self.connection
                .execute(Statement::from_string(db_backend, sql))
                .await
                .ok();
        }

        // Re-enable foreign key checks
        if self.is_mysql() {
            self.connection
                .execute(Statement::from_string(
                    db_backend,
                    "SET FOREIGN_KEY_CHECKS = 1".to_string(),
                ))
                .await
                .ok();
        }

        Ok(())
    }

    /// Clean up test data (called on drop if configured)
    pub async fn cleanup(&self) -> Result<(), TestDatabaseError> {
        if self.config.cleanup {
            self.truncate_all().await?;
        }
        Ok(())
    }
}

/// Errors that can occur when managing test database
#[derive(Debug)]
pub enum TestDatabaseError {
    /// Connection failed
    ConnectionFailed(String),
    /// Migration failed
    MigrationFailed(String),
    /// Unsupported database type
    UnsupportedDatabase,
    /// No TEST_DATABASE_URL environment variable
    NoEnvironmentVariable,
    /// Query execution failed
    QueryFailed(String),
}

impl std::fmt::Display for TestDatabaseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectionFailed(e) => write!(f, "Database connection failed: {}", e),
            Self::MigrationFailed(e) => write!(f, "Migration failed: {}", e),
            Self::UnsupportedDatabase => write!(f, "Unsupported database type"),
            Self::NoEnvironmentVariable => write!(f, "TEST_DATABASE_URL not set"),
            Self::QueryFailed(e) => write!(f, "Query failed: {}", e),
        }
    }
}

impl std::error::Error for TestDatabaseError {}

impl From<DbErr> for TestDatabaseError {
    fn from(err: DbErr) -> Self {
        TestDatabaseError::QueryFailed(err.to_string())
    }
}

/// Helper macro to skip test if database is not available
#[macro_export]
macro_rules! skip_if_no_db {
    () => {
        if std::env::var("TEST_DATABASE_URL").is_err() {
            eprintln!("Skipping test: TEST_DATABASE_URL not set");
            return;
        }
    };
}

/// Helper macro to run test with both MySQL and PostgreSQL
#[macro_export]
macro_rules! test_both_databases {
    ($test_fn:ident) => {
        paste::paste! {
            #[tokio::test]
            #[ignore = "requires MySQL database"]
            async fn [<$test_fn _mysql>]() {
                let db = TestDatabase::mysql().await.expect("MySQL connection failed");
                $test_fn(&db).await;
            }

            #[tokio::test]
            #[ignore = "requires PostgreSQL database"]
            async fn [<$test_fn _postgres>]() {
                let db = TestDatabase::postgres().await.expect("PostgreSQL connection failed");
                $test_fn(&db).await;
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_mysql() {
        let config = TestDatabaseConfig::mysql("localhost", 3306, "test", "user", "pass");
        assert!(config.url.starts_with("mysql://"));
        assert!(config.url.contains("localhost:3306"));
    }

    #[test]
    fn test_config_postgres() {
        let config = TestDatabaseConfig::postgres("localhost", 5432, "test", "user", "pass");
        assert!(config.url.starts_with("postgres://"));
        assert!(config.url.contains("localhost:5432"));
    }

    #[test]
    fn test_docker_configs() {
        let mysql = TestDatabaseConfig::mysql_docker();
        assert!(mysql.url.contains("3307"));

        let postgres = TestDatabaseConfig::postgres_docker();
        assert!(postgres.url.contains("5433"));
    }
}
