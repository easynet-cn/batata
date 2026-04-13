//! Skill optimization service — matches Nacos SkillOptimizationServiceImpl
//!
//! Uses a multi-turn conversation pattern:
//! Message 1 (user): Skill content (only the target file's content)
//! Message 2 (assistant): Acknowledge and ask optimization direction
//! Message 3 (user): Optimization requirements (history > tools > goal)

use std::pin::Pin;
use std::sync::Arc;

use futures::StreamExt;
use tokio_stream::Stream;

use crate::agent::CopilotAgentManager;
use crate::model::{ConversationMessage, SkillOptimizationRequest, StreamChunk};
use crate::prompt::skill_optimization;
use crate::stream;

const SKILL_MD_FILE_NAME: &str = "SKILL.md";
const SKILL_MD_KEY: &str = "skill-md";
const RESOURCE_KEYWORD_EN: &str = "resource";
const RESOURCE_KEYWORD_ZH: &str = "资源";

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
        // 1. Validate skill
        if request.skill.is_null() {
            anyhow::bail!("Skill object is required in request");
        }

        // 2. Validate target file name (required, matches Nacos)
        let target_file_name = request.target_file_name.as_deref().unwrap_or("");
        if target_file_name.is_empty() {
            anyhow::bail!("Target file name is required. Please select a file to optimize.");
        }

        // 3. Check if Copilot is enabled
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

        // 4. Build conversation messages (multi-turn: user→assistant→user)
        let messages = build_conversation_messages(&request.skill, &request);

        // 5. Call LLM with conversation history
        let s = stream::chat_stream_with_history(
            config.effective_base_url(),
            api_key,
            config.model.clone(),
            skill_optimization::SYSTEM_PROMPT.to_string(),
            messages,
            String::new(), // Last user message is already in the messages list
        )
        .await?;
        Ok(s.boxed())
    }
}

