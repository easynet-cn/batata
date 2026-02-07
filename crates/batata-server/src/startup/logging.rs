//! File-based logging module inspired by Nacos logging structure.
//!
//! This module provides multi-file logging similar to Nacos, where different
//! components write to separate log files with daily rotation:
//!
//! | Log File             | Component                        | Target Prefixes                     |
//! |---------------------|----------------------------------|-------------------------------------|
//! | batata.log          | Root logger (all components)      | (all)                               |
//! | naming-server.log   | Service discovery                 | batata_naming                       |
//! | config-server.log   | Configuration management          | batata_config                       |
//! | nacos-cluster.log   | Cluster communication             | batata_core::cluster                |
//! | core-auth.log       | Authentication and authorization  | batata_auth                         |
//! | remote.log          | Remote/gRPC communication         | batata_api, batata_core::connection |
//! | console.log         | Console management API            | batata_console                      |
//! | protocol-raft.log   | Raft consensus protocol           | batata_consistency                  |
//! | plugin-control.log  | Plugin system                     | batata_plugin*                      |
//! | mesh.log            | Service mesh (xDS/Istio MCP)      | batata_mesh                         |
//! | persistence.log     | Database persistence              | batata_persistence                  |
//!
//! Log files are stored in `~/batata/logs` by default.
//! Override with `BATATA_LOG_DIR` environment variable or `nacos.logs.path` config.

use std::path::PathBuf;

use opentelemetry::trace::TracerProvider;
use opentelemetry_sdk::trace::SdkTracerProvider;
use tracing::Level;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{RollingFileAppender, Rotation};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer, Registry, fmt};

use super::telemetry::OtelConfig;

// ---------------------------------------------------------------------------
// Component log file definitions (inspired by Nacos nacos-logback.xml)
// ---------------------------------------------------------------------------

/// Internal definition for a component log file.
struct ComponentLogDef {
    /// Log file name (e.g. "naming-server.log")
    file_name: &'static str,
    /// Target module prefixes routed to this file
    targets: &'static [&'static str],
}

/// Component log definitions matching Nacos's per-module logging strategy.
///
/// Each entry produces a separate rolling log file. Events are routed based on
/// their `tracing` target (Rust module path). The root `batata.log` file always
/// captures *all* events regardless of target.
const COMPONENT_LOGS: &[ComponentLogDef] = &[
    // Nacos: naming-server.log
    ComponentLogDef {
        file_name: "naming-server.log",
        targets: &["batata_naming"],
    },
    // Nacos: config-server.log
    ComponentLogDef {
        file_name: "config-server.log",
        targets: &["batata_config"],
    },
    // Nacos: nacos-cluster.log
    ComponentLogDef {
        file_name: "nacos-cluster.log",
        targets: &["batata_core::cluster"],
    },
    // Nacos: core-auth.log
    ComponentLogDef {
        file_name: "core-auth.log",
        targets: &[
            "batata_auth",
            "batata_server::auth",
            "batata_server::middleware",
        ],
    },
    // Nacos: remote.log (gRPC / connection layer)
    ComponentLogDef {
        file_name: "remote.log",
        targets: &[
            "batata_api",
            "batata_server::service",
            "batata_core::connection",
        ],
    },
    // Console management API
    ComponentLogDef {
        file_name: "console.log",
        targets: &["batata_console", "batata_server::console"],
    },
    // Nacos: protocol-raft.log
    ComponentLogDef {
        file_name: "protocol-raft.log",
        targets: &["batata_consistency"],
    },
    // Nacos: plugin-control.log
    ComponentLogDef {
        file_name: "plugin-control.log",
        targets: &[
            "batata_plugin",
            "batata_plugin_consul",
            "batata_plugin_apollo",
        ],
    },
    // Nacos: istio-main.log
    ComponentLogDef {
        file_name: "mesh.log",
        targets: &["batata_mesh"],
    },
    // Nacos: nacos-persistence.log
    ComponentLogDef {
        file_name: "persistence.log",
        targets: &["batata_persistence"],
    },
];

