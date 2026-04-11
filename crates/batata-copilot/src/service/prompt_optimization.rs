//! Prompt optimization service — matches Nacos PromptOptimizationServiceImpl

use std::pin::Pin;
use std::sync::Arc;

use futures::StreamExt;
use tokio_stream::Stream;

use crate::agent::CopilotAgentManager;
use crate::model::response::StreamResponseType;
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
        // 1. Validate (matches Nacos)
        if request.prompt.is_empty() {
            anyhow::bail!("Prompt is required");
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

        // 3. Build user message (matches Nacos Chinese format)
        let user_msg = build_user_message(&request);

        // 4. Call LLM and filter THINKING type (matches Nacos behavior)
        let s = stream::chat_stream(
            config.effective_base_url(),
            api_key,
            config.model.clone(),
            prompt_optimization::SYSTEM_PROMPT.to_string(),
            user_msg,
        )
        .await?;

        // Filter out THINKING type — Nacos PromptOptimizationServiceImpl returns null for thinking
        let filtered = s.filter(|chunk| {
            let keep = chunk.chunk_type != StreamResponseType::Thinking;
            std::future::ready(keep)
        });
        Ok(filtered.boxed())
    }
}

/// Build user message — matches Nacos PromptOptimizationServiceImpl.buildUserMessage
fn build_user_message(request: &PromptOptimizationRequest) -> String {
    let mut sb = String::new();
    sb.push_str("请优化以下 Prompt：\n\n");
    sb.push_str("【原始 Prompt】\n");
    sb.push_str(&request.prompt);

    if let Some(ref goal) = request.optimization_goal
        && !goal.is_empty()
    {
        sb.push_str("\n\n【优化目标】\n");
        sb.push_str(goal);
    }

    sb
}
