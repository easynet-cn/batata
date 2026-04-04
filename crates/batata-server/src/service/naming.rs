// Naming service layer for service discovery operations
// Uses NacosNamingServiceImpl from batata-naming-nacos as the primary implementation

pub use batata_naming_nacos::service::NacosNamingServiceImpl;

/// Type alias for backward compatibility with tests and benchmarks
pub type NamingService = NacosNamingServiceImpl;
