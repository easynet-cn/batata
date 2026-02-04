//! Apollo notification endpoint handler
//!
//! GET `/notifications/v2` - Long polling for config updates

use actix_web::{HttpResponse, web};
use std::sync::Arc;

use crate::model::NotificationQueryParams;
use crate::service::ApolloNotificationService;

/// Get notifications (long polling)
///
/// This endpoint implements HTTP long polling. It will:
/// 1. Check if any requested configs have changed since the client's last notification ID
/// 2. If changed, return immediately with the new notification
/// 3. If not changed, hold the connection for up to 60 seconds waiting for changes
/// 4. If timeout occurs with no changes, return empty array
///
/// ## Request
/// - `appId`: Application ID
/// - `cluster`: Cluster name
/// - `notifications`: JSON array of `[{"namespaceName":"application","notificationId":-1}]`
///
/// ## Response
/// - 200 OK: Array of `ApolloConfigNotification` for changed configs
/// - Empty array `[]` if no changes (timeout)
pub async fn get_notifications(
    notification_service: web::Data<Arc<ApolloNotificationService>>,
    query: web::Query<NotificationQueryParams>,
) -> HttpResponse {
    // Parse notifications from JSON
    let notifications = match query.parse_notifications() {
        Ok(n) => n,
        Err(e) => {
            tracing::warn!("Failed to parse notifications: {}", e);
            return HttpResponse::BadRequest().json(serde_json::json!({
                "status": 400,
                "message": format!("Invalid notifications format: {}", e)
            }));
        }
    };

    if notifications.is_empty() {
        return HttpResponse::Ok().json(Vec::<()>::new());
    }

    // Long poll for changes
    let results = notification_service
        .poll_notifications(
            &query.app_id,
            &query.cluster,
            query.data_center.as_deref(),
            notifications,
        )
        .await;

    HttpResponse::Ok().json(results)
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_notification_query_parse() {
        let json = r#"[{"namespaceName":"application","notificationId":-1}]"#;
        let notifications: Vec<crate::model::NotificationRequest> =
            serde_json::from_str(json).unwrap();
        assert_eq!(notifications.len(), 1);
        assert_eq!(notifications[0].namespace_name, "application");
        assert_eq!(notifications[0].notification_id, -1);
    }
}
