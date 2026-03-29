//! Prompt debug service — tests prompts with user input

use std::sync::Arc;

use std::pin::Pin;

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

    /// Debug a prompt by simulating it with user input.
    pub async fn debug_stream(
        &self,
        request: PromptDebugRequest,
    ) -> anyhow::Result<Pin<Box<dyn Stream<Item = StreamChunk> + Send>>> {
        if request.prompt.is_empty() {
            anyhow::bail!("prompt is required");
        }
        if request.user_input.is_empty() {
            anyhow::bail!("userInput is required");
        }

        let config = self.agent_manager.get_config().await;
        let api_key = config
            .effective_api_key()
            .ok_or_else(|| anyhow::anyhow!("Copilot API key not configured"))?;

        // Use the prompt-under-test as system prompt, user_input as user message
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
