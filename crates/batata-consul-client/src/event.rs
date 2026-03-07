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
            params.push(("node".to_string(), node_filter.to_string()));
        }
        if !service_filter.is_empty() {
            params.push(("service".to_string(), service_filter.to_string()));
        }
        if !tag_filter.is_empty() {
            params.push(("tag".to_string(), tag_filter.to_string()));
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
            extra.push(("name".to_string(), name.to_string()));
        }
        self.get_with_extra("/v1/event/list", opts, &extra).await
    }
}
