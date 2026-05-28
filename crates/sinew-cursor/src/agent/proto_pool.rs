use std::sync::OnceLock;

use prost_reflect::DescriptorPool;
use sinew_core::Result;

static POOL: OnceLock<DescriptorPool> = OnceLock::new();

pub fn agent_pool() -> Result<&'static DescriptorPool> {
    Ok(POOL.get_or_init(|| {
        let bytes = include_bytes!("../../proto/agent.pb");
        DescriptorPool::decode(bytes.as_ref())
            .unwrap_or_else(|err| panic!("agent.pb descriptor pool: {err}"))
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn loads_agent_messages() {
        let pool = agent_pool().expect("pool");
        assert!(
            pool.get_message_by_name("agent.v1.AgentClientMessage")
                .is_some()
        );
        assert!(
            pool.get_message_by_name("agent.v1.AgentServerMessage")
                .is_some()
        );
    }
}
