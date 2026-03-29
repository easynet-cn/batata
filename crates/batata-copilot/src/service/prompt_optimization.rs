//! Prompt optimization service

use std::sync::Arc;

use std::pin::Pin;

use futures::StreamExt;
use tokio_stream::Stream;

use crate::agent::CopilotAgentManager;
use crate::model::{PromptOptimizationRequest, StreamChunk};
use crate::prompt::prompt_optimization;
use crate::stream;

pub struct PromptOptimizationService {
    agent_manager: Arc<CopilotAgentManager>,
}

impl PromptOptimizationService {
    pub fn new(agent_manager: Arc<CopilotAgentManager>) -> Self {
        Self { agent_manager }
    }

    pub async fn optimize_stream(
        &self,
        request: PromptOptimizationRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamChunk> + Send>>> {
        if request.prompt.is_empty() {
            anyhow::bail!("prompt is required");
        }

        let config = self.agent_manager.get_config().await;
        let api_key = config
            .effective_api_key()
            .ok_or_else(|| anyhow::anyhow!("Copilot API key not configured"))?;

        let mut user_msg = format!("## Prompt to Optimize\n\n{}", request.prompt);
        if let Some(ref goal) = request.optimization_goal {
            user_msg.push_str(&format!("\n\n## Optimization Goal\n\n{}", goal));
        }

        let s = stream::chat_stream(
            config.effective_base_url(),
            api_key,
            config.model.clone(),
            prompt_optimization::SYSTEM_PROMPT.to_string(),
            user_msg,
        )
        .await?;
        Ok(s.boxed())
    }
}