/// Build conversation messages as user-assistant-user pairs.
/// Matches Nacos SkillOptimizationServiceImpl.buildConversationMessages
fn build_conversation_messages(
    skill: &serde_json::Value,
    request: &SkillOptimizationRequest,
) -> Vec<(String, String)> {
    let mut messages = Vec::new();

    let has_optimization_goal = request
        .optimization_goal
        .as_ref()
        .is_some_and(|g| !g.is_empty());

    // Check selectedMcpTools from params (matches Nacos controller behavior)
    let selected_mcp_tools_from_params: Option<&Vec<serde_json::Value>> = request
        .params
        .get("selectedMcpTools")
        .and_then(|v| v.as_array());
    let has_selected_tools = selected_mcp_tools_from_params.is_some_and(|tools| !tools.is_empty())
        || !request.selected_mcp_tools.is_empty();

    let has_conversation_history = request
        .conversation_history
        .as_ref()
        .is_some_and(|h| !h.messages.is_empty());

    let target_file_name = request.target_file_name.as_deref().unwrap_or("");

    // Message 1: User provides Skill information (only the target file's content)
    let mut skill_info = String::new();
    let name = skill.get("name").and_then(|v| v.as_str()).unwrap_or("");
    skill_info.push_str(&format!("名称：{}\n", name));

    let is_skill_md = target_file_name == SKILL_MD_FILE_NAME || target_file_name == SKILL_MD_KEY;

    if is_skill_md {
        let description = skill
            .get("description")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let skill_md = skill.get("skillMd").and_then(|v| v.as_str()).unwrap_or("");
        skill_info.push_str(&format!("描述：{}\n", description));
        skill_info.push_str(&format!("SKILL.md：\n{}\n", skill_md));
    } else if let Some(resource) = skill.get("resource").and_then(|v| v.as_object())
        && !resource.is_empty()
    {
        let mut found = false;
        for (key, res) in resource {
            let res_name = res.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let match_by_key = key == target_file_name;
            let match_by_name = !res_name.is_empty() && res_name == target_file_name;

            if match_by_key || match_by_name {
                found = true;
                skill_info.push_str(&format!("\n目标文件：{}\n", key));
                skill_info.push_str(&format!("文件名：{}\n", res_name));
                if let Some(t) = res.get("type").and_then(|v| v.as_str())
                    && !t.is_empty()
                {
                    skill_info.push_str(&format!("类型：{}\n", t));
                }
                if let Some(c) = res.get("content").and_then(|v| v.as_str())
                    && !c.is_empty()
                {
                    skill_info.push_str(&format!("内容：\n{}\n", c));
                }
                break;
            }
        }
        if !found {
            skill_info.push_str(&format!(
                "\n注意：未找到指定的文件 {}，以下是所有资源文件供参考：\n",
                target_file_name
            ));
            for (key, res) in resource {
                let res_name = res.get("name").and_then(|v| v.as_str()).unwrap_or("");
                skill_info.push_str(&format!("- {}：{}", key, res_name));
                if let Some(t) = res.get("type").and_then(|v| v.as_str())
                    && !t.is_empty()
                {
                    skill_info.push_str(&format!(" (type: {})", t));
                }
                skill_info.push('\n');
            }
        }
    }

    messages.push(("user".to_string(), skill_info));

    // Message 2: Assistant acknowledges and asks how to optimize
    messages.push((
        "assistant".to_string(),
        "我已经收到了这个 Skill 的信息。你希望我怎么优化这条 skill？".to_string(),
    ));

    // Message 3: User provides optimization requirements
    if !has_conversation_history && !has_selected_tools && !has_optimization_goal {
        // Simplest case: no specific requirements
        let simple_request = format!(
            "请帮我看看这个文件（{}）有没有明显可以优化的地方。\
             请只优化这个文件的内容，其他文件保持不变。如果没有明显问题，保持原样即可。",
            target_file_name
        );
        messages.push(("user".to_string(), simple_request));
    } else {
        // Complex case: build structured optimization request
        let mut opt_request = String::new();

        // Part 1: Conversation history
        if has_conversation_history {
            let history = request.conversation_history.as_ref().unwrap();
            opt_request.push_str("以下是一段对话交互历史。请仔细分析这段对话，完成以下任务：\n");
            opt_request.push_str(
                "1. 分析对话中的交互场景：识别用户的需求、助手的处理逻辑、工具调用的模式和流程\n",
            );
            opt_request
                .push_str("2. 将对话场景沉淀为标准流程：提取出可复用的标准操作步骤和决策逻辑\n");
            opt_request.push_str("3. 基于沉淀的标准流程优化 Skill：将分析出的标准流程融入到 Skill 的 instruction 中，确保 Skill 能够支持类似的对话场景\n\n");

            if let Some(ref title) = history.title
                && !title.is_empty()
            {
                opt_request.push_str(&format!("对话主题：{}\n", title));
            }
            if let Some(ref context) = history.context
                && !context.is_empty()
            {
                opt_request.push_str(&format!("对话上下文：{}\n", context));
            }
            opt_request.push_str("\n对话交互内容：\n");
            append_conversation_messages(&mut opt_request, &history.messages);
        }

        // Part 2: MCP tools
        if has_selected_tools {
            let tools = if let Some(tools) = selected_mcp_tools_from_params {
                tools.as_slice()
            } else {
                request.selected_mcp_tools.as_slice()
            };
            if has_conversation_history {
                opt_request.push('\n');
            }
            opt_request.push_str("我希望将以下 MCP 工具整合到这个 Skill 中：\n\n");
            for tool in tools {
                if let Some(name) = tool.get("name") {
                    opt_request.push_str(&format!("工具：{}\n", name));
                }
                if let Some(desc) = tool.get("description") {
                    opt_request.push_str(&format!("描述：{}\n", desc));
                }
                if let Some(schema) = tool.get("inputSchema") {
                    opt_request.push_str(&format!("参数：{}\n", schema));
                }
                opt_request.push('\n');
            }
        }

        // Part 3: Optimization goal (last, highest priority via recency effect)
        if has_optimization_goal {
            let goal = request.optimization_goal.as_deref().unwrap();
            if has_conversation_history || has_selected_tools {
                opt_request.push('\n');
            }
            opt_request.push_str(&format!("【重要】我的优化目标是：{}\n", goal));
            opt_request.push_str(
                "请优先考虑并聚焦于这个优化目标，所有优化建议和改动都应该围绕这个目标展开。",
            );

            // If goal mentions resources, add SKILL.md constraint
            let goal_lower = goal.to_lowercase();
            let contains_resource_keyword = goal_lower.contains(RESOURCE_KEYWORD_ZH)
                || goal_lower.contains(RESOURCE_KEYWORD_EN)
                || goal_lower.contains("增加")
                || goal_lower.contains("添加")
                || goal_lower.contains("add")
                || goal_lower.contains("增加资源")
                || goal_lower.contains("添加资源");
            if contains_resource_keyword {
                opt_request.push_str("\n【绝对禁止】如果优化目标涉及添加或增加资源，请注意：");
                opt_request.push_str("\n- 绝对不能将 SKILL.md 放在 resource 字段中");
                opt_request.push_str("\n- 绝对不能创建名为 SKILL.md 的资源文件");
                opt_request.push_str(
                    "\n- 绝对不能将 SKILL.md 放在任何资源类型（template、data、script 等）下",
                );
                opt_request.push_str(
                    "\n- SKILL.md 是特殊的元数据文件，位于 skillMd 字段，不需要也不应该在 resource 中定义",
                );
                opt_request.push_str(
                    "\n- 只能添加真正的资源文件（如 .json、.yaml、.txt 等），绝对不能添加 SKILL.md",
                );
            }
        }

        // Part 4: Final request with context-aware emphasis
        opt_request.push_str("\n\n");

        // Always add SKILL.md constraint
        opt_request.push_str("\n【绝对禁止】无论优化目标是什么，都绝对不能：");
        opt_request.push_str("\n- 将 SKILL.md 放在 resource 字段中");
        opt_request.push_str("\n- 创建名为 SKILL.md 的资源文件");
        opt_request.push_str("\n- 将 SKILL.md 放在任何资源类型下");

        let target_file_constraint = format!(
            "【重要】请只优化文件 {} 的内容，其他文件保持不变。",
            target_file_name
        );

        if has_optimization_goal {
            opt_request.push_str(&target_file_constraint);
            opt_request.push(' ');
            opt_request.push_str("请基于以上要求优化这个文件，务必优先满足我的优化目标");
            if has_conversation_history && has_selected_tools {
                opt_request.push_str(
                    "，同时将从对话历史中分析出的标准流程融入到优化方案中，并确保工具整合服务于优化目标",
                );
            } else if has_conversation_history {
                opt_request.push_str("，同时将从对话历史中分析出的标准流程融入到优化方案中");
            } else if has_selected_tools {
                opt_request.push_str("，并确保工具整合服务于优化目标");
            }
            opt_request.push('。');
        } else if has_conversation_history {
            opt_request.push_str(&target_file_constraint);
            opt_request.push(' ');
            opt_request
                .push_str("请基于以上要求，特别是从对话历史中分析出的标准流程，优化这个文件");
            if has_selected_tools {
                opt_request.push_str("，并整合上述工具");
            }
            opt_request.push('。');
        } else if has_selected_tools {
            opt_request.push_str(&target_file_constraint);
            opt_request.push(' ');
            opt_request.push_str("请基于以上要求，整合上述工具并优化这个文件。");
        } else {
            opt_request.push_str(&target_file_constraint);
            opt_request.push(' ');
            opt_request.push_str("请基于以上要求优化这个文件。");
        }

        messages.push(("user".to_string(), opt_request));
    }

    messages
}

/// Append conversation messages in Batata/Nacos format
fn append_conversation_messages(sb: &mut String, messages: &[ConversationMessage]) {
    for message in messages {
        let msg_type = message.msg_type.to_lowercase();
        match msg_type.as_str() {
            "user" => {
                sb.push_str(&format!("用户：{}\n", message.content));
            }
            "tool_call" => {
                sb.push_str("工具调用：");
                if let Some(ref name) = message.tool_name {
                    sb.push_str(name);
                }
                if let Some(ref input) = message.tool_input
                    && !input.is_empty()
                {
                    sb.push_str(&format!("，参数：{:?}", input));
                }
                if let Some(ref output) = message.tool_output {
                    sb.push_str(&format!("，结果：{}", output));
                }
                sb.push('\n');
            }
            "model" => {
                sb.push_str(&format!("助手：{}\n", message.content));
            }
            _ => {}
        }
    }
}
