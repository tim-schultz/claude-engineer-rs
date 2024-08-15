mod github_tools;
mod prompts;
use async_recursion::async_recursion;
use conversation_manager::Message;
use env_logger::Env;
use log::debug;
use prompts::{BASE_SYSTEM_PROMPT, CHAIN_OF_THOUGHT_PROMPT};

use serde_json::Value;

mod tools;
use tools::{ToolExecutor, TOOLS};

mod conversation_manager;
use conversation_manager::ConversationManager;

// mod language_documentation;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::io::Read;
use std::process::Command;

use dotenv::dotenv;

use anthropic_sdk::{AnthropicResponse, Client, ContentItem};
use log::{error, info, warn};

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
    conversation_manager: ConversationManager,
    tool_executor: ToolExecutor,
}

pub const MODEL: &str = "claude-3-5-sonnet-20240620";
pub const CONTINUATION_EXIT_PHRASE: &str = "AUTOMODE_COMPLETE";
pub const MAX_CONTINUATION_ITERATIONS: i8 = 25;

impl Claude {
    pub fn new(model: &str) -> Result<Self> {
        dotenv().ok();

        let api_key = std::env::var("ANTHROPIC_API_KEY_RS")
            .context("Failed to get ANTHROPIC_API_KEY_RS from environment")?;
        // .beta("max-tokens-3-5-sonnet-2024-07-15")
        let client = Client::new()
            .auth(&api_key)
            .model(model)
            .max_tokens(4000)
            .tools(&TOOLS)
            .beta("prompt-caching-2024-07-31");
        let system_prompt = format!(
            r#"
            {}
            {}"#,
            BASE_SYSTEM_PROMPT, CHAIN_OF_THOUGHT_PROMPT
        );
        let tool_client = client.clone().system(&system_prompt.clone());
        let tool_executor =
            ToolExecutor::new(tool_client).context("Failed to create ToolExecutor")?;
        let conversation_manager = ConversationManager::new(1000);
        Ok(Self {
            client,
            system_prompt,
            conversation_manager,
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
        info!("Calling ask_claude_simple function");

        self.conversation_manager.clear_current();

        self.conversation_manager.add_to_current(Message {
            role: "user".to_string(),
            content: MessageContent::Text(prompt.to_string()),
        });
        info!("Added new message to current conversation");

        let combined_conversation = self.conversation_manager.get_combined_conversation();
        info!(
            "Combined conversation message count: {}",
            combined_conversation.len()
        );

        let messages =
            serde_json::to_value(&combined_conversation).context("Failed to serialize messages")?;
        info!("Serialized messages for Anthropic request");

        let request = self
            .client
            .clone()
            .messages(&messages)
            .system(&self.system_prompt)
            .build()
            .context("Failed to build Anthropic request")?;
        info!("Built Anthropic request");

        match request.execute_and_return_json().await {
            Ok(res) => {
                info!("Successfully executed Anthropic request");
                Ok(res)
            }
            Err(e) => {
                error!("Failed to execute Anthropic request: {:?}", e);
                Err(e.into())
            }
        }
    }

    pub async fn ask_claude_tool(
        &mut self,
        tool_results: Vec<ToolUseResult>,
    ) -> Result<AnthropicResponse> {
        info!("Tool usages: {:?}", &tool_results);
        for tool_usage in tool_results {
            self.conversation_manager.add_to_current(Message {
                role: "assistant".to_string(),
                content: MessageContent::ToolUseAssistant(vec![ToolUseAssistant {
                    tool_type: "tool_use".to_string(),
                    id: tool_usage.id.clone(),
                    name: tool_usage.name.clone(),
                    input: tool_usage.input.clone(),
                }]),
            });

            self.conversation_manager.add_to_current(Message {
                role: "user".to_string(),
                content: MessageContent::ToolUseUser(vec![ToolUseUser {
                    tool_type: "tool_result".to_string(),
                    tool_use_id: tool_usage.id.clone(),
                    content: tool_usage.tool_result,
                }]),
            });
        }

        let combined_conversation = self.conversation_manager.get_combined_conversation();
        info!(
            "Combined conversation length: {}",
            combined_conversation.len()
        );
        let messages = serde_json::to_value(&combined_conversation)
            .context("Failed to serialize messages after tool use")?;

        let request = self
            .client
            .clone()
            .messages(&messages)
            .system(&self.system_prompt)
            .build()
            .context("Failed to build Anthropic request after tool use")?;

        let res = request
            .execute_and_return_json()
            .await
            .context("Failed to execute Anthropic request after tool use")?;
        info!("Tool result: {:?}", res);
        Ok(res)
    }

    pub fn commit_conversation(&mut self) {
        self.conversation_manager.commit_current_to_history();
    }

    #[async_recursion]
    pub async fn chat_with_claude(&mut self, prompt: &str) -> Result<String> {
        let response = match self.ask_claude_simple(prompt).await {
            Ok(anthropic_response) => {
                info!("Anthropic response: {:?}", anthropic_response);
                let (response_text, tool_usages) = self
                    .process_content_response(anthropic_response.content)
                    .await?;

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
                    return self.chat_with_claude(prompt).await;
                }
                error!("Execution failed: {:?}", e);
                Err(e.context("Failed to execute query with tools"))
            }
        }?;
        Ok(response)
    }

    pub fn load_existing_prompt(&mut self, file_path: &str) -> Result<String> {
        let file = match fs::File::open(file_path).context("Failed to open prompt.txt") {
            Ok(file) => file,
            Err(_) => {
                return Ok(String::new());
            }
        };
        let mut contents = String::new();
        io::BufReader::new(file)
            .read_to_string(&mut contents)
            .context("Failed to read contents of prompt.txt")?;
        Ok(contents)
    }

    pub fn load_text_editor(&mut self) -> Result<String> {
        let file_path = "prompt.txt";
        let existing_content = self
            .load_existing_prompt(file_path)
            .context("Failed to load existing prompt")?;
        fs::write(file_path, existing_content).context("Failed to write to text.txt")?;

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
    env_logger::Builder::from_env(Env::default().default_filter_or("info")).init();

    info!("Starting the program");

    let mut claude = Claude::new(MODEL).context("Failed to initialize Claude")?;
    info!("Claude instance initialized with model: {}", MODEL);

    let mut prompt = claude
        .load_text_editor()
        .context("Failed to load text editor")?;
    info!("Text editor loaded successfully");

    let mut iteration = 0;
    loop {
        if iteration > 0 {
            info!(
                r#"
                Starting a new iteration. How would you like to proceed?
                c: Continue from the last response
                e: Exit the program
                n: Input a new prompt
            "#
            );

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            let command = input.trim().to_lowercase();

            match command.as_str() {
                "c" => {
                    info!("Continuing from the last response");
                }
                "e" => {
                    info!("Exiting the program");
                    break;
                }
                "n" => {
                    info!("Inputting a new prompt");
                    prompt = claude
                        .load_text_editor()
                        .context("Failed to load text editor")?;
                }
                _ => {
                    info!("Invalid command. Continuing from the last response");
                    panic!("Invalid command");
                }
            }
        }

        info!("Starting iteration {}", iteration);
        info!("Processing contents: {}", &prompt);

        match claude.chat_with_claude(&prompt).await {
            Ok(response) => {
                info!(
                    "Received response from Claude (iteration {}): {}",
                    iteration, &response
                );
                if response.contains(CONTINUATION_EXIT_PHRASE) {
                    info!("Exit phrase detected. Exiting the loop.");
                    break;
                } else {
                    info!("Continuing to next iteration");
                }
            }
            Err(e) => {
                error!(
                    "Failed to chat with Claude (iteration {}): {:?}",
                    iteration, e
                );
                return Err(e.context("Failed to initiate query with tools"));
            }
        }

        iteration += 1;
    }

    fs::remove_file("prompt.txt")?;
    info!("Program completed successfully");
    Ok(())
}
