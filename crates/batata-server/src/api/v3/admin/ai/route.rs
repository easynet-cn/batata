use actix_web::{Scope, web};

use super::{a2a, mcp};

pub fn routes() -> Scope {
    web::scope("/ai")
        .service(mcp::routes())
        .service(a2a::routes())
}
