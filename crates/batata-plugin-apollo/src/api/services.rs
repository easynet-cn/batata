//! Apollo service discovery handlers
//!
//! HTTP handlers for Apollo service discovery endpoints.

use actix_web::HttpResponse;

/// Get config service address
///
/// `GET /services/config`
pub async fn get_config_service() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!([{
        "appName": "APOLLO-CONFIGSERVICE",
        "instanceId": "localhost:apollo-configservice:8080",
        "homepageUrl": "http://localhost:8080/"
    }]))
}

/// Get admin service address
///
/// `GET /services/admin`
pub async fn get_admin_service() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!([{
        "appName": "APOLLO-ADMINSERVICE",
        "instanceId": "localhost:apollo-adminservice:8080",
        "homepageUrl": "http://localhost:8080/"
    }]))
}

#[cfg(test)]
mod tests {
    use actix_web::body::MessageBody;
    use actix_web::http::StatusCode;

    use super::*;

    #[actix_web::test]
    async fn test_get_config_service() {
        let response = get_config_service().await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().try_into_bytes().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_array());
        assert_eq!(json[0]["appName"], "APOLLO-CONFIGSERVICE");
    }

    #[actix_web::test]
    async fn test_get_admin_service() {
        let response = get_admin_service().await;
        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().try_into_bytes().unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.is_array());
        assert_eq!(json[0]["appName"], "APOLLO-ADMINSERVICE");
    }
}
