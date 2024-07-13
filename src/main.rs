mod prompts;
use prompts::{BASE_SYSTEM_PROMPT, CHAIN_OF_THOUGHT_PROMPT};

mod tools;
use tools::TOOLS;

use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::process::Command;

use anthropic_sdk::Client;

pub struct Claude {
    client: Client,
    system_prompt: String,
}

pub const MODEL: &str = "claude-3-5-sonnet-20240620";
pub const CONTINUATION_EXIT_PHRASE: &str = "AUTOMODE_COMPLETE";
pub const MAX_CONTINUATION_ITERATIONS: i8 = 25;

impl Claude {
    pub fn new(model: &str) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")?;
        let client = Client::new().auth(&api_key).model(model);
        // use update_system_prompt(current_iteration=None, max_iterations=None): to rust when auto mode is enabled
        // until then this system prompt will work
        let system_prompt = format!(
            r#"
            {}
            {}"#,
            BASE_SYSTEM_PROMPT, CHAIN_OF_THOUGHT_PROMPT
        );
        Ok(Self {
            client,
            system_prompt,
        })
    }

    // def update_system_prompt(current_iteration=None, max_iterations=None):
    //     global base_system_prompt, automode_system_prompt
    //     chain_of_thought_prompt = """
    //     Answer the user's request using relevant tools (if they are available). Before calling a tool, do some analysis within <thinking></thinking> tags. First, think about which of the provided tools is the relevant tool to answer the user's request. Second, go through each of the required parameters of the relevant tool and determine if the user has directly provided or given enough information to infer a value. When deciding if the parameter can be inferred, carefully consider all the context to see if it supports a specific value. If all of the required parameters are present or can be reasonably inferred, close the thinking tag and proceed with the tool call. BUT, if one of the values for a required parameter is missing, DO NOT invoke the function (not even with fillers for the missing params) and instead, ask the user to provide the missing parameters. DO NOT ask for more information on optional parameters if it is not provided.

    //     Do not reflect on the quality of the returned search results in your response.
    //     """
    //     if automode:
    //         iteration_info = ""
    //         if current_iteration is not None and max_iterations is not None:
    //             iteration_info = f"You are currently on iteration {current_iteration} out of {max_iterations} in automode."
    //         return base_system_prompt + "\n\n" + automode_system_prompt.format(iteration_info=iteration_info) + "\n\n" + chain_of_thought_prompt
    //     else:
    //         return base_system_prompt + "\n\n" + chain_of_thought_prompt
    // pub async fn update_system_prompt(&self) -> Result<()> {
    //     let message = &serde_json::json!([{"role": "system", "content": prompt}]);
    //     dbg!(&message);

    //     let request = self
    //         .client
    //         .clone()
    //         .tools(&TOOLS)
    //         .max_tokens(3000)
    //         .messages(message)
    //         .build()?;

    //     let mut response = String::new();
    //     request
    //         .execute(|text| {
    //             response.push_str(&text);
    //             async move {}
    //         })
    //         .await?;

    //     Ok(())
    // }
    pub async fn initiate_query_with_tools(&self, prompt: &str) -> Result<String> {
        let message = &serde_json::json!([{"role": "user", "content": prompt}]);
        dbg!(&message);

        let request = self
            .client
            .clone()
            .tools(&TOOLS)
            .max_tokens(3000)
            .messages(message)
            .system(&self.system_prompt)
            .build()?;

        let mut response = String::new();
        request
            .execute(|text| {
                response.push_str(&text);
                async move {}
            })
            .await?;

        Ok(response)
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Create a new text file
    let file_path = "text.txt";
    fs::write(file_path, "")?;

    // Attempt to open the file with an editor
    let formatted_path = format!("./{}", file_path);
    println!("Attempting to open file: {}", formatted_path);

    let editors = ["vim"];

    for editor in editors.iter() {
        match Command::new(editor).arg(&formatted_path).status() {
            Ok(status) => {
                println!("{} editor exited with status: {}", editor, status);
                break; // Exit the loop if an editor succeeds
            }
            Err(e) => {
                eprintln!("Failed to open {} editor: {}", editor, e);
                if editor == editors.last().unwrap() {
                    println!("No suitable editor found. Skipping edit step.");
                }
            }
        }
    }

    // Read the contents of the file after it's closed
    let mut file = fs::File::open(file_path)?;
    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    // Print the contents to the console
    println!("File contents:");
    println!("{}", contents);

    let claude = Claude::new(MODEL)?;

    let response = claude.initiate_query_with_tools(&contents).await?;

    println!("Response: {}", response);

    Ok(())
}
