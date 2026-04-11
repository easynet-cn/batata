/// Prompt debug does NOT have its own system prompt in Nacos.
/// The user's prompt is used directly as the system prompt,
/// and the user input is sent as the user message.
/// This constant is kept for reference only.
pub const _SYSTEM_PROMPT_UNUSED: &str = "\
You are a prompt testing assistant. The user will provide a system prompt and a user input. \
Your job is to simulate the behavior of an AI model that has been given that system prompt, \
and respond to the user input exactly as that model would.";
