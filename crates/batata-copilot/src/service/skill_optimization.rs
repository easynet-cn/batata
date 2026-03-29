//! Skill optimization service

use std::sync::Arc;

use std::pin::Pin;

use futures::StreamExt;
use tokio_stream::Stream;

use crate::agent::CopilotAgentManager;
use crate::model::{SkillOptimizationRequest, StreamChunk};
use crate::prompt::skill_optimization;
use crate::stream;

pub struct SkillOptimizationService {
    agent_manager: Arc<CopilotAgentManager>,
}

impl SkillOptimizationService {
    pub fn new(agent_manager: Arc<CopilotAgentManager>) -> Self {
        Self { agent_manager }
    }

    pub async fn optimize_stream(
        &self,
        request: SkillOptimizationRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamChunk> + Send>>> {
        let config = self.agent_manager.get_config().await;
        let api_key = config
            .effective_api_key()
            .ok_or_else(|| anyhow::anyhow!("Copilot API key not configured"))?;

        let skill_json = serde_json::to_string_pretty(&request.skill)?;
        let mut user_msg = format!("## Current Skill\n\n```json\n{}\n```", skill_json);

        if let Some(ref goal) = request.optimization_goal {
            user_msg.push_str(&format!("\n\n## Optimization Goal\n\n{}", goal));
        }
        if let Some(ref target) = request.target_file_name {
            user_msg.push_str(&format!("\n\n## Target File\n\nOptimize: `{}`", target));
        }
        if !request.selected_mcp_tools.is_empty() {
            let tools =
                serde_json::to_string_pretty(&request.selected_mcp_tools).unwrap_or_default();
            user_msg.push_str(&format!(
                "\n\n## Available MCP Tools\n\n```json\n{}\n```",
                tools
            ));
        }

        if let Some(ref history) = request.conversation_history {
            let pairs: Vec<(String, String)> = history
                .messages
                .iter()
                .map(|m| (m.role.clone(), m.content.clone()))
                .collect();
            let s = stream::chat_stream_with_history(
                config.effective_base_url(),
                api_key,
                config.model.clone(),
                skill_optimization::SYSTEM_PROMPT.to_string(),
                pairs,
                user_msg,
            )
            .await?;
            Ok(s.boxed())
        } else {
            let s = stream::chat_stream(
                config.effective_base_url(),
                api_key,
                config.model.clone(),
                skill_optimization::SYSTEM_PROMPT.to_string(),
                user_msg,
            )
            .await?;
            Ok(s.boxed())
        }
    }
}
