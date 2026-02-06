//! File-based logging module inspired by Nacos logging structure.
//!
//! This module provides file-based logging with daily rotation:
//! - batata.log: Main server log (all components)
//!
//! For component-specific filtering, use RUST_LOG environment variable:
//! - RUST_LOG=batata_config=debug,batata_naming=info,info
//!
//! Log files are stored in `~/batata/logs` by default.
//! Override with BATATA_LOG_DIR environment variable.

use std::path::PathBuf;
use std::sync::Arc;

use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Registry, fmt};

/// Log file configuration
#[derive(Debug, Clone)]
pub struct LogFileConfig {
    /// Log directory path
    pub log_dir: PathBuf,
    /// Log file name prefix
    pub file_name: String,
    /// Log rotation policy
    pub rotation: LogRotation,
    /// Maximum log level for this file
    pub level: Level,
    /// Target filter (module path pattern)
    pub target_filter: Option<String>,
}

/// Log rotation policy
#[derive(Debug, Clone, Copy)]
pub enum LogRotation {
    /// Rotate daily
    Daily,
    /// Rotate hourly
    Hourly,
    /// Never rotate (single file)
    Never,
}

impl From<LogRotation> for Rotation {
    fn from(rotation: LogRotation) -> Self {
        match rotation {
            LogRotation::Daily => Rotation::DAILY,
            LogRotation::Hourly => Rotation::HOURLY,
            LogRotation::Never => Rotation::NEVER,
        }
    }
}

/// Logging configuration for the entire application
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Base log directory
    pub log_dir: PathBuf,
    /// Enable console output
    pub console_output: bool,
    /// Console log level
    pub console_level: Level,
    /// Enable file logging
    pub file_logging: bool,
    /// Default log level for files
    pub file_level: Level,
    /// Log rotation policy
    pub rotation: LogRotation,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
        Self {
            log_dir: PathBuf::from(format!("{}/batata/logs", home)),
            console_output: true,
            console_level: Level::INFO,
            file_logging: true,
            file_level: Level::INFO,
            rotation: LogRotation::Daily,
        }
    }
}

impl LoggingConfig {
    /// Create from environment variables
    pub fn from_env() -> Self {
        let log_dir = std::env::var("BATATA_LOG_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
                PathBuf::from(format!("{}/batata/logs", home))
            });

        let console_output = std::env::var("BATATA_LOG_CONSOLE")
            .map(|v| v.to_lowercase() != "false" && v != "0")
            .unwrap_or(true);

        let file_logging = std::env::var("BATATA_LOG_FILE")
            .map(|v| v.to_lowercase() == "true" || v == "1")
            .unwrap_or(true);

        let console_level = std::env::var("BATATA_LOG_LEVEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(Level::INFO);

        let file_level = std::env::var("BATATA_LOG_FILE_LEVEL")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(console_level);

        Self {
            log_dir,
            console_output,
            console_level,
            file_logging,
            file_level,
            rotation: LogRotation::Daily,
        }
    }

    /// Create from application configuration
    pub fn from_config(
        log_dir: Option<String>,
        console_output: bool,
        file_logging: bool,
        level: String,
    ) -> Self {
        let log_dir = log_dir.map(PathBuf::from).unwrap_or_else(|| {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            PathBuf::from(format!("{}/batata/logs", home))
        });

        let level = level.parse().unwrap_or(Level::INFO);

        Self {
            log_dir,
            console_output,
            console_level: level,
            file_logging,
            file_level: level,
            rotation: LogRotation::Daily,
        }
    }
}

/// Guard that keeps the logging system alive.
/// Must be kept alive for the duration of the application.
pub struct LoggingGuard {
    guards: Vec<WorkerGuard>,
}

impl LoggingGuard {
    fn new(guards: Vec<WorkerGuard>) -> Self {
        Self { guards }
    }

    /// Take ownership of the guards (for Arc wrapping)
    fn into_guards(self) -> Vec<WorkerGuard> {
        self.guards
    }
}

/// Initialize file-based logging with a single combined log file.
///
/// This creates a batata.log file with daily rotation, similar to Nacos.
/// Use RUST_LOG environment variable to control per-module logging levels.
///
/// Returns a guard that must be kept alive for the duration of the application.
pub fn init_file_logging(config: &LoggingConfig) -> Result<LoggingGuard, Box<dyn std::error::Error>> {
    // Create log directory if it doesn't exist
    if config.file_logging {
        std::fs::create_dir_all(&config.log_dir)?;
    }

    let mut guards = Vec::new();

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(config.console_level.to_string()));

    if config.file_logging && config.console_output {
        // Both file and console logging
        let file_appender = RollingFileAppender::new(
            config.rotation.into(),
            &config.log_dir,
            "batata.log",
        );
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        guards.push(guard);

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_target(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false);

        let console_layer = fmt::layer()
            .with_target(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true);

        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .with(file_layer)
            .try_init()?;
    } else if config.file_logging {
        // File logging only
        let file_appender = RollingFileAppender::new(
            config.rotation.into(),
            &config.log_dir,
            "batata.log",
        );
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        guards.push(guard);

        let file_layer = fmt::layer()
            .with_writer(non_blocking)
            .with_target(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false);

        Registry::default()
            .with(env_filter)
            .with(file_layer)
            .try_init()?;
    } else {
        // Console logging only
        let console_layer = fmt::layer()
            .with_target(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true);

        Registry::default()
            .with(env_filter)
            .with(console_layer)
            .try_init()?;
    }

    if config.file_logging {
        // Print log directory info (can't use tracing yet since it's just initialized)
        eprintln!(
            "File logging initialized. Log directory: {}",
            config.log_dir.display()
        );
    }

    Ok(LoggingGuard::new(guards))
}

/// Initialize simple file logging (alias for init_file_logging)
pub fn init_simple_file_logging(
    config: &LoggingConfig,
) -> Result<LoggingGuard, Box<dyn std::error::Error>> {
    init_file_logging(config)
}

/// Initialize multi-file logging with separate files for each component.
///
/// This creates the main batata.log file similar to Nacos.
/// For component-specific logging, use RUST_LOG environment variable:
/// - RUST_LOG=batata_config=debug,batata_naming=info,info
///
/// Returns a guard that must be kept alive for the duration of the application.
pub fn init_multi_file_logging(
    config: &LoggingConfig,
) -> Result<Arc<LoggingGuard>, Box<dyn std::error::Error>> {
    let guard = init_file_logging(config)?;
    Ok(Arc::new(LoggingGuard::new(guard.into_guards())))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_logging_config_default() {
        let config = LoggingConfig::default();
        assert!(config.console_output);
        assert!(config.file_logging);
        assert_eq!(config.console_level, Level::INFO);
    }

    #[test]
    fn test_log_rotation_conversion() {
        assert!(matches!(
            Rotation::from(LogRotation::Daily),
            Rotation::DAILY
        ));
        assert!(matches!(
            Rotation::from(LogRotation::Hourly),
            Rotation::HOURLY
        ));
        assert!(matches!(
            Rotation::from(LogRotation::Never),
            Rotation::NEVER
        ));
    }
}
