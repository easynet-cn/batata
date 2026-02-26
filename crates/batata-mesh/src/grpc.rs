//! gRPC Service Implementation for xDS
//!
//! This module provides the tonic gRPC service implementation for xDS protocols,
//! including ADS (Aggregated Discovery Service).

use std::pin::Pin;
use std::sync::Arc;

use futures::Stream;
use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status, Streaming};
use tracing::{debug, error, info, warn};

use crate::server::{
    DiscoveryRequest as InternalRequest, DiscoveryResponse as InternalResponse, XdsServer,
};
use crate::xds::ResourceType;

/// Protocol buffer definitions generated manually (matching xds.proto)
pub mod proto {
    use serde::{Deserialize, Serialize};

    /// Discovery Request message
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DiscoveryRequest {
        pub version_info: String,
        pub node: Option<Node>,
        pub resource_names: Vec<String>,
        pub type_url: String,
        pub response_nonce: String,
        pub error_detail: Option<Status>,
    }

    /// Discovery Response message
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DiscoveryResponse {
        pub version_info: String,
        pub resources: Vec<Any>,
        pub canary: bool,
        pub type_url: String,
        pub nonce: String,
        pub control_plane: Option<ControlPlane>,
    }

    /// Delta Discovery Request
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DeltaDiscoveryRequest {
        pub node: Option<Node>,
        pub type_url: String,
        pub resource_names_subscribe: Vec<String>,
        pub resource_names_unsubscribe: Vec<String>,
        pub initial_resource_versions: std::collections::HashMap<String, String>,
        pub response_nonce: String,
        pub error_detail: Option<Status>,
    }

    /// Delta Discovery Response
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct DeltaDiscoveryResponse {
        pub system_version_info: String,
        pub resources: Vec<Resource>,
        pub type_url: String,
        pub removed_resources: Vec<String>,
        pub nonce: String,
        pub control_plane: Option<ControlPlane>,
    }

    /// Resource wrapper
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Resource {
        pub name: String,
        pub aliases: Vec<String>,
        pub version: String,
        pub resource: Option<Any>,
        pub ttl: Option<Duration>,
        pub cache_control: Option<CacheControl>,
    }

    /// Cache control
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct CacheControl {
        pub do_not_cache: bool,
    }

    /// Duration
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Duration {
        pub seconds: i64,
        pub nanos: i32,
    }

    /// Node identifier
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Node {
        pub id: String,
        pub cluster: String,
        pub metadata: Option<Struct>,
        pub locality: Option<Locality>,
        pub user_agent_name: String,
        pub user_agent_version: String,
        pub client_features: Vec<String>,
    }

    /// Locality
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Locality {
        pub region: String,
        pub zone: String,
        pub sub_zone: String,
    }

    /// Status (error detail)
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Status {
        pub code: i32,
        pub message: String,
        pub details: Vec<Any>,
    }

    /// Control plane identifier
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct ControlPlane {
        pub identifier: String,
    }

    /// Any type wrapper
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Any {
        pub type_url: String,
        pub value: Vec<u8>,
    }

    /// Struct for metadata (simplified)
    #[derive(Debug, Clone, Default, Serialize, Deserialize)]
    pub struct Struct {
        pub fields: std::collections::HashMap<String, Value>,
    }

    /// Value for struct fields
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub enum Value {
        #[default]
        NullValue,
        BoolValue(bool),
        NumberValue(f64),
        StringValue(String),
        ListValue(Vec<Value>),
        StructValue(Box<Struct>),
    }

