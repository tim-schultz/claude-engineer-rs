mod github_tools;
mod prompts;
use async_recursion::async_recursion;
use prompts::{BASE_SYSTEM_PROMPT, CHAIN_OF_THOUGHT_PROMPT};

use serde_json::Value;
mod tools;
use tools::{ToolExecutor, TOOLS};

use anyhow::{anyhow, Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Read;
use std::process::Command;

use anthropic_sdk::{AnthropicResponse, Client, ContentItem};
use log::{error, info, warn};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Message {
    role: String,
    content: MessageContent,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum MessageContent {
    Text(String),
    ToolUseAssistant(Vec<ToolUseAssistant>),
    ToolUseUser(Vec<ToolUseUser>),
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolUseAssistant {
    #[serde(rename = "type")]
    tool_type: String,
    id: String,
    name: String,
    input: Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolUseUser {
    #[serde(rename = "type")]
    tool_type: String,
    tool_use_id: String,
    content: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ToolUseResult {
    id: String,
    name: String,
    input: Value,
    tool_result: String,
}

pub struct Claude {
    client: Client,
    system_prompt: String,
    conversation_history: Vec<Message>,
    current_conversation: Vec<Message>,
    tool_executor: ToolExecutor,
}

pub const MODEL: &str = "claude-3-5-sonnet-20240620";
pub const CONTINUATION_EXIT_PHRASE: &str = "AUTOMODE_COMPLETE";
pub const MAX_CONTINUATION_ITERATIONS: i8 = 25;

impl Claude {
    pub fn new(model: &str) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")
            .context("Failed to get ANTHROPIC_API_KEY from environment")?;
        let client = Client::new()
            .auth(&api_key)
            .model(model)
            .beta("max-tokens-3-5-sonnet-2024-07-15")
            .max_tokens(8000)
            .tools(&TOOLS);
        let system_prompt = format!(
            r#"
            {}
            {}"#,
            BASE_SYSTEM_PROMPT, CHAIN_OF_THOUGHT_PROMPT
        );
        let tool_client = client.clone().system(&system_prompt.clone());
        let tool_executor =
            ToolExecutor::new(tool_client).context("Failed to create ToolExecutor")?;
        Ok(Self {
            client,
            system_prompt,
            conversation_history: vec![],
            current_conversation: vec![],
            tool_executor,
        })
    }

    pub async fn process_content_response(
        &mut self,
        content: Vec<ContentItem>,
    ) -> Result<(String, Vec<ToolUseResult>)> {
        let mut response_text = String::new();
        let mut tool_results: Vec<ToolUseResult> = vec![];
        for item in content {
            match item {
                ContentItem::Text { text } => {
                    info!("Assistant: {}", text);
                    response_text.push_str(&text);
                }
                ContentItem::ToolUse { id, name, input } => {
                    info!("Tool Use: {} ({}), Input: {:?}", name, id, input);
                    let tool_result = self
                        .tool_executor
                        .execute_tool(&name, &input)
                        .await
                        .with_context(|| format!("Failed to execute tool: {}", name))?;

                    tool_results.push(ToolUseResult {
                        id,
                        name,
                        input,
                        tool_result,
                    });
                }
            }
        }
        Ok((response_text, tool_results))
    }

    pub async fn ask_claude_simple(&mut self, prompt: &str) -> Result<AnthropicResponse> {
        self.current_conversation = vec![Message {
            role: "user".to_string(),
            content: MessageContent::Text(prompt.to_string()),
        }];

        let mut combined_conversation = self.conversation_history.clone();
        combined_conversation.extend(self.current_conversation.clone());
        let messages =
            serde_json::to_value(&combined_conversation).context("Failed to serialize messages")?;

        let request = self
            .client
            .clone()
            .messages(&messages)
            .system(&self.system_prompt)
            .build()
            .context("Failed to build Anthropic request")?;

        let res = request
            .execute_and_return_json()
            .await
            .context("Failed to execute Anthropic request")?;
        Ok(res)
    }

    pub async fn ask_claude_tool(
        &mut self,
        tool_results: Vec<ToolUseResult>,
    ) -> Result<AnthropicResponse> {
        for tool_usage in tool_results {
            self.current_conversation.push(Message {
                role: "assistant".to_string(),
                content: MessageContent::ToolUseAssistant(vec![ToolUseAssistant {
                    tool_type: "tool_use".to_string(),
                    id: tool_usage.id.clone(),
                    name: tool_usage.name.clone(),
                    input: tool_usage.input.clone(),
                }]),
            });

            self.current_conversation.push(Message {
                role: "user".to_string(),
                content: MessageContent::ToolUseUser(vec![ToolUseUser {
                    tool_type: "tool_result".to_string(),
                    tool_use_id: tool_usage.id.clone(),
                    content: tool_usage.tool_result,
                }]),
            });
        }

        let mut combined_conversation_after_tool = self.conversation_history.clone();
        combined_conversation_after_tool.extend(self.current_conversation.clone());
        info!(
            "Combined conversation length: {}",
            combined_conversation_after_tool.len()
        );
        let messages_after_tool = serde_json::to_value(&combined_conversation_after_tool)
            .context("Failed to serialize messages after tool use")?;

        let request = self
            .client
            .clone()
            .messages(&messages_after_tool)
            .system(&self.system_prompt)
            .build()
            .context("Failed to build Anthropic request after tool use")?;

        let res = request
            .execute_and_return_json()
            .await
            .context("Failed to execute Anthropic request after tool use")?;
        Ok(res)
    }

    #[async_recursion]
    pub async fn recursive_ask_claude_tool(
        &mut self,
        tool_results: Vec<ToolUseResult>,
    ) -> Result<()> {
        let max_iterations = 5;
        let mut current_iteration = 0;
        let tool_result = self.ask_claude_tool(tool_results).await?;
        if tool_result.stop_reason == "tool_use" && current_iteration < max_iterations {
            current_iteration += 1;
            let (response_text, tool_usages) =
                self.process_content_response(tool_result.content).await?;

            self.recursive_ask_claude_tool(tool_usages).await?;
        } else if current_iteration >= max_iterations {
            warn!("Reached maximum iterations in recursive_ask_claude_tool");
        }
        Ok(())
    }

    pub async fn chat_with_claude(&mut self, prompt: &str) -> Result<String> {
        let response = match self.ask_claude_simple(prompt).await {
            Ok(anthropic_response) => {
                info!("Anthropic response: {:?}", anthropic_response);
                let (response_text, tool_usages) = self
                    .process_content_response(anthropic_response.content)
                    .await?;

                info!("Tool usages: {:?}", tool_usages);
                let tool_result = self.ask_claude_tool(tool_usages).await?;
                info!("Tool result: {:?}", tool_result);

                if tool_result.stop_reason == "tool_use" {
                    let (response_text, tool_usages) =
                        self.process_content_response(tool_result.content).await?;
                    let tool_result = self.ask_claude_tool(tool_usages).await?;
                    if tool_result.stop_reason == "tool_use" {
                        return Ok(response_text);
                    }
                }

                Ok(response_text)
            }
            Err(e) => {
                if e.to_string()
                    .contains("Too many Requests. You have been rate limited.")
                {
                    warn!("Rate limited. Waiting for 5 seconds before retrying...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    return self.recursive_initiate_query_with_tools(prompt).await;
                }
                error!("Execution failed: {:?}", e);
                Err(e.context("Failed to execute query with tools"))
            }
        }?;
        Ok(response)
    }

    #[async_recursion]
    pub async fn recursive_initiate_query_with_tools(&mut self, prompt: &str) -> Result<String> {
        let response = match self.ask_claude_simple(prompt).await {
            Ok(anthropic_response) => {
                info!("Anthropic response: {:?}", anthropic_response);
                let (response_text, tool_usages) = self
                    .process_content_response(anthropic_response.content)
                    .await?;

                info!("Tool usages: {:?}", tool_usages);
                let tool_result = self.ask_claude_tool(tool_usages).await?;

                if tool_result.stop_reason == "tool_use" {
                    let (response_text, tool_usages) =
                        self.process_content_response(tool_result.content).await?;
                    let tool_result = self.ask_claude_tool(tool_usages).await?;
                    if tool_result.stop_reason == "tool_use" {
                        return Ok(response_text);
                    }
                }

                Ok(response_text)
            }
            Err(e) => {
                if e.to_string()
                    .contains("Too many Requests. You have been rate limited.")
                {
                    warn!("Rate limited. Waiting for 5 seconds before retrying...");
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                    return self.recursive_initiate_query_with_tools(prompt).await;
                }
                error!("Execution failed: {:?}", e);
                Err(e.context("Failed to execute query with tools"))
            }
        }?;

        // let response = res?;
        if response.contains(CONTINUATION_EXIT_PHRASE) {
            Ok(response)
        } else {
            info!("Response: {}", response);
            println!("Do you want to proceed with current response? (y/n)");
            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if input.trim().to_lowercase() == "y" {
                self.recursive_initiate_query_with_tools(&response).await?;
            } else {
                let new_prompt = self.load_text_editor()?;
                self.recursive_initiate_query_with_tools(&new_prompt)
                    .await?;
            }
            Ok("Exiting...".to_string())
        }
    }

    pub fn load_text_editor(&mut self) -> Result<String> {
        let file_path = "text.txt";
        fs::write(file_path, "").context("Failed to write to text.txt")?;

        let formatted_path = format!("./{}", file_path);
        info!("Attempting to open file: {}", formatted_path);

        let editors = ["vim"];

        for editor in editors.iter() {
            match Command::new(editor).arg(&formatted_path).status() {
                Ok(status) => {
                    info!("{} editor exited with status: {}", editor, status);
                    break;
                }
                Err(e) => {
                    warn!("Failed to open {} editor: {}", editor, e);
                    if editor == editors.last().unwrap() {
                        warn!("No suitable editor found. Skipping edit step.");
                    }
                }
            }
        }

        let mut file = fs::File::open(file_path).context("Failed to open text.txt")?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)
            .context("Failed to read contents of text.txt")?;
        Ok(contents)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let mut claude = Claude::new(MODEL).context("Failed to initialize Claude")?;
    let contents = claude
        .load_text_editor()
        .context("Failed to load text editor")?;

    loop {
        let response = claude
            .chat_with_claude(&contents)
            .await
            .context("Failed to initiate query with tools")?;
        info!("Looped response: {}", &response);
        if response.contains(CONTINUATION_EXIT_PHRASE) {
            break;
        }
    }

    Ok(())
}
