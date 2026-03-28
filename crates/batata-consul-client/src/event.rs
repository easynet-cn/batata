use crate::client::ConsulClient;
use crate::error::Result;
use crate::model::{QueryMeta, QueryOptions, UserEvent, WriteMeta, WriteOptions};

/// Event API operations
impl ConsulClient {
    /// Fire a user event
    pub async fn event_fire(
        &self,
        name: &str,
        payload: Option<&[u8]>,
        node_filter: &str,
        service_filter: &str,
        tag_filter: &str,
        opts: &WriteOptions,
    ) -> Result<(UserEvent, WriteMeta)> {
        let path = format!("/v1/event/fire/{}", name);
        let mut params = Vec::new();
        if !node_filter.is_empty() {
            params.push(("node", node_filter.to_string()));
        }
        if !service_filter.is_empty() {
            params.push(("service", service_filter.to_string()));
        }
        if !tag_filter.is_empty() {
            params.push(("tag", tag_filter.to_string()));
        }

        let start = std::time::Instant::now();
        let mut req_params = Vec::new();
        self.apply_write_options(&mut req_params, opts);
        req_params.extend(params);

        let token = self.effective_token(&opts.token);
        let url = self.url(&path);

        let mut req = self.client.put(&url);
        if !token.is_empty() {
            req = req.header("X-Consul-Token", token);
        }
        if !req_params.is_empty() {
            req = req.query(&req_params);
        }
        if let Some(p) = payload {
            req = req.body(p.to_vec());
        }

        let response = req.send().await.map_err(crate::error::ConsulError::Http)?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let body = response.text().await.unwrap_or_default();
            return Err(crate::error::ConsulError::Api {
                status,
                message: body,
            });
        }

        let event = response
            .json::<UserEvent>()
            .await
            .map_err(crate::error::ConsulError::Http)?;
        let meta = WriteMeta {
            request_time: start.elapsed(),
        };
        Ok((event, meta))
    }

    /// List recent events, optionally filtered by name
    pub async fn event_list(
        &self,
        name: &str,
        opts: &QueryOptions,
    ) -> Result<(Vec<UserEvent>, QueryMeta)> {
        let mut extra = Vec::new();
        if !name.is_empty() {
            extra.push(("name", name.to_string()));
        }
        self.get_with_extra("/v1/event/list", opts, &extra).await
    }

    /// Convert an event UUID to a Consul index.
    ///
    /// This is a pure computation (no HTTP call). It takes the UUID string
    /// (e.g., "b54fe110-7af5-cafc-d1eb-2c6a526ab502") and XORs the lower
    /// and upper halves to produce a 64-bit index, matching the Go client's
    /// `IDToIndex` behavior.
    pub fn event_id_to_index(uuid: &str) -> u64 {
        // UUID format: xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx
        // lower = [0:8] + [9:13] + [14:18] = 16 hex chars
        // upper = [19:23] + [24:36] = 16 hex chars
        let lower = format!("{}{}{}", &uuid[0..8], &uuid[9..13], &uuid[14..18]);
        let upper = format!("{}{}", &uuid[19..23], &uuid[24..36]);

        let low_val = u64::from_str_radix(&lower, 16).expect("failed to parse lower UUID half");
        let high_val = u64::from_str_radix(&upper, 16).expect("failed to parse upper UUID half");

        low_val ^ high_val
    }
}
