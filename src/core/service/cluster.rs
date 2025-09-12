use std::sync::Arc;

use dashmap::DashMap;

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
