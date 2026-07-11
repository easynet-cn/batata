use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApolloPluginConfig {
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_http_workers")]
    pub http_workers: usize,
}

fn default_enabled() -> bool {
    false
}

fn default_port() -> u16 {
    8080
}

fn default_http_workers() -> usize {
    4
}

impl Default for ApolloPluginConfig {
    fn default() -> Self {
        Self {
            enabled: default_enabled(),
            port: default_port(),
            http_workers: default_http_workers(),
        }
    }
}

impl ApolloPluginConfig {
    pub fn from_config(config: &config::Config) -> Self {
        let enabled = config
            .get_bool("batata.plugin.apollo.enabled")
            .unwrap_or(true);
        let port = config.get_int("batata.plugin.apollo.port").unwrap_or(8080) as u16;
        let http_workers = {
            let v = config
                .get_int("batata.plugin.apollo.http.workers")
                .unwrap_or(0) as usize;
            if v == 0 {
                let cpus = std::thread::available_parallelism()
                    .map(|n| n.get())
                    .unwrap_or(4);
                std::cmp::min(4, cpus / 2).max(2)
            } else {
                v
            }
        };

        Self {
            enabled,
            port,
            http_workers,
        }
    }
}
