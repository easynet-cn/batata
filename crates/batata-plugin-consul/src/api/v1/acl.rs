//! Consul ACL API handlers with scope-relative route macros.
//!
//! These delegate to the original handlers in `crate::acl`.

use actix_web::{HttpRequest, HttpResponse, Scope, delete, get, post, put, web};

use crate::acl::{
    AclService, AuthMethodRequest, BindingRuleRequest, CloneTokenRequest, CreatePolicyRequest,
    CreateTokenRequest, LoginRequest, PolicyUpdateRequest, RoleRequest,
    TemplatedPolicyPreviewRequest, TokenUpdateRequest,
};
use crate::index_provider::ConsulIndexProvider;
use crate::model::ConsulDatacenterConfig;

// ============================================================================
// Bootstrap and auth endpoints
// ============================================================================

#[put("/bootstrap")]
async fn acl_bootstrap(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::acl_bootstrap(acl_service, index_provider).await
}

#[post("/login")]
async fn acl_login(
    acl_service: web::Data<AclService>,
    body: web::Json<LoginRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::acl_login(acl_service, body, index_provider).await
}

#[post("/logout")]
async fn acl_logout(
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::acl_logout(acl_service, req, index_provider).await
}

// ============================================================================
// Replication status
// ============================================================================

#[get("/replication")]
async fn acl_replication(
    dc_config: web::Data<ConsulDatacenterConfig>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::acl_replication(dc_config, index_provider).await
}

// ============================================================================
// Authorization
// ============================================================================

#[post("/authorize")]
async fn acl_authorize(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    body: web::Json<Vec<crate::acl::AclAuthorizationCheck>>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::acl_authorize(req, acl_service, body, index_provider).await
}

// ============================================================================
// Token management
// ============================================================================

#[get("/tokens")]
async fn list_tokens(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::list_tokens(acl_service, index_provider).await
}

#[get("/token/self")]
async fn get_token_self(
    acl_service: web::Data<AclService>,
    req: HttpRequest,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_token_self(acl_service, req, index_provider).await
}

#[put("/token")]
async fn create_token(
    acl_service: web::Data<AclService>,
    body: web::Json<CreateTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::create_token(acl_service, body, index_provider).await
}

#[put("/token/{accessor_id}/clone")]
async fn clone_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<CloneTokenRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::clone_token(acl_service, path, body, index_provider).await
}

#[get("/token/{accessor_id}")]
async fn get_token(
    req: HttpRequest,
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_token(req, acl_service, path, index_provider).await
}

#[put("/token/{accessor_id}")]
async fn update_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<TokenUpdateRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::update_token(acl_service, path, body, index_provider).await
}

#[delete("/token/{accessor_id}")]
async fn delete_token(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::delete_token(acl_service, path, index_provider).await
}

// ============================================================================
// Policy management
// ============================================================================

#[get("/policies")]
async fn list_policies(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::list_policies(acl_service, index_provider).await
}

#[put("/policy")]
async fn create_policy(
    acl_service: web::Data<AclService>,
    body: web::Json<CreatePolicyRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::create_policy(acl_service, body, index_provider).await
}

#[get("/policy/name/{name}")]
async fn get_policy_by_name(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_policy_by_name(acl_service, path, index_provider).await
}

#[get("/policy/{id}")]
async fn get_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_policy(acl_service, path, index_provider).await
}

#[put("/policy/{id}")]
async fn update_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<PolicyUpdateRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::update_policy(acl_service, path, body, index_provider).await
}

#[delete("/policy/{id}")]
async fn delete_policy(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::delete_policy(acl_service, path, index_provider).await
}

// ============================================================================
// Role management
// ============================================================================

#[get("/roles")]
async fn list_roles(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::list_roles(acl_service, index_provider).await
}

#[put("/role")]
async fn create_role(
    acl_service: web::Data<AclService>,
    body: web::Json<RoleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::create_role(acl_service, body, index_provider).await
}

#[get("/role/name/{name}")]
async fn get_role_by_name(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_role_by_name(acl_service, path, index_provider).await
}

