pub mod v1 {
    mod auth;
    pub mod route;
}
pub mod v3 {
    mod auth;
    mod permission;
    mod role;
    pub mod route;
    mod user;
}
pub mod model;
pub mod service {
    pub mod auth;
    pub mod permission;
    pub mod role;
    pub mod user;
}
