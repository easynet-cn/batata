// Naming module gRPC handlers
// Implements handlers for service discovery requests

use std::sync::Arc;

use batata_core::model::Connection;
use tonic::Status;

use crate::{
    api::{
        grpc::Payload,
        naming::model::{
            BatchInstanceRequest, BatchInstanceResponse, DE_REGISTER_INSTANCE, InstanceRequest,
            InstanceResponse, NamingFuzzyWatchChangeNotifyRequest,
            NamingFuzzyWatchChangeNotifyResponse, NamingFuzzyWatchRequest,
            NamingFuzzyWatchResponse, NamingFuzzyWatchSyncRequest, NamingFuzzyWatchSyncResponse,
            NotifySubscriberRequest, NotifySubscriberResponse, PersistentInstanceRequest,
            QueryServiceResponse, REGISTER_INSTANCE, ServiceListRequest, ServiceListResponse,
            ServiceQueryRequest, SubscribeServiceRequest, SubscribeServiceResponse,
        },
        remote::model::{RequestTrait, ResponseCode, ResponseTrait},
    },
    service::{naming::NamingService, rpc::{AuthRequirement, PayloadHandler}},
};

// Handler for InstanceRequest - registers or deregisters a service instance
#[derive(Clone)]
pub struct InstanceRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for InstanceRequestHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = InstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let instance = request.instance;
        let req_type = &request.r#type;

        let result = if req_type == REGISTER_INSTANCE {
            self.naming_service
                .register_instance(namespace, group_name, service_name, instance)
        } else if req_type == DE_REGISTER_INSTANCE {
            self.naming_service
                .deregister_instance(namespace, group_name, service_name, &instance)
        } else {
            false
        };

        let mut response = InstanceResponse::new();
        response.response.request_id = request_id;
        response.r#type = req_type.clone();

        if !result {
            response.response.result_code = ResponseCode::Fail.code();
            response.response.error_code = ResponseCode::Fail.code();
            response.response.success = false;
            response.response.message = "Operation failed".to_string();
        }

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "InstanceRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Authenticated
    }
}

// Handler for BatchInstanceRequest - batch registers or deregisters instances
#[derive(Clone)]
pub struct BatchInstanceRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for BatchInstanceRequestHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = BatchInstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let instances = request.instances;
        let req_type = &request.r#type;

        let result = if req_type == REGISTER_INSTANCE {
            self.naming_service.batch_register_instances(
                namespace,
                group_name,
                service_name,
                instances,
            )
        } else if req_type == DE_REGISTER_INSTANCE {
            self.naming_service.batch_deregister_instances(
                namespace,
                group_name,
                service_name,
                &instances,
            )
        } else {
            false
        };

        let mut response = BatchInstanceResponse::new();
        response.response.request_id = request_id;
        response.r#type = req_type.clone();

        if !result {
            response.response.result_code = ResponseCode::Fail.code();
            response.response.error_code = ResponseCode::Fail.code();
            response.response.success = false;
            response.response.message = "Operation failed".to_string();
        }

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "BatchInstanceRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Authenticated
    }
}

// Handler for ServiceListRequest - lists services in a namespace
#[derive(Clone)]
pub struct ServiceListRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for ServiceListRequestHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ServiceListRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let page_no = request.page_no.max(1);
        let page_size = request.page_size.max(10);

        let (count, service_names) = self
            .naming_service
            .list_services(namespace, group_name, page_no, page_size);

        let mut response = ServiceListResponse::new();
        response.response.request_id = request_id;
        response.count = count;
        response.service_names = service_names;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ServiceListRequest"
    }
}

// Handler for ServiceQueryRequest - queries service details and instances
#[derive(Clone)]
pub struct ServiceQueryRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for ServiceQueryRequestHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = ServiceQueryRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let cluster = &request.cluster;
        let healthy_only = request.healthy_only;

        let service_info = self.naming_service.get_service(
            namespace,
            group_name,
            service_name,
            cluster,
            healthy_only,
        );

        let mut response = QueryServiceResponse::new();
        response.response.request_id = request_id;
        response.service_info = service_info;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "ServiceQueryRequest"
    }
}

// Handler for SubscribeServiceRequest - subscribes to service changes
#[derive(Clone)]
pub struct SubscribeServiceRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for SubscribeServiceRequestHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = SubscribeServiceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let clusters = &request.clusters;
        let subscribe = request.subscribe;

        let connection_id = &connection.meta_info.connection_id;

        if subscribe {
            self.naming_service
                .subscribe(connection_id, namespace, group_name, service_name);
        } else {
            self.naming_service
                .unsubscribe(connection_id, namespace, group_name, service_name);
        }

        // Return current service info
        let service_info =
            self.naming_service
                .get_service(namespace, group_name, service_name, clusters, false);

        let mut response = SubscribeServiceResponse::new();
        response.response.request_id = request_id;
        response.service_info = service_info;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "SubscribeServiceRequest"
    }
}

