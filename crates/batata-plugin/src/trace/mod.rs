//! Trace Plugin - Event subscription for observability
//!
//! Mirrors Nacos `NacosTraceSubscriber` / `NacosTracePluginManager`.
//! External plugins register into [`TraceSubscriberRegistry`] at startup;
//! core/naming/config fire [`TraceEvent`]s at key call sites.

mod default;
mod event;
mod registry;

pub use default::LoggingTraceSubscriber;
pub use event::{TraceEvent, TraceEventKind};
pub use registry::{TraceSubscriber, TraceSubscriberRegistry, global_trace_registry};