#[get("/role/{id}")]
async fn get_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_role(acl_service, path, index_provider).await
}

#[put("/role/{id}")]
async fn update_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<RoleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::update_role(acl_service, path, body, index_provider).await
}

#[delete("/role/{id}")]
async fn delete_role(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::delete_role(acl_service, path, index_provider).await
}

// ============================================================================
// Binding Rule management
// ============================================================================

#[get("/binding-rules")]
async fn list_binding_rules(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::list_binding_rules(acl_service, index_provider).await
}

#[put("/binding-rule")]
async fn create_binding_rule(
    acl_service: web::Data<AclService>,
    body: web::Json<BindingRuleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::create_binding_rule(acl_service, body, index_provider).await
}

#[get("/binding-rule/{id}")]
async fn get_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_binding_rule(acl_service, path, index_provider).await
}

#[put("/binding-rule/{id}")]
async fn update_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<BindingRuleRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::update_binding_rule(acl_service, path, body, index_provider).await
}

#[delete("/binding-rule/{id}")]
async fn delete_binding_rule(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::delete_binding_rule(acl_service, path, index_provider).await
}

// ============================================================================
// Auth Method management
// ============================================================================

#[get("/auth-methods")]
async fn list_auth_methods(
    acl_service: web::Data<AclService>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::list_auth_methods(acl_service, index_provider).await
}

#[put("/auth-method")]
async fn create_auth_method(
    acl_service: web::Data<AclService>,
    body: web::Json<AuthMethodRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::create_auth_method(acl_service, body, index_provider).await
}

#[get("/auth-method/{name}")]
async fn get_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_auth_method(acl_service, path, index_provider).await
}

#[put("/auth-method/{name}")]
async fn update_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    body: web::Json<AuthMethodRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::update_auth_method(acl_service, path, body, index_provider).await
}

#[delete("/auth-method/{name}")]
async fn delete_auth_method(
    acl_service: web::Data<AclService>,
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::delete_auth_method(acl_service, path, index_provider).await
}

// ============================================================================
// Templated policies
// ============================================================================

#[get("/templated-policies")]
async fn list_templated_policies(index_provider: web::Data<ConsulIndexProvider>) -> HttpResponse {
    crate::acl::list_templated_policies(index_provider).await
}

#[get("/templated-policy/name/{name}")]
async fn get_templated_policy(
    path: web::Path<String>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::get_templated_policy(path, index_provider).await
}

#[post("/templated-policy/preview/{name}")]
async fn preview_templated_policy(
    path: web::Path<String>,
    body: web::Json<TemplatedPolicyPreviewRequest>,
    index_provider: web::Data<ConsulIndexProvider>,
) -> HttpResponse {
    crate::acl::preview_templated_policy(path, body, index_provider).await
}

// ============================================================================
// Scope builder
// ============================================================================

pub fn routes() -> Scope {
    web::scope("/acl")
        // Bootstrap and auth
        .service(acl_bootstrap)
        .service(acl_login)
        .service(acl_logout)
        // Replication
        .service(acl_replication)
        // Authorization
        .service(acl_authorize)
        // Token management
        .service(list_tokens)
        .service(get_token_self)
        .service(create_token)
        .service(clone_token)
        .service(get_token)
        .service(update_token)
        .service(delete_token)
        // Policy management
        .service(list_policies)
        .service(create_policy)
        .service(get_policy_by_name)
        .service(get_policy)
        .service(update_policy)
        .service(delete_policy)
        // Role management
        .service(list_roles)
        .service(create_role)
        .service(get_role_by_name)
        .service(get_role)
        .service(update_role)
        .service(delete_role)
        // Binding Rule management
        .service(list_binding_rules)
        .service(create_binding_rule)
        .service(get_binding_rule)
        .service(update_binding_rule)
        .service(delete_binding_rule)
        // Auth Method management
        .service(list_auth_methods)
        .service(create_auth_method)
        .service(get_auth_method)
        .service(update_auth_method)
        .service(delete_auth_method)
        // Templated policies
        .service(list_templated_policies)
        .service(get_templated_policy)
        .service(preview_templated_policy)
}
