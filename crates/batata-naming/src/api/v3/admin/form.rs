//! Shared form extraction for V3 admin naming handlers.

use actix_web::web;
use serde::de::DeserializeOwned;

/// Resolve an admin form from the URL query string and/or the request body.
///
/// Upstream binds these parameters with a Spring form object, which accepts
/// both encodings. The Nacos maintainer client used to send service/cluster
/// writes as an `application/x-www-form-urlencoded` body (`setParamValue`),
/// while 3.3.0 GA moved the same parameters into the URL query string
/// (`appendQuery`). Accepting both keeps the admin API compatible with either
/// SDK generation.
///
/// `Option<...>` extractors turn a deserialization failure into `None`, so a
/// query that is empty or only partially populated falls back to the body
/// instead of producing a raw 400 before the handler can build a proper
/// `Result` response.
pub(crate) fn merge_form<T: DeserializeOwned>(
    query: Option<web::Query<T>>,
    body: Option<web::Form<T>>,
) -> Option<T> {
    query
        .map(web::Query::into_inner)
        .or_else(|| body.map(web::Form::into_inner))
}

#[cfg(test)]
mod tests {
    use super::*;
    use actix_web::{FromRequest, test::TestRequest};
    use serde::Deserialize;

    #[derive(Debug, Deserialize, PartialEq)]
    #[serde(rename_all = "camelCase")]
    struct SampleForm {
        service_name: String,
        #[serde(default)]
        protect_threshold: Option<f64>,
    }

    fn sample(name: &str, threshold: f64) -> SampleForm {
        SampleForm {
            service_name: name.to_string(),
            protect_threshold: Some(threshold),
        }
    }

    /// Extract the query side with the real actix extractor, from a real URI.
    /// This is the shape the 3.3.0 GA maintainer client sends.
    async fn query_from_uri(uri: &str) -> Option<web::Query<SampleForm>> {
        let req = TestRequest::with_uri(uri).to_http_request();
        Option::<web::Query<SampleForm>>::extract(&req)
            .await
            .expect("Option extractor cannot fail")
    }

    #[actix_web::test]
    async fn query_only_request_is_accepted() {
        let query = query_from_uri(
            "/v3/admin/ns/service?namespaceId=public&groupName=DEFAULT_GROUP&serviceName=ga-svc&protectThreshold=0.8",
        )
        .await
        .expect("GA query must deserialize");

        let form = merge_form(Some(query), None).expect("query-only form");
        assert_eq!(form, sample("ga-svc", 0.8));
    }

    #[test]
    fn body_only_request_is_still_accepted() {
        // 3.2.4 and earlier sent the same fields in a form body.
        let form = merge_form(None, Some(web::Form(sample("legacy-svc", 0.5))));
        assert_eq!(form, Some(sample("legacy-svc", 0.5)));
    }

    #[actix_web::test]
    async fn query_wins_when_both_sources_are_present() {
        let query = query_from_uri("/v3/admin/ns/service?serviceName=query-svc").await;
        let form = merge_form(query, Some(web::Form(sample("body-svc", 0.5))));
        assert_eq!(
            form,
            Some(SampleForm {
                service_name: "query-svc".to_string(),
                protect_threshold: None,
            })
        );
    }

    #[test]
    fn no_source_returns_none() {
        assert_eq!(merge_form::<SampleForm>(None, None), None);
    }

    #[actix_web::test]
    async fn incomplete_query_falls_back_to_the_body() {
        // A partial query cannot deserialize `ServiceForm` (serviceName is
        // required); Option must yield None so the body can still be used.
        let query = query_from_uri("/v3/admin/ns/service?namespaceId=public").await;
        assert!(query.is_none());
        let form = merge_form(query, Some(web::Form(sample("body-svc", 0.5))));
        assert_eq!(form, Some(sample("body-svc", 0.5)));
    }
}
