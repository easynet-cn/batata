//! Distro filter middleware for AP mode request routing.
//!
//! In cluster mode, ephemeral instance operations (register/deregister/update)
//! should be handled by the responsible node determined by consistent hashing.
//! This middleware intercepts naming write requests and proxies them to the
//! responsible node if the current node is not responsible.

use std::future::{Ready, ready};
use std::sync::Arc;

use actix_service::{Service, Transform};
use actix_web::dev::{ServiceRequest, ServiceResponse};
use actix_web::{Error, HttpResponse};
use futures::future::LocalBoxFuture;

use batata_core::service::distro::DistroProtocol;

/// Header set on proxied requests to prevent redirect loops.
const NACOS_SERVER_HEADER: &str = "Nacos-Server";

/// Extract the distro key (namespace@@group@@serviceName) from query params.
fn extract_distro_key(query: &str) -> Option<String> {
    let mut namespace = "";
    let mut group = "";
    let mut service_name = "";

    for part in query.split('&') {
        if let Some((key, value)) = part.split_once('=') {
            match key {
                "namespaceId" | "namespace" => namespace = value,
                "groupName" | "group" => group = value,
                "serviceName" => service_name = value,
                _ => {}
            }
        }
    }

    if service_name.is_empty() {
        return None;
    }

    Some(format!(
        "{}@@{}@@{}",
        if namespace.is_empty() {
            "public"
        } else {
            namespace
        },
        if group.is_empty() {
            "DEFAULT_GROUP"
        } else {
            group
        },
        service_name
    ))
}

/// Check if a request is a naming instance write operation that requires distro routing.
fn is_naming_write_request(method: &str, path: &str) -> bool {
    if !path.contains("/ns/instance") {
        return false;
    }
    // Only writes need routing; reads can be served by any node
    matches!(method, "POST" | "PUT" | "DELETE")
}

/// Actix middleware that routes naming write requests to the responsible node.
///
/// If no DistroProtocol is configured (standalone mode), requests pass through.
pub struct DistroFilter {
    distro_protocol: Option<Arc<DistroProtocol>>,
}

impl DistroFilter {
    pub fn new(distro_protocol: Option<Arc<DistroProtocol>>) -> Self {
        Self { distro_protocol }
    }
}

impl<S, B> Transform<S, ServiceRequest> for DistroFilter
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type Transform = DistroFilterService<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(DistroFilterService {
            service,
            distro_protocol: self.distro_protocol.clone(),
        }))
    }
}

pub struct DistroFilterService<S> {
    service: S,
    distro_protocol: Option<Arc<DistroProtocol>>,
}

impl<S, B> Service<ServiceRequest> for DistroFilterService<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<actix_web::body::EitherBody<B>>;
    type Error = Error;
    type Future = LocalBoxFuture<'static, Result<Self::Response, Self::Error>>;

    fn poll_ready(
        &self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.service.poll_ready(cx)
    }

    fn call(&self, req: ServiceRequest) -> Self::Future {
        // Fast path: no distro protocol (standalone mode)
        let Some(distro) = self.distro_protocol.clone() else {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        };

        let method = req.method().as_str().to_string();
        let path = req.path().to_string();
        let query = req.query_string().to_string();

        // Only intercept naming instance write requests
        if !is_naming_write_request(&method, &path) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        // Check if this is already a proxied request (prevent loops)
        let is_proxied = req.headers().contains_key(NACOS_SERVER_HEADER);
        if is_proxied {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        // Extract the distro key from query params
        let Some(distro_key) = extract_distro_key(&query) else {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        };

        let local_address = distro.local_address().to_string();
        let mapper = distro.mapper().clone();

        // Check if this node is responsible
        if mapper.is_responsible(&distro_key, &local_address) {
            let fut = self.service.call(req);
            return Box::pin(async move {
                let res = fut.await?;
                Ok(res.map_into_left_body())
            });
        }

        // Not responsible — return redirect info to the responsible node
        let responsible_node = mapper
            .responsible_node(&distro_key)
            .unwrap_or(local_address);

        let fut = self.service.call(req);
        Box::pin(async move {
            tracing::debug!(
                distro_key = %distro_key,
                responsible = %responsible_node,
                "Distro: request should be handled by another node"
            );

            // Return 307 Temporary Redirect to the responsible node
            // The client (SDK or load balancer) should retry at the correct node
            let redirect_url = format!("http://{}{}", responsible_node, path);
            let response = HttpResponse::TemporaryRedirect()
                .insert_header(("Location", redirect_url))
                .insert_header(("X-Distro-Responsible", responsible_node))
                .json(serde_json::json!({
                    "code": 307,
                    "message": "Request routed to responsible node",
                    "data": serde_json::Value::Null
                }));
            let res = fut.await?;
            Ok(res.into_response(response).map_into_right_body())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_distro_key() {
        assert_eq!(
            extract_distro_key("namespaceId=public&groupName=DEFAULT_GROUP&serviceName=my-service"),
            Some("public@@DEFAULT_GROUP@@my-service".to_string())
        );
    }

    #[test]
    fn test_extract_distro_key_defaults() {
        assert_eq!(
            extract_distro_key("serviceName=svc1"),
            Some("public@@DEFAULT_GROUP@@svc1".to_string())
        );
    }

    #[test]
    fn test_extract_distro_key_missing_service() {
        assert_eq!(
            extract_distro_key("namespaceId=public&groupName=DEFAULT_GROUP"),
            None
        );
    }

    #[test]
    fn test_is_naming_write_request() {
        assert!(is_naming_write_request("POST", "/nacos/v2/ns/instance"));
        assert!(is_naming_write_request("PUT", "/nacos/v2/ns/instance"));
        assert!(is_naming_write_request("DELETE", "/nacos/v2/ns/instance"));
        assert!(!is_naming_write_request("GET", "/nacos/v2/ns/instance"));
        assert!(!is_naming_write_request(
            "GET",
            "/nacos/v2/ns/instance/list"
        ));
        assert!(!is_naming_write_request("POST", "/nacos/v2/cs/config"));
    }
}
