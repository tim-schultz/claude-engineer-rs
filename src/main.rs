use anyhow::Result;
use serde_json::{json, Value};
use std::fs;
use std::io::Read;
use std::process::Command;

use anthropic_sdk::Client;

pub struct Claude {
    client: Client,
}

pub const MODEL: &str = "claude-3-5-sonnet-20240620";

impl Claude {
    pub fn new(model: &str) -> Result<Self> {
        let api_key = std::env::var("ANTHROPIC_API_KEY")?;
        let client = Client::new().auth(&api_key).model(model);
        Ok(Self { client })
    }
    pub fn tools(&self) -> Result<Value> {
        let tools = json!([
            {
                "name": "create_folder",
                "description": "Create a new folder at the specified path. Use this when you need to create a new directory in the project structure.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The path where the folder should be created"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "create_file",
                "description": "Create a new file at the specified path with content. Use this when you need to create a new file in the project structure.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The path where the file should be created"
                        },
                        "content": {
                            "type": "string",
                            "description": "The content of the file"
                        }
                    },
                    "required": ["path", "content"]
                }
            },
            {
                "name": "search_file",
                "description": "Search for a specific pattern in a file and return the line numbers where the pattern is found. Use this to locate specific code or text within a file.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The path of the file to search"
                        },
                        "search_pattern": {
                            "type": "string",
                            "description": "The pattern to search for in the file"
                        }
                    },
                    "required": ["path", "search_pattern"]
                }
            },
            {
            "name": "edit_and_apply",
            "description": "Apply changes to a file. Use this when you need to edit a file.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path of the file to edit"
                    },
                    "new_content": {
                        "type": "string",
                        "description": "The new content to apply to the file"
                    }
                },
                "required": ["path", "new_content"]
            }
        },
            {
                "name": "read_file",
                "description": "Read the contents of a file at the specified path. Use this when you need to examine the contents of an existing file.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The path of the file to read"
                        }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "list_files",
                "description": "List all files and directories in the specified folder. Use this when you need to see the contents of a directory.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "path": {
                            "type": "string",
                            "description": "The path of the folder to list (default: current directory)"
                        }
                    }
                }
            },
            {
                "name": "tavily_search",
                "description": "Perform a web search using Tavily API to get up-to-date information or additional context. Use this when you need current information or feel a search could provide a better answer.",
                "input_schema": {
                    "type": "object",
                    "properties": {
                        "query": {
                            "type": "string",
                            "description": "The search query"
                        }
                    },
                    "required": ["query"]
                }
            }
        ]);
        Ok(tools)
    }
    pub async fn initiate_query_with_tools(&self, prompt: &str) -> Result<String> {
        let tools = self.tools()?;

        let message = &serde_json::json!([{"role": "user", "content": prompt}]);
        dbg!(&message);

        let request = self
            .client
            .clone()
            .tools(&tools)
            .max_tokens(3000)
            .messages(message)
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