// ---------------------------------------------------------------------------
// Log rotation policy
// ---------------------------------------------------------------------------

/// Log rotation policy
#[derive(Debug, Clone, Copy)]
pub enum LogRotation {
    /// Rotate daily (default, matches Nacos)
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

// ---------------------------------------------------------------------------
// Logging configuration
// ---------------------------------------------------------------------------

/// Logging configuration for the entire application.
#[derive(Debug, Clone)]
pub struct LoggingConfig {
    /// Base log directory (default: `~/batata/logs`)
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
    /// Create from environment variables.
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

    /// Create from application configuration.
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

// ---------------------------------------------------------------------------
// Logging guard
// ---------------------------------------------------------------------------

/// Guard that keeps the logging system alive.
///
/// Holds file appender worker guards and optional OpenTelemetry provider.
/// Must be kept alive for the duration of the application. When dropped,
/// the OTEL provider is shut down and all buffered log output is flushed.
pub struct LoggingGuard {
    _file_guards: Vec<WorkerGuard>,
    _otel_provider: Option<SdkTracerProvider>,
}

impl Drop for LoggingGuard {
    fn drop(&mut self) {
        // Shutdown OTEL provider first so any final spans are exported
        if let Some(provider) = self._otel_provider.take()
            && let Err(e) = provider.shutdown()
        {
            eprintln!("Failed to shutdown OpenTelemetry tracer provider: {:?}", e);
        }
        // WorkerGuards are dropped automatically, flushing remaining log output
    }
}

// ---------------------------------------------------------------------------
// Initialization functions
// ---------------------------------------------------------------------------

/// Initialize the logging system with multi-file output and optional OpenTelemetry.
///
/// This sets up:
/// - Console output (optional, human-readable format with colors)
/// - Root log file `batata.log` that captures **all** events
/// - Component-specific log files with target-based routing (see [`COMPONENT_LOGS`])
/// - OpenTelemetry distributed tracing (optional)
///
/// The global `RUST_LOG` env var controls the **minimum** level for all layers.
/// Component log files use per-layer [`Targets`] filters to route events by
/// their tracing target (module path).
///
/// # Returns
///
/// A [`LoggingGuard`] that must be kept alive for the duration of the application.
pub fn init_logging(
    config: &LoggingConfig,
    otel_config: Option<&OtelConfig>,
) -> Result<LoggingGuard, Box<dyn std::error::Error>> {
    // NOTE: Do NOT call LogTracer::init() here — try_init() below already
    // does this automatically via the `tracing-log` default feature of
    // tracing-subscriber.

    // Create log directory if needed
    if config.file_logging {
        std::fs::create_dir_all(&config.log_dir)?;
    }

    let mut guards: Vec<WorkerGuard> = Vec::new();
    let mut layers: Vec<Box<dyn Layer<Registry> + Send + Sync>> = Vec::new();

    // --- Console layer (human-readable with ANSI colors, per-layer EnvFilter) ---
    if config.console_output {
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(config.console_level.to_string()));
        let console_layer = fmt::layer()
            .with_target(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_filter(filter);
        layers.push(Box::new(console_layer));
    }

    // --- File layers ---
    if config.file_logging {
        // Root log file: batata.log (captures all events, per-layer EnvFilter)
        let root_appender =
            RollingFileAppender::new(config.rotation.into(), &config.log_dir, "batata.log");
        let (root_nb, root_guard) = tracing_appender::non_blocking(root_appender);
        guards.push(root_guard);

        let root_filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new(config.file_level.to_string()));
        let root_layer = fmt::layer()
            .with_writer(root_nb)
            .with_target(true)
            .with_thread_names(true)
            .with_file(true)
            .with_line_number(true)
            .with_ansi(false)
            .with_filter(root_filter);
        layers.push(Box::new(root_layer));

        // Component-specific log files with per-layer Targets filtering
        for component in COMPONENT_LOGS {
            let appender = RollingFileAppender::new(
                config.rotation.into(),
                &config.log_dir,
                component.file_name,
            );
            let (nb, guard) = tracing_appender::non_blocking(appender);
            guards.push(guard);

            // Build a Targets filter that matches all target prefixes for this component.
            // Uses TRACE level so component files capture everything from their targets;
            // the root file and console use EnvFilter/RUST_LOG for level control.
            let mut targets = Targets::new();
            for target in component.targets {
                targets = targets.with_target(*target, LevelFilter::TRACE);
            }

            let layer = fmt::layer()
                .with_writer(nb)
                .with_target(true)
                .with_thread_names(true)
                .with_file(true)
                .with_line_number(true)
                .with_ansi(false)
                .with_filter(targets);
            layers.push(Box::new(layer));
        }
    }

