//! Skill generation service — matches Nacos SkillGenerationServiceImpl

use std::pin::Pin;
use std::sync::Arc;

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
        // 1. Validate request (matches Nacos)
        if request.background_info.is_empty() {
            anyhow::bail!("Background information is required");
        }

        // 2. Check if Copilot is enabled (matches Nacos isEnabled check)
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

        // 3. Build user message (matches Nacos buildUserMessage)
        let user_msg = build_user_message(&request);

        // 4. Call LLM with streaming
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

/// Build user message — matches Nacos SkillGenerationServiceImpl.buildUserMessage
fn build_user_message(request: &SkillGenerationRequest) -> String {
    let mut sb = String::new();

    let has_conversation_history = request
        .conversation_history
        .as_ref()
        .is_some_and(|h| !h.messages.is_empty());

    // Add conversation history analysis if provided
    if has_conversation_history {
        let history = request.conversation_history.as_ref().unwrap();
        sb.push_str("对话历史分析（请充分理解这段对话历史，判断是否适合沉淀为一个 Skill）：\n\n");

        if let Some(ref title) = history.title
            && !title.is_empty()
        {
            sb.push_str(&format!("对话主题：{}\n", title));
        }
        if let Some(ref context) = history.context
            && !context.is_empty()
        {
            sb.push_str(&format!("对话上下文：{}\n", context));
        }
        sb.push_str("\n对话内容：\n");

        for (i, message) in history.messages.iter().enumerate() {
            let idx = i + 1;
            let msg_type = message.msg_type.to_lowercase();
            match msg_type.as_str() {
                "user" => {
                    sb.push_str(&format!("[{}] 用户输入：{}\n", idx, message.content));
                }
                "tool_call" => {
                    sb.push_str(&format!("[{}] 工具调用：", idx));
                    if let Some(ref name) = message.tool_name {
                        sb.push_str(name);
                    }
                    if let Some(ref input) = message.tool_input
                        && !input.is_empty()
                    {
                        sb.push_str(&format!("，输入参数：{:?}", input));
                    }
                    if let Some(ref output) = message.tool_output {
                        sb.push_str(&format!("，输出结果：{}", output));
                    }
                    sb.push('\n');
                }
                "model" => {
                    sb.push_str(&format!("[{}] 模型回复：{}\n", idx, message.content));
                }
                _ => {
                    sb.push_str(&format!("[{}] {}：", idx, message.msg_type));
                    if !message.content.is_empty() {
                        sb.push_str(&message.content);
                    }
                    sb.push('\n');
                }
            }
        }

        sb.push_str("\n对话历史分析要求：\n");
        sb.push_str("1. 请充分理解这段对话历史，包括用户输入、工具调用、模型回复的完整流程\n");
        sb.push_str("2. 判断这段对话历史是否适合沉淀为一个 Skill\n");
        sb.push_str("3. 如果适合，请识别对话历史中的关键信息：\n");
        sb.push_str("   - 用户的实际需求和意图\n");
        sb.push_str("   - 工具调用的模式和逻辑\n");
        sb.push_str("   - 模型回复的策略和方式\n");
        sb.push_str("   - 对话中体现出的 Skill 应该具备的核心能力\n");
        sb.push_str("4. 基于对话历史分析，生成一个能够复现类似对话场景的 Skill\n");
        sb.push_str(
            "5. 如果对话历史中涉及工具调用，请在生成的 Skill instruction 中详细说明如何调用这些工具\n",
        );
        sb.push_str("6. 如果对话历史中体现了特定的处理逻辑或策略，请在生成的 Skill instruction 中体现这些逻辑\n\n");
    }

    sb.push_str("请根据以下背景信息生成一个 Agent Skill：\n\n");
    sb.push_str("背景信息：\n");
    sb.push_str(&request.background_info);
    sb.push_str("\n\n");

    // Add MCP tools information if provided
    if !request.selected_mcp_tools.is_empty() {
        sb.push_str("可用的 MCP 工具（可根据 Skill 功能需求合理选择使用）：\n");
        for tool in &request.selected_mcp_tools {
            if let Some(name) = tool.get("name") {
                sb.push_str(&format!("- 工具名称：{}\n", name));
            }
            if let Some(desc) = tool.get("description") {
                sb.push_str(&format!("  描述：{}\n", desc));
            }
            if let Some(schema) = tool.get("inputSchema") {
                sb.push_str(&format!("  输入参数：{}\n", schema));
            }
            sb.push('\n');
        }
        sb.push_str("工具使用说明：\n");
        sb.push_str("1. 请根据 Skill 的功能需求和上下文，合理判断是否需要使用这些工具\n");
        sb.push_str("2. 如果工具对实现 Skill 功能有帮助，则在 instruction 中详细说明如何调用这些工具，包括：\n");
        sb.push_str("   - 工具名称和用途\n");
        sb.push_str("   - 调用时机（在什么情况下调用该工具）\n");
        sb.push_str("   - 输入参数说明（每个参数的含义、类型、是否必需、如何获取）\n");
        sb.push_str("   - 输出结果处理（如何处理工具返回的结果，如何解析和使用返回数据）\n");
        sb.push_str("   - 错误处理（工具调用失败时的处理方式和备选方案）\n");
        sb.push_str(
            "3. 如果工具对实现 Skill 功能没有帮助，则不需要在 instruction 中提及这些工具\n",
        );
        sb.push_str(
            "4. 如果使用了工具，确保工具调用逻辑清晰、可执行，工具应该与 Skill 功能紧密结合\n",
        );
        sb.push_str("5. 如果使用了多个工具，在 instruction 中明确说明工具调用的步骤和流程，包括工具调用的顺序\n");
        sb.push_str(
            "6. 如果使用了工具，提供具体的工具调用示例，说明如何构造参数、调用工具、处理结果\n\n",
        );
    }

    sb.push_str("请根据 Agent Skill 的最佳实践，生成一个完整、高质量、可直接使用的 Skill。");

    sb
}
