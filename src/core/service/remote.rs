use tonic::{Request, Status};

pub fn context_interceptor(request: Request<()>) -> Result<Request<()>, Status> {
    Ok(request)
}
