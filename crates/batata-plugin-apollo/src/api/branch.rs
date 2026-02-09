//! Apollo branch/gray release API handlers
//!
//! HTTP handlers for branch management and gray release operations.

use std::sync::Arc;

use actix_web::{HttpResponse, web};
use serde::Deserialize;

use crate::model::GrayRule;
use crate::service::ApolloBranchService;

/// Path parameters for branch operations
#[derive(Debug, Deserialize)]
pub struct BranchPathParams {
    pub env: String,
    pub app_id: String,
    pub cluster: String,
    pub namespace: String,
    pub branch: String,
}

/// Path parameters for namespace-level branch operations
#[derive(Debug, Deserialize)]
pub struct BranchListPathParams {
    pub env: String,
    pub app_id: String,
    pub cluster: String,
    pub namespace: String,
}

/// Request to create a branch
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateBranchRequest {
    pub branch_name: String,
    pub operator: String,
}

/// Request to update gray release rules
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateGrayRulesRequest {
    pub rules: Vec<GrayRule>,
}

/// Request to publish a gray release
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GrayReleaseRequest {
    pub release_title: String,
    pub operator: String,
}

/// Request to merge a branch
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MergeBranchRequest {
    pub operator: String,
}

// ============================================================================
// Branch CRUD
// ============================================================================

