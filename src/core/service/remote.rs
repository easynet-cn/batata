use tonic::{Request, Status};

use crate::core::model::Connection;

pub fn context_interceptor<T>(mut request: Request<T>) -> Result<Request<T>, Status> {
    let mut connection = Connection::default();

    let remote_addr = request.remote_addr().unwrap();

    connection.meta_info.connection_id = format!(
        "{}_{}_{}",
        chrono::Utc::now().timestamp_millis(),
        remote_addr.ip().to_string(),
        remote_addr.port()
    );

    request.extensions_mut().insert(connection);

    Ok(request)
}
