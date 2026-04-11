//! Prompt debug service — matches Nacos PromptDebugServiceImpl

use std::pin::Pin;
use std::sync::Arc;

use futures::StreamExt;
use tokio_stream::Stream;

use crate::agent::CopilotAgentManager;
use crate::model::{PromptDebugRequest, StreamChunk};
use crate::stream;

pub struct PromptDebugService {
    agent_manager: Arc<CopilotAgentManager>,
}

impl PromptDebugService {
    pub fn new(agent_manager: Arc<CopilotAgentManager>) -> Self {
        Self { agent_manager }
    }

    /// Debug a prompt by using it as system prompt with user input.
    /// Unlike optimization, THINKING is included in debug response (matches Nacos).
    pub async fn debug_stream(
        &self,
        request: PromptDebugRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamChunk> + Send>>> {
        // 1. Validate (matches Nacos)
        if request.prompt.is_empty() {
            anyhow::bail!("Prompt is required");
        }
        if request.user_input.is_empty() {
            anyhow::bail!("User input is required");
        }

        // 2. Check if Copilot is enabled
        if !self.agent_manager.is_enabled().await {
            anyhow::bail!(
                "AI 功能未启用：请配置 Copilot API Key。\
                 请设置 nacos.copilot.llm.apiKey 或环境变量 COPILOT_API_KEY"
            );
        }

        let config = self.agent_manager.get_config().await;
        let api_key = config
            .effective_api_key()
            .ok_or_else(|| anyhow::anyhow!("Copilot API key not configured"))?;

        // 3. Use the prompt-under-test as system prompt, user_input as user message
        let s = stream::chat_stream(
            config.effective_base_url(),
            api_key,
            config.model.clone(),
            request.prompt,
            request.user_input,
        )
        .await?;
        Ok(s.boxed())
    }
}