    // --- Optional OpenTelemetry layer (with per-layer level filter) ---
    let otel_provider = match otel_config {
        Some(otel_cfg) if otel_cfg.enabled => {
            match super::telemetry::init_tracer_provider(otel_cfg) {
                Ok(provider) => {
                    let tracer = provider.tracer("batata");
                    let otel_filter: LevelFilter = config.file_level.into();
                    layers.push(Box::new(
                        OpenTelemetryLayer::new(tracer).with_filter(otel_filter),
                    ));
                    Some(provider)
                }
                Err(e) => {
                    eprintln!(
                        "Failed to initialize OpenTelemetry: {}. Continuing without it.",
                        e
                    );
                    None
                }
            }
        }
        _ => None,
    };

    // --- Initialize the global tracing subscriber ---
    // All filtering is per-layer (no global EnvFilter), so each layer
    // independently decides which events to process.
    Registry::default()
        .with(layers)
        .try_init()
        .map_err(|e| format!("Failed to initialize logging: {}", e))?;

    // Log initialization summary (now that the subscriber is active)
    if config.file_logging {
        tracing::info!(
            log_dir = %config.log_dir.display(),
            component_files = COMPONENT_LOGS.len(),
            "File logging initialized: batata.log (root) + {} component log files",
            COMPONENT_LOGS.len()
        );
        for component in COMPONENT_LOGS {
            tracing::debug!(
                file = component.file_name,
                targets = ?component.targets,
                "Registered component log file"
            );
        }
    }

    Ok(LoggingGuard {
        _file_guards: guards,
        _otel_provider: otel_provider,
    })
}

/// Initialize simple file logging without OpenTelemetry.
///
/// Convenience wrapper around [`init_logging`] with OTEL disabled.
pub fn init_file_logging(
    config: &LoggingConfig,
) -> Result<LoggingGuard, Box<dyn std::error::Error>> {
    init_logging(config, None)
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
        assert_eq!(config.file_level, Level::INFO);
    }

    #[test]
    fn test_logging_config_from_config() {
        let config = LoggingConfig::from_config(
            Some("/tmp/test-logs".to_string()),
            false,
            true,
            "debug".to_string(),
        );
        assert_eq!(config.log_dir, PathBuf::from("/tmp/test-logs"));
        assert!(!config.console_output);
        assert!(config.file_logging);
        assert_eq!(config.file_level, Level::DEBUG);
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

    #[test]
    fn test_component_log_definitions() {
        // Verify all component log definitions have valid file names and targets
        for component in COMPONENT_LOGS {
            assert!(
                component.file_name.ends_with(".log"),
                "Log file name should end with .log: {}",
                component.file_name
            );
            assert!(
                !component.targets.is_empty(),
                "Component {} should have at least one target",
                component.file_name
            );
        }
    }

    #[test]
    fn test_component_log_count() {
        // We should have 10 component log files matching Nacos categories
        assert_eq!(COMPONENT_LOGS.len(), 10);
    }
}