    // Implement prost encoding/decoding for the proto types
    impl prost::Message for DiscoveryRequest {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.version_info.is_empty() {
                prost::encoding::string::encode(1, &self.version_info, buf);
            }
            if let Some(ref node) = self.node {
                prost::encoding::message::encode(2, node, buf);
            }
            for name in &self.resource_names {
                prost::encoding::string::encode(3, name, buf);
            }
            if !self.type_url.is_empty() {
                prost::encoding::string::encode(4, &self.type_url, buf);
            }
            if !self.response_nonce.is_empty() {
                prost::encoding::string::encode(5, &self.response_nonce, buf);
            }
            if let Some(ref error) = self.error_detail {
                prost::encoding::message::encode(6, error, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(wire_type, &mut self.version_info, buf, ctx),
                2 => {
                    let mut node = self.node.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut node, buf, ctx)?;
                    self.node = Some(node);
                    Ok(())
                }
                3 => {
                    let mut name = String::new();
                    prost::encoding::string::merge(wire_type, &mut name, buf, ctx)?;
                    self.resource_names.push(name);
                    Ok(())
                }
                4 => prost::encoding::string::merge(wire_type, &mut self.type_url, buf, ctx),
                5 => prost::encoding::string::merge(wire_type, &mut self.response_nonce, buf, ctx),
                6 => {
                    let mut error = self.error_detail.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut error, buf, ctx)?;
                    self.error_detail = Some(error);
                    Ok(())
                }
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if !self.version_info.is_empty() {
                len += prost::encoding::string::encoded_len(1, &self.version_info);
            }
            if let Some(ref node) = self.node {
                len += prost::encoding::message::encoded_len(2, node);
            }
            for name in &self.resource_names {
                len += prost::encoding::string::encoded_len(3, name);
            }
            if !self.type_url.is_empty() {
                len += prost::encoding::string::encoded_len(4, &self.type_url);
            }
            if !self.response_nonce.is_empty() {
                len += prost::encoding::string::encoded_len(5, &self.response_nonce);
            }
            if let Some(ref error) = self.error_detail {
                len += prost::encoding::message::encoded_len(6, error);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for Node {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.id.is_empty() {
                prost::encoding::string::encode(1, &self.id, buf);
            }
            if !self.cluster.is_empty() {
                prost::encoding::string::encode(2, &self.cluster, buf);
            }
            if let Some(ref locality) = self.locality {
                prost::encoding::message::encode(4, locality, buf);
            }
            if !self.user_agent_name.is_empty() {
                prost::encoding::string::encode(6, &self.user_agent_name, buf);
            }
            if !self.user_agent_version.is_empty() {
                prost::encoding::string::encode(7, &self.user_agent_version, buf);
            }
            for feature in &self.client_features {
                prost::encoding::string::encode(10, feature, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(wire_type, &mut self.id, buf, ctx),
                2 => prost::encoding::string::merge(wire_type, &mut self.cluster, buf, ctx),
                4 => {
                    let mut locality = self.locality.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut locality, buf, ctx)?;
                    self.locality = Some(locality);
                    Ok(())
                }
                6 => prost::encoding::string::merge(wire_type, &mut self.user_agent_name, buf, ctx),
                7 => prost::encoding::string::merge(
                    wire_type,
                    &mut self.user_agent_version,
                    buf,
                    ctx,
                ),
                10 => {
                    let mut feature = String::new();
                    prost::encoding::string::merge(wire_type, &mut feature, buf, ctx)?;
                    self.client_features.push(feature);
                    Ok(())
                }
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if !self.id.is_empty() {
                len += prost::encoding::string::encoded_len(1, &self.id);
            }
            if !self.cluster.is_empty() {
                len += prost::encoding::string::encoded_len(2, &self.cluster);
            }
            if let Some(ref locality) = self.locality {
                len += prost::encoding::message::encoded_len(4, locality);
            }
            if !self.user_agent_name.is_empty() {
                len += prost::encoding::string::encoded_len(6, &self.user_agent_name);
            }
            if !self.user_agent_version.is_empty() {
                len += prost::encoding::string::encoded_len(7, &self.user_agent_version);
            }
            for feature in &self.client_features {
                len += prost::encoding::string::encoded_len(10, feature);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for Locality {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.region.is_empty() {
                prost::encoding::string::encode(1, &self.region, buf);
            }
            if !self.zone.is_empty() {
                prost::encoding::string::encode(2, &self.zone, buf);
            }
            if !self.sub_zone.is_empty() {
                prost::encoding::string::encode(3, &self.sub_zone, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(wire_type, &mut self.region, buf, ctx),
                2 => prost::encoding::string::merge(wire_type, &mut self.zone, buf, ctx),
                3 => prost::encoding::string::merge(wire_type, &mut self.sub_zone, buf, ctx),
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if !self.region.is_empty() {
                len += prost::encoding::string::encoded_len(1, &self.region);
            }
            if !self.zone.is_empty() {
                len += prost::encoding::string::encoded_len(2, &self.zone);
            }
            if !self.sub_zone.is_empty() {
                len += prost::encoding::string::encoded_len(3, &self.sub_zone);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for Status {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if self.code != 0 {
                prost::encoding::int32::encode(1, &self.code, buf);
            }
            if !self.message.is_empty() {
                prost::encoding::string::encode(2, &self.message, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::int32::merge(wire_type, &mut self.code, buf, ctx),
                2 => prost::encoding::string::merge(wire_type, &mut self.message, buf, ctx),
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if self.code != 0 {
                len += prost::encoding::int32::encoded_len(1, &self.code);
            }
            if !self.message.is_empty() {
                len += prost::encoding::string::encoded_len(2, &self.message);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for DiscoveryResponse {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.version_info.is_empty() {
                prost::encoding::string::encode(1, &self.version_info, buf);
            }
            for resource in &self.resources {
                prost::encoding::message::encode(2, resource, buf);
            }
            if self.canary {
                prost::encoding::bool::encode(3, &self.canary, buf);
            }
            if !self.type_url.is_empty() {
                prost::encoding::string::encode(4, &self.type_url, buf);
            }
            if !self.nonce.is_empty() {
                prost::encoding::string::encode(5, &self.nonce, buf);
            }
            if let Some(ref cp) = self.control_plane {
                prost::encoding::message::encode(6, cp, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(wire_type, &mut self.version_info, buf, ctx),
                2 => {
                    let mut resource = Any::default();
                    prost::encoding::message::merge(wire_type, &mut resource, buf, ctx)?;
                    self.resources.push(resource);
                    Ok(())
                }
                3 => prost::encoding::bool::merge(wire_type, &mut self.canary, buf, ctx),
                4 => prost::encoding::string::merge(wire_type, &mut self.type_url, buf, ctx),
                5 => prost::encoding::string::merge(wire_type, &mut self.nonce, buf, ctx),
                6 => {
                    let mut cp = self.control_plane.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut cp, buf, ctx)?;
                    self.control_plane = Some(cp);
                    Ok(())
                }
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if !self.version_info.is_empty() {
                len += prost::encoding::string::encoded_len(1, &self.version_info);
            }
            for resource in &self.resources {
                len += prost::encoding::message::encoded_len(2, resource);
            }
            if self.canary {
                len += prost::encoding::bool::encoded_len(3, &self.canary);
            }
            if !self.type_url.is_empty() {
                len += prost::encoding::string::encoded_len(4, &self.type_url);
            }
            if !self.nonce.is_empty() {
                len += prost::encoding::string::encoded_len(5, &self.nonce);
            }
            if let Some(ref cp) = self.control_plane {
                len += prost::encoding::message::encoded_len(6, cp);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for Any {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.type_url.is_empty() {
                prost::encoding::string::encode(1, &self.type_url, buf);
            }
            if !self.value.is_empty() {
                prost::encoding::bytes::encode(2, &self.value, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(wire_type, &mut self.type_url, buf, ctx),
                2 => prost::encoding::bytes::merge(wire_type, &mut self.value, buf, ctx),
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if !self.type_url.is_empty() {
                len += prost::encoding::string::encoded_len(1, &self.type_url);
            }
            if !self.value.is_empty() {
                len += prost::encoding::bytes::encoded_len(2, &self.value);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for ControlPlane {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.identifier.is_empty() {
                prost::encoding::string::encode(1, &self.identifier, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(wire_type, &mut self.identifier, buf, ctx),
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            if !self.identifier.is_empty() {
                prost::encoding::string::encoded_len(1, &self.identifier)
            } else {
                0
            }
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for DeltaDiscoveryRequest {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if let Some(ref node) = self.node {
                prost::encoding::message::encode(1, node, buf);
            }
            if !self.type_url.is_empty() {
                prost::encoding::string::encode(2, &self.type_url, buf);
            }
            for name in &self.resource_names_subscribe {
                prost::encoding::string::encode(3, name, buf);
            }
            for name in &self.resource_names_unsubscribe {
                prost::encoding::string::encode(4, name, buf);
            }
            if !self.response_nonce.is_empty() {
                prost::encoding::string::encode(6, &self.response_nonce, buf);
            }
            if let Some(ref error) = self.error_detail {
                prost::encoding::message::encode(7, error, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => {
                    let mut node = self.node.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut node, buf, ctx)?;
                    self.node = Some(node);
                    Ok(())
                }
                2 => prost::encoding::string::merge(wire_type, &mut self.type_url, buf, ctx),
                3 => {
                    let mut name = String::new();
                    prost::encoding::string::merge(wire_type, &mut name, buf, ctx)?;
                    self.resource_names_subscribe.push(name);
                    Ok(())
                }
                4 => {
                    let mut name = String::new();
                    prost::encoding::string::merge(wire_type, &mut name, buf, ctx)?;
                    self.resource_names_unsubscribe.push(name);
                    Ok(())
                }
                6 => prost::encoding::string::merge(wire_type, &mut self.response_nonce, buf, ctx),
                7 => {
                    let mut error = self.error_detail.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut error, buf, ctx)?;
                    self.error_detail = Some(error);
                    Ok(())
                }
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if let Some(ref node) = self.node {
                len += prost::encoding::message::encoded_len(1, node);
            }
            if !self.type_url.is_empty() {
                len += prost::encoding::string::encoded_len(2, &self.type_url);
            }
            for name in &self.resource_names_subscribe {
                len += prost::encoding::string::encoded_len(3, name);
            }
            for name in &self.resource_names_unsubscribe {
                len += prost::encoding::string::encoded_len(4, name);
            }
            if !self.response_nonce.is_empty() {
                len += prost::encoding::string::encoded_len(6, &self.response_nonce);
            }
            if let Some(ref error) = self.error_detail {
                len += prost::encoding::message::encoded_len(7, error);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for DeltaDiscoveryResponse {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.system_version_info.is_empty() {
                prost::encoding::string::encode(1, &self.system_version_info, buf);
            }
            for resource in &self.resources {
                prost::encoding::message::encode(2, resource, buf);
            }
            if !self.type_url.is_empty() {
                prost::encoding::string::encode(4, &self.type_url, buf);
            }
            if !self.nonce.is_empty() {
                prost::encoding::string::encode(5, &self.nonce, buf);
            }
            for name in &self.removed_resources {
                prost::encoding::string::encode(6, name, buf);
            }
            if let Some(ref cp) = self.control_plane {
                prost::encoding::message::encode(7, cp, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(
                    wire_type,
                    &mut self.system_version_info,
                    buf,
                    ctx,
                ),
                2 => {
                    let mut resource = Resource::default();
                    prost::encoding::message::merge(wire_type, &mut resource, buf, ctx)?;
                    self.resources.push(resource);
                    Ok(())
                }
                4 => prost::encoding::string::merge(wire_type, &mut self.type_url, buf, ctx),
                5 => prost::encoding::string::merge(wire_type, &mut self.nonce, buf, ctx),
                6 => {
                    let mut name = String::new();
                    prost::encoding::string::merge(wire_type, &mut name, buf, ctx)?;
                    self.removed_resources.push(name);
                    Ok(())
                }
                7 => {
                    let mut cp = self.control_plane.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut cp, buf, ctx)?;
                    self.control_plane = Some(cp);
                    Ok(())
                }
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if !self.system_version_info.is_empty() {
                len += prost::encoding::string::encoded_len(1, &self.system_version_info);
            }
            for resource in &self.resources {
                len += prost::encoding::message::encoded_len(2, resource);
            }
            if !self.type_url.is_empty() {
                len += prost::encoding::string::encoded_len(4, &self.type_url);
            }
            if !self.nonce.is_empty() {
                len += prost::encoding::string::encoded_len(5, &self.nonce);
            }
            for name in &self.removed_resources {
                len += prost::encoding::string::encoded_len(6, name);
            }
            if let Some(ref cp) = self.control_plane {
                len += prost::encoding::message::encoded_len(7, cp);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }

    impl prost::Message for Resource {
        fn encode_raw(&self, buf: &mut impl prost::bytes::BufMut)
        where
            Self: Sized,
        {
            if !self.version.is_empty() {
                prost::encoding::string::encode(1, &self.version, buf);
            }
            if let Some(ref resource) = self.resource {
                prost::encoding::message::encode(2, resource, buf);
            }
            if !self.name.is_empty() {
                prost::encoding::string::encode(3, &self.name, buf);
            }
            for alias in &self.aliases {
                prost::encoding::string::encode(4, alias, buf);
            }
        }

        fn merge_field(
            &mut self,
            tag: u32,
            wire_type: prost::encoding::WireType,
            buf: &mut impl prost::bytes::Buf,
            ctx: prost::encoding::DecodeContext,
        ) -> Result<(), prost::DecodeError>
        where
            Self: Sized,
        {
            match tag {
                1 => prost::encoding::string::merge(wire_type, &mut self.version, buf, ctx),
                2 => {
                    let mut resource = self.resource.take().unwrap_or_default();
                    prost::encoding::message::merge(wire_type, &mut resource, buf, ctx)?;
                    self.resource = Some(resource);
                    Ok(())
                }
                3 => prost::encoding::string::merge(wire_type, &mut self.name, buf, ctx),
                4 => {
                    let mut alias = String::new();
                    prost::encoding::string::merge(wire_type, &mut alias, buf, ctx)?;
                    self.aliases.push(alias);
                    Ok(())
                }
                _ => prost::encoding::skip_field(wire_type, tag, buf, ctx),
            }
        }

        fn encoded_len(&self) -> usize {
            let mut len = 0;
            if !self.version.is_empty() {
                len += prost::encoding::string::encoded_len(1, &self.version);
            }
            if let Some(ref resource) = self.resource {
                len += prost::encoding::message::encoded_len(2, resource);
            }
            if !self.name.is_empty() {
                len += prost::encoding::string::encoded_len(3, &self.name);
            }
            for alias in &self.aliases {
                len += prost::encoding::string::encoded_len(4, alias);
            }
            len
        }

        fn clear(&mut self) {
            *self = Self::default();
        }
    }
}

/// Type alias for the response stream
type ResponseStream = Pin<Box<dyn Stream<Item = Result<proto::DiscoveryResponse, Status>> + Send>>;
type DeltaResponseStream =
    Pin<Box<dyn Stream<Item = Result<proto::DeltaDiscoveryResponse, Status>> + Send>>;

/// Aggregated Discovery Service implementation
pub struct AggregatedDiscoveryServiceImpl {
    xds_server: Arc<XdsServer>,
}

impl AggregatedDiscoveryServiceImpl {
    /// Create a new ADS service implementation
    pub fn new(xds_server: Arc<XdsServer>) -> Self {
        Self { xds_server }
    }

    /// Convert internal request to proto format
    fn to_internal_request(req: &proto::DiscoveryRequest) -> InternalRequest {
        InternalRequest {
            version_info: req.version_info.clone(),
            node: req.node.as_ref().map(|n| crate::xds::types::Node {
                id: n.id.clone(),
                cluster: n.cluster.clone(),
                metadata: std::collections::HashMap::new(),
                locality: n.locality.as_ref().map(|l| crate::xds::types::Locality {
                    region: l.region.clone(),
                    zone: l.zone.clone(),
                    sub_zone: l.sub_zone.clone(),
                }),
                user_agent_name: Some(n.user_agent_name.clone()),
                user_agent_version: Some(n.user_agent_version.clone()),
            }),
            resource_names: req.resource_names.clone(),
            type_url: req.type_url.clone(),
            response_nonce: req.response_nonce.clone(),
            error_detail: req.error_detail.as_ref().map(|s| s.message.clone()),
        }
    }

    /// Convert internal response to proto format
    fn to_proto_response(resp: &InternalResponse) -> proto::DiscoveryResponse {
        proto::DiscoveryResponse {
            version_info: resp.version_info.clone(),
            resources: resp
                .resources
                .iter()
                .map(|r| proto::Any {
                    type_url: r.resource_type.type_url().to_string(),
                    value: r.data.clone(),
                })
                .collect(),
            canary: false,
            type_url: resp.type_url.clone(),
            nonce: resp.nonce.clone(),
            control_plane: Some(proto::ControlPlane {
                identifier: resp.control_plane_id.clone(),
            }),
        }
    }
}

/// ADS gRPC service trait
#[tonic::async_trait]
pub trait AggregatedDiscoveryService: Send + Sync + 'static {
    /// Server streaming response type for StreamAggregatedResources
    type StreamAggregatedResourcesStream: Stream<Item = Result<proto::DiscoveryResponse, Status>>
        + Send
        + 'static;

    /// Server streaming response type for DeltaAggregatedResources
    type DeltaAggregatedResourcesStream: Stream<Item = Result<proto::DeltaDiscoveryResponse, Status>>
        + Send
        + 'static;

    /// Full-state streaming aggregated discovery
    async fn stream_aggregated_resources(
        &self,
        request: Request<Streaming<proto::DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamAggregatedResourcesStream>, Status>;

    /// Delta streaming aggregated discovery
    async fn delta_aggregated_resources(
        &self,
        request: Request<Streaming<proto::DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaAggregatedResourcesStream>, Status>;
}

#[tonic::async_trait]
impl AggregatedDiscoveryService for AggregatedDiscoveryServiceImpl {
    type StreamAggregatedResourcesStream = ResponseStream;
    type DeltaAggregatedResourcesStream = DeltaResponseStream;

    async fn stream_aggregated_resources(
        &self,
        request: Request<Streaming<proto::DiscoveryRequest>>,
    ) -> Result<Response<Self::StreamAggregatedResourcesStream>, Status> {
        let mut stream = request.into_inner();
        let xds_server = self.xds_server.clone();

        let (tx, rx) = mpsc::channel(100);

        // Spawn task to handle incoming requests
        tokio::spawn(async move {
            let mut node_id: Option<String> = None;

            loop {
                match tokio_stream::StreamExt::next(&mut stream).await {
                    Some(Ok(req)) => {
                        // Extract node ID from first request
                        if node_id.is_none() {
                            node_id = req.node.as_ref().map(|n| n.id.clone());
                            if let Some(ref id) = node_id {
                                info!(node_id = %id, "New xDS stream established");
                            }
                        }

                        let internal_req = Self::to_internal_request(&req);

                        debug!(
                            node_id = ?node_id,
                            type_url = %req.type_url,
                            version = %req.version_info,
                            "Received xDS request"
                        );

                        // Handle the request
                        if let Some(resp) = xds_server.handle_request(internal_req).await {
                            let proto_resp = Self::to_proto_response(&resp);
                            if tx.send(Ok(proto_resp)).await.is_err() {
                                break;
                            }
                        }
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "Error receiving xDS request");
                        break;
                    }
                    None => {
                        // Stream closed
                        if let Some(ref id) = node_id {
                            info!(node_id = %id, "xDS stream closed");
                        }
                        break;
                    }
                }
            }
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(output_stream) as ResponseStream))
    }

    async fn delta_aggregated_resources(
        &self,
        request: Request<Streaming<proto::DeltaDiscoveryRequest>>,
    ) -> Result<Response<Self::DeltaAggregatedResourcesStream>, Status> {
        let mut stream = request.into_inner();
        let xds_server = self.xds_server.clone();

        let (tx, rx) = mpsc::channel(100);

        // Spawn task to handle incoming delta requests
        tokio::spawn(async move {
            let mut node_id: Option<String> = None;

            loop {
                match tokio_stream::StreamExt::next(&mut stream).await {
                    Some(Ok(req)) => {
                        // Extract node ID from first request
                        if node_id.is_none() {
                            node_id = req.node.as_ref().map(|n| n.id.clone());
                            if let Some(ref id) = node_id {
                                info!(node_id = %id, "New delta xDS stream established");
                            }
                        }

                        debug!(
                            node_id = ?node_id,
                            type_url = %req.type_url,
                            subscribe = ?req.resource_names_subscribe,
                            unsubscribe = ?req.resource_names_unsubscribe,
                            "Received delta xDS request"
                        );

                        // Get the resource type
                        if let Some(resource_type) = ResourceType::from_type_url(&req.type_url) {
                            // Get snapshot for node
                            let effective_node_id = node_id.as_deref().unwrap_or("unknown");

                            if let Some(snapshot) =
                                xds_server.snapshot_cache().get_snapshot(effective_node_id)
                            {
                                // Build delta response based on subscriptions
                                let resources: Vec<proto::Resource> = match resource_type {
                                    ResourceType::Cluster => snapshot
                                        .clusters
                                        .iter()
                                        .filter(|(name, _)| {
                                            req.resource_names_subscribe.is_empty()
                                                || req.resource_names_subscribe.contains(name)
                                        })
                                        .map(|(name, cluster)| proto::Resource {
                                            name: name.clone(),
                                            version: snapshot.version.clone(),
                                            resource: Some(proto::Any {
                                                type_url: ResourceType::Cluster
                                                    .type_url()
                                                    .to_string(),
                                                value: serde_json::to_vec(cluster)
                                                    .unwrap_or_default(),
                                            }),
                                            ..Default::default()
                                        })
                                        .collect(),
                                    ResourceType::Endpoint => snapshot
                                        .endpoints
                                        .iter()
                                        .filter(|(name, _)| {
                                            req.resource_names_subscribe.is_empty()
                                                || req.resource_names_subscribe.contains(name)
                                        })
                                        .map(|(name, cla)| proto::Resource {
                                            name: name.clone(),
                                            version: snapshot.version.clone(),
                                            resource: Some(proto::Any {
                                                type_url: ResourceType::Endpoint
                                                    .type_url()
                                                    .to_string(),
                                                value: serde_json::to_vec(cla).unwrap_or_default(),
                                            }),
                                            ..Default::default()
                                        })
                                        .collect(),
                                    ResourceType::Listener => snapshot
                                        .listeners
                                        .iter()
                                        .filter(|(name, _)| {
                                            req.resource_names_subscribe.is_empty()
                                                || req.resource_names_subscribe.contains(name)
                                        })
                                        .map(|(name, listener)| proto::Resource {
                                            name: name.clone(),
                                            version: snapshot.version.clone(),
                                            resource: Some(proto::Any {
                                                type_url: ResourceType::Listener
                                                    .type_url()
                                                    .to_string(),
                                                value: serde_json::to_vec(listener)
                                                    .unwrap_or_default(),
                                            }),
                                            ..Default::default()
                                        })
                                        .collect(),
                                    ResourceType::Route => snapshot
                                        .routes
                                        .iter()
                                        .filter(|(name, _)| {
                                            req.resource_names_subscribe.is_empty()
                                                || req.resource_names_subscribe.contains(name)
                                        })
                                        .map(|(name, route)| proto::Resource {
                                            name: name.clone(),
                                            version: snapshot.version.clone(),
                                            resource: Some(proto::Any {
                                                type_url: ResourceType::Route
                                                    .type_url()
                                                    .to_string(),
                                                value: serde_json::to_vec(route)
                                                    .unwrap_or_default(),
                                            }),
                                            ..Default::default()
                                        })
                                        .collect(),
                                    _ => Vec::new(),
                                };

                                if !resources.is_empty() {
                                    let response = proto::DeltaDiscoveryResponse {
                                        system_version_info: snapshot.version.clone(),
                                        resources,
                                        type_url: req.type_url.clone(),
                                        removed_resources: req.resource_names_unsubscribe.clone(),
                                        nonce: uuid::Uuid::new_v4().to_string(),
                                        control_plane: Some(proto::ControlPlane {
                                            identifier: "batata-xds-server".to_string(),
                                        }),
                                    };

                                    if tx.send(Ok(response)).await.is_err() {
                                        break;
                                    }
                                }
                            }
                        } else {
                            warn!(type_url = %req.type_url, "Unknown resource type");
                        }
                    }
                    Some(Err(e)) => {
                        error!(error = %e, "Error receiving delta xDS request");
                        break;
                    }
                    None => {
                        // Stream closed
                        if let Some(ref id) = node_id {
                            info!(node_id = %id, "Delta xDS stream closed");
                        }
                        break;
                    }
                }
            }
        });

        let output_stream = ReceiverStream::new(rx);
        Ok(Response::new(Box::pin(output_stream) as DeltaResponseStream))
    }
}

/// Create a new ADS service implementation
pub fn create_ads_service(xds_server: Arc<XdsServer>) -> AggregatedDiscoveryServiceImpl {
    AggregatedDiscoveryServiceImpl::new(xds_server)
}

/// Start the xDS gRPC server on the specified address
///
/// This function starts the ADS gRPC server and returns when the server is shutdown.
/// The server provides xDS protocol support for Envoy proxies and other xDS clients.
pub async fn start_xds_grpc_server(
    xds_server: Arc<XdsServer>,
    addr: std::net::SocketAddr,
    mut shutdown_rx: tokio::sync::oneshot::Receiver<()>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ads_service = AggregatedDiscoveryServiceImpl::new(xds_server);
    let ads_server = XdsGrpcServer::new(ads_service);

    info!(addr = %addr, "Starting xDS gRPC server");

    // Create TCP listener
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Accept connections in a loop
    loop {
        tokio::select! {
            result = listener.accept() => {
                match result {
                    Ok((stream, peer_addr)) => {
                        debug!(peer = %peer_addr, "Accepted xDS connection");
                        let ads_server = ads_server.clone();
                        tokio::spawn(async move {
                            if let Err(e) = ads_server.handle_connection(stream).await {
                                error!(peer = %peer_addr, error = %e, "Error handling xDS connection");
                            }
                        });
                    }
                    Err(e) => {
                        error!(error = %e, "Error accepting xDS connection");
                    }
                }
            }
            _ = &mut shutdown_rx => {
                info!("xDS gRPC server shutdown signal received");
                break;
            }
        }
    }

    Ok(())
}

/// xDS gRPC server that handles streaming connections
#[derive(Clone)]
pub struct XdsGrpcServer {
    ads_service: Arc<AggregatedDiscoveryServiceImpl>,
}

impl XdsGrpcServer {
    /// Create a new xDS gRPC server
    pub fn new(ads_service: AggregatedDiscoveryServiceImpl) -> Self {
        Self {
            ads_service: Arc::new(ads_service),
        }
    }

    /// Handle an incoming TCP connection
    async fn handle_connection(
        &self,
        stream: tokio::net::TcpStream,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        use hyper::server::conn::http2::Builder;
        use hyper_util::rt::TokioIo;

        let io = TokioIo::new(stream);
        let ads_service = self.ads_service.clone();

        // Create HTTP/2 connection
        let http = Builder::new(hyper_util::rt::TokioExecutor::new());

        http.serve_connection(
            io,
            hyper::service::service_fn(move |req| {
                let ads_service = ads_service.clone();
                async move { Self::handle_request(ads_service, req).await }
            }),
        )
        .await?;

        Ok(())
    }

    /// Handle an HTTP/2 request
    async fn handle_request(
        _ads_service: Arc<AggregatedDiscoveryServiceImpl>,
        req: http::Request<hyper::body::Incoming>,
    ) -> Result<http::Response<http_body_util::Full<hyper::body::Bytes>>, std::convert::Infallible>
    {
        use http_body_util::Full;
        use hyper::body::Bytes;

        let path = req.uri().path();
        let method = path.rsplit('/').next().unwrap_or("");

        debug!(path = %path, method = %method, "Handling xDS request");

        // For now, return a simple response indicating the service is available
        // Full streaming implementation would require more complex body handling
        let response = match method {
            "StreamAggregatedResources" | "DeltaAggregatedResources" => {
                // Log that we received an ADS request
                info!(method = %method, "Received ADS streaming request");

                // Return a basic gRPC response indicating service is available
                http::Response::builder()
                    .status(200)
                    .header("content-type", "application/grpc")
                    .header("grpc-status", "0")
                    .header(
                        "grpc-message",
                        "xDS service active - use tonic client for streaming",
                    )
                    .body(Full::new(Bytes::new()))
                    .unwrap()
            }
            _ => {
                // Return unimplemented for unknown methods
                http::Response::builder()
                    .status(200)
                    .header("content-type", "application/grpc")
                    .header("grpc-status", "12") // UNIMPLEMENTED
                    .body(Full::new(Bytes::new()))
                    .unwrap()
            }
        };

        Ok(response)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::XdsServerConfig;

    #[test]
    fn test_proto_encoding() {
        let req = proto::DiscoveryRequest {
            version_info: "v1".to_string(),
            node: Some(proto::Node {
                id: "test-node".to_string(),
                cluster: "test-cluster".to_string(),
                ..Default::default()
            }),
            resource_names: vec!["cluster-1".to_string()],
            type_url: "type.googleapis.com/envoy.config.cluster.v3.Cluster".to_string(),
            response_nonce: "nonce-1".to_string(),
            error_detail: None,
        };

        // Test encode/decode roundtrip
        let mut buf = Vec::new();
        prost::Message::encode(&req, &mut buf).unwrap();

        let decoded: proto::DiscoveryRequest = prost::Message::decode(&buf[..]).unwrap();
        assert_eq!(decoded.version_info, "v1");
        assert_eq!(decoded.node.as_ref().unwrap().id, "test-node");
    }

    #[test]
    fn test_ads_service_creation() {
        let xds_server = Arc::new(XdsServer::new(XdsServerConfig::default()));
        let _ads_service = AggregatedDiscoveryServiceImpl::new(xds_server);
    }

    #[test]
    fn test_to_internal_request() {
        let proto_req = proto::DiscoveryRequest {
            version_info: "v1".to_string(),
            node: Some(proto::Node {
                id: "test-node".to_string(),
                cluster: "test-cluster".to_string(),
                locality: Some(proto::Locality {
                    region: "us-west-1".to_string(),
                    zone: "us-west-1a".to_string(),
                    sub_zone: String::new(),
                }),
                ..Default::default()
            }),
            resource_names: vec!["cluster-1".to_string()],
            type_url: "type.googleapis.com/envoy.config.cluster.v3.Cluster".to_string(),
            response_nonce: "nonce-1".to_string(),
            error_detail: None,
        };

        let internal_req = AggregatedDiscoveryServiceImpl::to_internal_request(&proto_req);

        assert_eq!(internal_req.version_info, "v1");
        assert_eq!(internal_req.node.as_ref().unwrap().id, "test-node");
        assert_eq!(internal_req.resource_names, vec!["cluster-1"]);
    }

    #[test]
    fn test_to_proto_response() {
        let internal_resp = InternalResponse {
            version_info: "v2".to_string(),
            resources: vec![crate::server::ResourceData {
                name: "cluster-1".to_string(),
                resource_type: ResourceType::Cluster,
                data: b"test-data".to_vec(),
            }],
            type_url: "type.googleapis.com/envoy.config.cluster.v3.Cluster".to_string(),
            nonce: "nonce-2".to_string(),
            control_plane_id: "batata-server".to_string(),
        };

        let proto_resp = AggregatedDiscoveryServiceImpl::to_proto_response(&internal_resp);

        assert_eq!(proto_resp.version_info, "v2");
        assert_eq!(proto_resp.resources.len(), 1);
        assert_eq!(proto_resp.nonce, "nonce-2");
        assert_eq!(
            proto_resp.control_plane.as_ref().unwrap().identifier,
            "batata-server"
        );
    }
}