/// Get branches for a namespace
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/branches`
pub async fn get_branches(
    service: web::Data<Arc<ApolloBranchService>>,
    path: web::Path<BranchListPathParams>,
) -> HttpResponse {
    match service.get_branches(&path.app_id, &path.cluster).await {
        Ok(branches) => {
            let branch_names: Vec<String> = branches.into_iter().map(|b| b.name).collect();
            HttpResponse::Ok().json(branch_names)
        }
        Err(e) => {
            tracing::error!("Failed to get branches: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

/// Create a branch
///
/// `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/branches`
pub async fn create_branch(
    service: web::Data<Arc<ApolloBranchService>>,
    path: web::Path<BranchListPathParams>,
    body: web::Json<CreateBranchRequest>,
) -> HttpResponse {
    match service
        .create_branch(
            &path.app_id,
            &path.cluster,
            &body.branch_name,
            &body.operator,
        )
        .await
    {
        Ok(name) => HttpResponse::Created().json(serde_json::json!({
            "branchName": name
        })),
        Err(e) => {
            let err_msg = e.to_string();
            tracing::error!("Failed to create branch: {}", err_msg);
            if err_msg.to_lowercase().contains("not found") {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "status": 400,
                    "message": err_msg
                }))
            } else {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": 500,
                    "message": err_msg
                }))
            }
        }
    }
}

/// Delete a branch
///
/// `DELETE /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}`
pub async fn delete_branch(
    service: web::Data<Arc<ApolloBranchService>>,
    path: web::Path<BranchPathParams>,
) -> HttpResponse {
    match service.delete_branch(&path.app_id, &path.branch).await {
        Ok(true) => HttpResponse::Ok().json(serde_json::json!({
            "success": true
        })),
        Ok(false) => HttpResponse::NotFound().json(serde_json::json!({
            "error": "Branch not found"
        })),
        Err(e) => {
            tracing::error!("Failed to delete branch: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

// ============================================================================
// Gray Release Rules
// ============================================================================

/// Get gray release rules for a branch
///
/// `GET /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/rules`
pub async fn get_gray_rules(
    service: web::Data<Arc<ApolloBranchService>>,
    path: web::Path<BranchPathParams>,
) -> HttpResponse {
    match service
        .get_gray_rules(&path.app_id, &path.cluster, &path.namespace)
        .await
    {
        Some(rule) => HttpResponse::Ok().json(rule),
        None => HttpResponse::NotFound().json(serde_json::json!({
            "error": "No gray release rules found"
        })),
    }
}

/// Update gray release rules for a branch
///
/// `PUT /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/rules`
pub async fn update_gray_rules(
    service: web::Data<Arc<ApolloBranchService>>,
    path: web::Path<BranchPathParams>,
    body: web::Json<UpdateGrayRulesRequest>,
) -> HttpResponse {
    match service
        .update_gray_rules(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.branch,
            body.into_inner().rules,
        )
        .await
    {
        Ok(rule) => HttpResponse::Ok().json(rule),
        Err(e) => {
            tracing::error!("Failed to update gray rules: {}", e);
            HttpResponse::InternalServerError().json(serde_json::json!({
                "error": e.to_string()
            }))
        }
    }
}

// ============================================================================
// Gray Release Operations
// ============================================================================

/// Publish a gray release on a branch
///
/// `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/releases`
pub async fn gray_release(
    service: web::Data<Arc<ApolloBranchService>>,
    path: web::Path<BranchPathParams>,
    body: web::Json<GrayReleaseRequest>,
) -> HttpResponse {
    match service
        .gray_release(
            &path.app_id,
            &path.cluster,
            &path.namespace,
            &path.branch,
            &body.release_title,
            &body.operator,
        )
        .await
    {
        Ok(release) => HttpResponse::Ok().json(release),
        Err(e) => {
            let err_msg = e.to_string();
            tracing::error!("Failed to publish gray release: {}", err_msg);
            if err_msg.to_lowercase().contains("not found") {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "status": 400,
                    "message": err_msg
                }))
            } else {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": 500,
                    "message": err_msg
                }))
            }
        }
    }
}

/// Merge a branch into the parent cluster
///
/// `POST /openapi/v1/envs/{env}/apps/{appId}/clusters/{cluster}/namespaces/{namespace}/branches/{branch}/merge`
pub async fn merge_branch(
    service: web::Data<Arc<ApolloBranchService>>,
    path: web::Path<BranchPathParams>,
    body: web::Json<MergeBranchRequest>,
) -> HttpResponse {
    match service
        .merge_branch(
            &path.app_id,
            &path.cluster,
            &path.branch,
            &path.namespace,
            &body.operator,
        )
        .await
    {
        Ok(release) => HttpResponse::Ok().json(release),
        Err(e) => {
            let err_msg = e.to_string();
            tracing::error!("Failed to merge branch: {}", err_msg);
            if err_msg.to_lowercase().contains("not found") {
                HttpResponse::BadRequest().json(serde_json::json!({
                    "status": 400,
                    "message": err_msg
                }))
            } else {
                HttpResponse::InternalServerError().json(serde_json::json!({
                    "status": 500,
                    "message": err_msg
                }))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::GrayRuleType;

    #[test]
    fn test_branch_path_params() {
        let json = r#"{"env":"DEV","app_id":"test","cluster":"default","namespace":"application","branch":"gray-1"}"#;
        let params: BranchPathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.env, "DEV");
        assert_eq!(params.app_id, "test");
        assert_eq!(params.cluster, "default");
        assert_eq!(params.namespace, "application");
        assert_eq!(params.branch, "gray-1");
    }

    #[test]
    fn test_branch_list_path_params() {
        let json = r#"{"env":"PRO","app_id":"app1","cluster":"default","namespace":"app"}"#;
        let params: BranchListPathParams = serde_json::from_str(json).unwrap();
        assert_eq!(params.env, "PRO");
        assert_eq!(params.app_id, "app1");
        assert_eq!(params.namespace, "app");
    }

    #[test]
    fn test_create_branch_request() {
        let json = r#"{"branchName":"gray-v2","operator":"admin"}"#;
        let req: CreateBranchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.branch_name, "gray-v2");
        assert_eq!(req.operator, "admin");
    }

    #[test]
    fn test_update_gray_rules_request() {
        let json = r#"{
            "rules": [
                {"ruleType": "ip", "value": "10.0.0.1,10.0.0.2"},
                {"ruleType": "label", "value": "env=staging"}
            ]
        }"#;
        let req: UpdateGrayRulesRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.rules.len(), 2);
        assert_eq!(req.rules[0].rule_type, GrayRuleType::Ip);
        assert_eq!(req.rules[1].rule_type, GrayRuleType::Label);
    }

    #[test]
    fn test_update_gray_rules_request_empty() {
        let json = r#"{"rules": []}"#;
        let req: UpdateGrayRulesRequest = serde_json::from_str(json).unwrap();
        assert!(req.rules.is_empty());
    }

    #[test]
    fn test_gray_release_request() {
        let json = r#"{"releaseTitle":"Gray v1.0","operator":"release-admin"}"#;
        let req: GrayReleaseRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.release_title, "Gray v1.0");
        assert_eq!(req.operator, "release-admin");
    }

    #[test]
    fn test_merge_branch_request() {
        let json = r#"{"operator":"admin"}"#;
        let req: MergeBranchRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.operator, "admin");
    }
}