// Handler for PersistentInstanceRequest - handles persistent (non-ephemeral) instances
#[derive(Clone)]
pub struct PersistentInstanceRequestHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for PersistentInstanceRequestHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = PersistentInstanceRequest::from(payload);
        let request_id = request.request_id();

        let namespace = &request.naming_request.namespace;
        let group_name = &request.naming_request.group_name;
        let service_name = &request.naming_request.service_name;
        let mut instance = request.instance;
        let req_type = &request.r#type;

        // Mark instance as persistent (non-ephemeral)
        instance.ephemeral = false;

        let result = if req_type == REGISTER_INSTANCE {
            self.naming_service
                .register_instance(namespace, group_name, service_name, instance)
        } else if req_type == DE_REGISTER_INSTANCE {
            self.naming_service
                .deregister_instance(namespace, group_name, service_name, &instance)
        } else {
            false
        };

        let mut response = InstanceResponse::new();
        response.response.request_id = request_id;
        response.r#type = req_type.clone();

        if !result {
            response.response.result_code = ResponseCode::Fail.code();
            response.response.error_code = ResponseCode::Fail.code();
            response.response.success = false;
            response.response.message = "Operation failed".to_string();
        }

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "PersistentInstanceRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Authenticated
    }
}

// Handler for NotifySubscriberRequest - notifies subscribers of service changes
// Note: This is typically a server-push request to clients
#[derive(Clone)]
pub struct NotifySubscriberHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for NotifySubscriberHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NotifySubscriberRequest::from(payload);
        let request_id = request.request_id();

        // This handler acknowledges the notification
        // In client context, would update local service cache
        let mut response = NotifySubscriberResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NotifySubscriberRequest"
    }
}

// Handler for NamingFuzzyWatchRequest - handles fuzzy pattern watch for services
#[derive(Clone)]
pub struct NamingFuzzyWatchHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &connection.meta_info.connection_id;
        let namespace = &request.namespace;
        let group_pattern = &request.group_name_pattern;
        let service_pattern = &request.service_name_pattern;
        let watch_type = &request.watch_type;

        // Register the fuzzy watch pattern for this connection
        self.naming_service.register_fuzzy_watch(
            connection_id,
            namespace,
            group_pattern,
            service_pattern,
            watch_type,
        );

        // Return matching service keys if this is an initializing request
        let mut response = NamingFuzzyWatchResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NamingFuzzyWatchRequest"
    }
}

// Handler for NamingFuzzyWatchChangeNotifyRequest - notifies fuzzy watch changes
#[derive(Clone)]
pub struct NamingFuzzyWatchChangeNotifyHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchChangeNotifyHandler {
    async fn handle(&self, _connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchChangeNotifyRequest::from(payload);
        let request_id = request.request_id();

        let mut response = NamingFuzzyWatchChangeNotifyResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NamingFuzzyWatchChangeNotifyRequest"
    }
}

// Handler for NamingFuzzyWatchSyncRequest - syncs fuzzy watch state
#[derive(Clone)]
pub struct NamingFuzzyWatchSyncHandler {
    pub naming_service: Arc<NamingService>,
}

#[tonic::async_trait]
impl PayloadHandler for NamingFuzzyWatchSyncHandler {
    async fn handle(&self, connection: &Connection, payload: &Payload) -> Result<Payload, Status> {
        let request = NamingFuzzyWatchSyncRequest::from(payload);
        let request_id = request.request_id();

        let connection_id = &connection.meta_info.connection_id;
        let namespace = &request.pattern_namespace;
        let group_pattern = &request.pattern_group_name;
        let service_pattern = &request.pattern_service_name;
        let sync_type = &request.sync_type;

        // For initial sync, get all matching services
        if sync_type == "all" || request.current_batch == 0 {
            // Get all services matching the pattern
            let _matched_services = self.naming_service.get_services_by_pattern(
                namespace,
                group_pattern,
                service_pattern,
            );
            // The matched services would be sent to client in batches
            // This handler acknowledges the sync request
        }

        // Also register the pattern if not already registered
        self.naming_service.register_fuzzy_watch(
            connection_id,
            namespace,
            group_pattern,
            service_pattern,
            sync_type,
        );

        let mut response = NamingFuzzyWatchSyncResponse::new();
        response.response.request_id = request_id;

        Ok(response.build_payload())
    }

    fn can_handle(&self) -> &'static str {
        "NamingFuzzyWatchSyncRequest"
    }

    fn auth_requirement(&self) -> AuthRequirement {
        AuthRequirement::Internal
    }
}
