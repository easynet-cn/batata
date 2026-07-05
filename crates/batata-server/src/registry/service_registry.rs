//! Service registry - type-safe service storage and retrieval
//!
//! This module provides a centralized registry for storing and
//! retrieving services by TypeId.

use std::any::{Any, TypeId};
use std::collections::HashMap;
use std::sync::Arc;

/// Service registry - stores services by TypeId for type-safe retrieval
///
/// Services are stored as `Arc<dyn Any + Send + Sync>` and can be
/// retrieved by their concrete type using the `get<T>` method.
///
/// # Example
///
/// ```ignore
/// let registry = ServiceRegistry::new();
/// registry.register(my_service.clone());
///
/// let retrieved: Arc<MyService> = registry.get::<MyService>().unwrap();
/// ```
#[derive(Clone, Default)]
pub struct ServiceRegistry {
    services: HashMap<TypeId, Arc<dyn Any + Send + Sync>>,
}

impl ServiceRegistry {
    /// Create a new empty service registry
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }
    
    /// Register a service
    ///
    /// If a service of the same type is already registered, it will be replaced.
    pub fn register<T: 'static + Send + Sync>(&mut self, service: Arc<T>) {
        self.services.insert(TypeId::of::<T>(), service);
    }
    
    /// Register a service as a trait object
    pub fn register_boxed(&mut self, type_id: TypeId, service: Arc<dyn Any + Send + Sync>) {
        self.services.insert(type_id, service);
    }
    
    /// Get a service by type
    ///
    /// Returns the service if found and castable to the requested type.
    pub fn get<T: 'static + Send + Sync>(&self) -> Option<Arc<T>> {
        self.services
            .get(&TypeId::of::<T>())
            .and_then(|s| s.clone().downcast::<T>().ok())
    }
    
    /// Check if a service of the given type is registered
    pub fn contains<T: 'static + Send + Sync>(&self) -> bool {
        self.services.contains_key(&TypeId::of::<T>())
    }
    
    /// Remove a service by type
    ///
    /// Returns the removed service if it existed.
    pub fn remove<T: 'static + Send + Sync>(&mut self) -> Option<Arc<T>> {
        self.services
            .remove(&TypeId::of::<T>())
            .and_then(|s| s.downcast::<T>().ok())
    }
    
    /// Get the number of registered services
    pub fn len(&self) -> usize {
        self.services.len()
    }
    
    /// Check if the registry is empty
    pub fn is_empty(&self) -> bool {
        self.services.is_empty()
    }
    
    /// Iterate over all service type IDs
    pub fn type_ids(&self) -> impl Iterator<Item = TypeId> + '_ {
        self.services.keys().copied()
    }
}

impl std::fmt::Debug for ServiceRegistry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ServiceRegistry")
            .field("service_count", &self.services.len())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    struct TestService {
        value: i32,
    }
    
    struct AnotherService {
        name: String,
    }
    
    #[test]
    fn test_register_and_get() {
        let mut registry = ServiceRegistry::new();
        
        let service = Arc::new(TestService { value: 42 });
        registry.register(service);
        
        let retrieved = registry.get::<TestService>();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().value, 42);
    }
    
    #[test]
    fn test_get_missing() {
        let registry = ServiceRegistry::new();
        let result = registry.get::<TestService>();
        assert!(result.is_none());
    }
    
    #[test]
    fn test_replace_service() {
        let mut registry = ServiceRegistry::new();
        
        registry.register(Arc::new(TestService { value: 1 }));
        registry.register(Arc::new(TestService { value: 2 }));
        
        let retrieved = registry.get::<TestService>().unwrap();
        assert_eq!(retrieved.value, 2);
    }
    
    #[test]
    fn test_contains() {
        let mut registry = ServiceRegistry::new();
        
        assert!(!registry.contains::<TestService>());
        
        registry.register(Arc::new(TestService { value: 1 }));
        
        assert!(registry.contains::<TestService>());
    }
    
    #[test]
    fn test_remove() {
        let mut registry = ServiceRegistry::new();
        
        registry.register(Arc::new(TestService { value: 1 }));
        let removed = registry.remove::<TestService>();
        
        assert!(removed.is_some());
        assert!(registry.get::<TestService>().is_none());
    }
    
    #[test]
    fn test_len_and_is_empty() {
        let mut registry = ServiceRegistry::new();
        
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
        
        registry.register(Arc::new(TestService { value: 1 }));
        assert!(!registry.is_empty());
        assert_eq!(registry.len(), 1);
        
        registry.register(Arc::new(AnotherService { name: "test".to_string() }));
        assert_eq!(registry.len(), 2);
    }
    
    #[test]
    fn test_multiple_services() {
        let mut registry = ServiceRegistry::new();
        
        registry.register(Arc::new(TestService { value: 42 }));
        registry.register(Arc::new(AnotherService { name: "hello".to_string() }));
        
        let test = registry.get::<TestService>();
        let another = registry.get::<AnotherService>();
        
        assert!(test.is_some());
        assert!(another.is_some());
        assert_eq!(test.unwrap().value, 42);
        assert_eq!(another.unwrap().name, "hello");
    }
    
    #[test]
    fn test_type_ids() {
        let mut registry = ServiceRegistry::new();
        
        registry.register(Arc::new(TestService { value: 1 }));
        registry.register(Arc::new(AnotherService { name: "test".to_string() }));
        
        let ids: Vec<_> = registry.type_ids().collect();
        assert_eq!(ids.len(), 2);
    }
}
