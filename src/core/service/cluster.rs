use std::sync::Arc;

use dashmap::DashMap;
use serde_json::Value;

use crate::{
    api::model::{Member, MemberBuilder},
    local_ip,
    model::common::Configuration,
};

#[derive(Clone, Debug)]
pub struct ServerMemberManager {
    port: u16,
    local_address: String,
    self_member: Arc<Member>,
    server_list: Arc<DashMap<String, Member>>,
}

impl ServerMemberManager {
    pub fn new(config: &Configuration) -> Self {
        let ip = local_ip();
        let port = config.server_main_port();
        let local_address = format!("{}:{}", ip, port);

        let server_list = Arc::new(DashMap::new());
        let mut member = MemberBuilder::new(ip, port).build();

        member.extend_info.write().unwrap().insert(
            Member::RAFT_PORT.to_string(),
            Value::from(member.calculate_raft_port()),
        );
        member
            .extend_info
            .write()
            .unwrap()
            .insert(Member::READY_TO_UPGRADE.to_string(), Value::from(true));
        member
            .extend_info
            .write()
            .unwrap()
            .insert(Member::VERSION.to_string(), Value::from(config.version()));
        member
            .extend_info
            .write()
            .unwrap()
            .insert(Member::SUPPORT_GRAY_MODEL.to_string(), Value::from(true));

        server_list.insert(local_address.clone(), member.clone());

        Self {
            port: port,
            local_address: local_address,
            self_member: Arc::new(member),
            server_list: server_list,
        }
    }

    pub fn all_members(&self) -> Vec<Member> {
        self.server_list.iter().map(|e| e.value().clone()).collect()
    }
}
