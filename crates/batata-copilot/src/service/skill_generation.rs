//! Skill generation service — generates new skills from background info

use std::sync::Arc;

use std::pin::Pin;

use futures::StreamExt;
use tokio_stream::Stream;

use crate::agent::CopilotAgentManager;
use crate::model::{SkillGenerationRequest, StreamChunk};
use crate::prompt::skill_generation;
use crate::stream;

pub struct SkillGenerationService {
    agent_manager: Arc<CopilotAgentManager>,
}

impl SkillGenerationService {
    pub fn new(agent_manager: Arc<CopilotAgentManager>) -> Self {
        Self { agent_manager }
    }

    pub async fn generate_stream(
        &self,
        request: SkillGenerationRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamChunk> + Send>>> {
        if request.background_info.is_empty() {
            anyhow::bail!("backgroundInfo is required");
        }

        let config = self.agent_manager.get_config().await;
        let api_key = config
            .effective_api_key()
            .ok_or_else(|| anyhow::anyhow!("Copilot API key not configured"))?;

        let mut user_msg = format!("## Background Information\n\n{}", request.background_info);

        if !request.selected_mcp_tools.is_empty() {
            let tools_json =
                serde_json::to_string_pretty(&request.selected_mcp_tools).unwrap_or_default();
            user_msg.push_str(&format!(
                "\n\n## Available MCP Tools\n\n```json\n{}\n```",
                tools_json
            ));
        }

        if let Some(ref history) = request.conversation_history {
            let history_pairs: Vec<(String, String)> = history
                .messages
                .iter()
                .map(|m| (m.role.clone(), m.content.clone()))
                .collect();

            let s = stream::chat_stream_with_history(
                config.effective_base_url(),
                api_key,
                config.model.clone(),
                skill_generation::SYSTEM_PROMPT.to_string(),
                history_pairs,
                user_msg,
            )
            .await?;
            Ok(s.boxed())
        } else {
            let s = stream::chat_stream(
                config.effective_base_url(),
                api_key,
                config.model.clone(),
                skill_generation::SYSTEM_PROMPT.to_string(),
                user_msg,
            )
            .await?;
            Ok(s.boxed())
        }
    }
}
