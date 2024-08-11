use anthropic_sdk::Client;
use anthropic_sdk::ContentItem;
use anyhow::{anyhow, Result};
use async_recursion::async_recursion;
use console::Term;
use diff;
use log::{debug, error, info, trace, warn};
use regex::escape;
use regex::{Regex, RegexBuilder};
use serde::Deserialize;
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use std::collections::HashMap;
use std::collections::HashSet;
use std::fs;
use std::io;
use std::time::Instant;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

use crate::conversation_manager::ConversationManager;
use crate::conversation_manager::Message;
use crate::github_tools;
use crate::MessageContent;

use once_cell::sync::Lazy;

pub static CODEEDITORMODEL: &str = "claude-3-5-sonnet-20240620";

pub static TOOLS: Lazy<Value> = Lazy::new(|| {
    json!([
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
            "description": "Apply AI-powered improvements to a file based on specific instructions and detailed project context. This function reads the file, processes it in batches using AI with conversation history and comprehensive code-related project context. It generates a diff and allows the user to confirm changes before applying them. The goal is to maintain consistency and prevent breaking connections between files. This tool should be used for complex code modifications that require understanding of the broader project context.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The absolute or relative path of the file to edit. Use forward slashes (/) for path separation, even on Windows systems."
                    },
                    "instructions": {
                        "type": "string",
                        "description": "After completing the code review, construct a plan for the change between <PLANNING> tags. Ask for additional source files or documentation that may be relevant. The plan should avoid duplication (DRY principle), and balance maintenance and flexibility. Present trade-offs and implementation choices at this step. Consider available Frameworks and Libraries and suggest their use when relevant. STOP at this step if we have not agreed a plan.\n\nOnce agreed, produce code between <OUTPUT> tags. Pay attention to Variable Names, Identifiers and String Literals, and check that they are reproduced accurately from the original source files unless otherwise directed. When naming by convention surround in double colons and in ::UPPERCASE::. Maintain existing code style, use language appropriate idioms. Produce Code Blocks with the language specified after the first backticks"
                    },
                    "project_context": {
                        "type": "string",
                        "description": "Comprehensive context about the project, including recent changes, new variables or functions, interconnections between files, coding standards, and any other relevant information that might affect the edit."
                    }
                },
                "required": ["path", "instructions", "project_context"]
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
            "name": "read_multiple_files",
            "description": "Read the contents of multiple files at the specified paths. This tool should be used when you need to examine the contents of multiple existing files at once. It will return the status of reading each file, and store the contents of successfully read files in the system prompt. If a file doesn't exist or can't be read, an appropriate error message will be returned for that file.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "An array of absolute or relative paths of the files to read. Use forward slashes (/) for path separation, even on Windows systems."
                    }
                },
                "required": ["paths"]
            }
        },
        {
            "name": "fetch_commit_changes",
            "description": "Fetch the the given commit's changes from a GitHub repository. Use this when you need to see the changes made in an external repository.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "The owner of the repository"
                    },
                    "repo": {
                        "type": "string",
                        "description": "The name of the repository"
                    },
                    "sha": {
                        "type": "string",
                        "description": "The SHA of the commit to fetch"
                    }
                },
                "required": ["owner", "repo", "sha"]
            }
        }
    ])
});

pub struct ToolExecutor {
    client: Client,
    code_editor_tokens: HashMap<String, u32>,
    code_editor_memory: Vec<String>,
    code_editor_files: HashSet<String>,
    conversation_manager: ConversationManager,
}

#[derive(Debug, Deserialize)]
pub struct EditInstruction {
    pub search: String,
    pub replace: String,
}

impl ToolExecutor {
    pub fn new(client: Client) -> Result<Self> {
        let conversation_manager = ConversationManager::new(1000);
        Ok(Self {
            client,
            code_editor_tokens: HashMap::new(),
            code_editor_memory: Vec::new(),
            code_editor_files: HashSet::new(),
            conversation_manager,
        })
    }

    pub async fn execute_tool(&mut self, tool_name: &str, tool_input: &Value) -> Result<String> {
        match tool_name {
            "create_folder" => {
                self.create_folder(tool_input["path"].as_str().ok_or(anyhow!("Missing path"))?)
            }
            "create_file" => self.create_file(
                tool_input["path"].as_str().ok_or(anyhow!("Missing path"))?,
                tool_input
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or(""),
            ),
            "edit_and_apply" => {
                self.edit_and_apply(
                    tool_input["path"].as_str().ok_or(anyhow!("Missing path"))?,
                    tool_input
                        .get("instructions")
                        .and_then(|c| c.as_str())
                        .ok_or(anyhow!("Missing new_content"))?,
                    tool_input["project_context"]
                        .as_str()
                        .ok_or(anyhow!("Missing project_context"))?,
                )
                .await
            }
            "read_file" => {
                self.read_file(tool_input["path"].as_str().ok_or(anyhow!("Missing path"))?)
            }
            "list_files" => self.list_files(
                tool_input
                    .get("path")
                    .and_then(|p| p.as_str())
                    .unwrap_or("."),
            ),
            "fetch_commit_changes" => {
                self.fetch_commit_changes(
                    tool_input["owner"]
                        .as_str()
                        .ok_or(anyhow!("Missing owner"))?,
                    tool_input["repo"].as_str().ok_or(anyhow!("Missing repo"))?,
                    tool_input["sha"].as_str().ok_or(anyhow!("Missing sha"))?,
                )
                .await
            }
            _ => Err(anyhow!("Unknown tool: {}", tool_name)),
        }
    }

    fn create_folder(&self, path: &str) -> Result<String> {
        fs::create_dir_all(path)?;
        Ok(format!("Folder created: {}", path))
    }

    fn create_file(&self, path: &str, content: &str) -> Result<String> {
        fs::write(path, content)?;
        Ok(format!("File created: {}", path))
    }

    fn highlight_diff(&self, diff_text: &str) -> String {
        let ps = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        let syntax = ps.find_syntax_by_extension("diff").unwrap();
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-ocean.dark"]);

        let mut highlighted = String::new();
        for line in LinesWithEndings::from(diff_text) {
            let ranges: Vec<(Style, &str)> = h.highlight(line, &ps);
            let escaped = as_24_bit_terminal_escaped(&ranges[..], true);
            highlighted.push_str(&escaped);
        }
        highlighted
    }

    fn generate_and_apply_diff(
        &self,
        original_content: &str,
        new_content: &str,
        path: &str,
    ) -> Result<String> {
        let diff = TextDiff::from_lines(original_content, new_content);

        if diff.ratio() == 1.0 {
            return Ok("No changes detected.".to_string());
        }

        let mut diff_text = String::new();
        for change in diff.iter_all_changes() {
            let sign = match change.tag() {
                ChangeTag::Delete => "-",
                ChangeTag::Insert => "+",
                ChangeTag::Equal => " ",
            };
            diff_text.push_str(&format!("{}{}", sign, change));
        }

        let highlighted_diff = self.highlight_diff(&diff_text);
        println!("Changes in {}:\n{}", path, highlighted_diff);

        println!("Do you want to apply these changes? (y/n)");
        let mut input = String::new();
        io::stdin().read_line(&mut input)?;

        if input.trim().to_lowercase() == "y" {
            fs::write(path, new_content)?;

            let added_lines = diff
                .iter_all_changes()
                .filter(|c| c.tag() == ChangeTag::Insert)
                .count();
            let removed_lines = diff
                .iter_all_changes()
                .filter(|c| c.tag() == ChangeTag::Delete)
                .count();

            Ok(format!(
                "Changes applied to {}:\n  Lines added: {}\n  Lines removed: {}",
                path, added_lines, removed_lines
            ))
        } else {
            Ok("Changes were not applied.".to_string())
        }
    }

    async fn parse_search_replace_blocks(&self, text: &str) -> Result<String> {
        let re =
            Regex::new(r"<SEARCH>\s*([\s\S]*?)\s*</SEARCH>\s*<REPLACE>\s*([\s\S]*?)\s*</REPLACE>")?;
        let blocks: Vec<_> = re
            .captures_iter(text)
            .map(|cap| {
                json!({
                    "search": cap.get(1).unwrap().as_str().trim(),
                    "replace": cap.get(2).unwrap().as_str().trim()
                })
            })
            .collect();
        Ok(serde_json::to_string(&blocks)?)
    }

    pub async fn generate_edit_instructions(
        &mut self,
        file_path: &str,
        file_content: &str,
        instructions: &str,
        project_context: &str,
        full_file_contents: &HashMap<String, String>,
    ) -> Result<String> {
        let memory_context = self
            .code_editor_memory
            .iter()
            .enumerate()
            .map(|(i, mem)| format!("Memory {}:\n{:?}", i + 1, mem))
            .collect::<Vec<_>>()
            .join("\n");

        let full_file_contents_context = full_file_contents
            .iter()
            .filter(|&(path, _)| path != file_path || !self.code_editor_files.contains(path))
            .map(|(path, content)| format!("--- {} ---\n{}", path, content))
            .collect::<Vec<_>>()
            .join("\n\n");

        let system_prompt = format!(
            r#"
            You are an AI coding agent that generates edit instructions for code files. Your task is to analyze the provided code and generate SEARCH/REPLACE blocks for necessary changes. Follow these steps:
    
            1. Review the entire file content to understand the context:
            {file_content}
    
            2. Carefully analyze the specific instructions:
            {instructions}
    
            3. Take into account the overall project context:
            {project_context}
    
            4. Consider the memory of previous edits:
            {memory_context}
    
            5. Consider the full context of all files in the project:
            {full_file_contents_context}
    
            6. Generate SEARCH/REPLACE blocks for each necessary change. Each block should:
               - Include enough context to uniquely identify the code to be changed
               - Provide the exact replacement code, maintaining correct indentation and formatting
               - Focus on specific, targeted changes rather than large, sweeping modifications
    
            7. Ensure that your SEARCH/REPLACE blocks:
               - Address all relevant aspects of the instructions
               - Maintain or enhance code readability and efficiency
               - Consider the overall structure and purpose of the code
               - Follow best practices and coding standards for the language
               - Maintain consistency with the project context and previous edits
               - Take into account the full context of all files in the project
    
            IMPORTANT: RETURN ONLY THE SEARCH/REPLACE BLOCKS. NO EXPLANATIONS OR COMMENTS.
            USE THE FOLLOWING FORMAT FOR EACH BLOCK:
    
            <SEARCH>
            Code to be replaced
            </SEARCH>
            <REPLACE>
            New code to insert
            </REPLACE>
    
            If no changes are needed, return an empty list.
            "#
        );

        info!("Sending edit instructions: {}", system_prompt);

        let request = self
            .client
            .clone()
            .system(&system_prompt)
            .messages(&json!([{"role": "user", "content": "Generate SEARCH/REPLACE blocks for the necessary changes."}]))
            .build()?;

        self.conversation_manager.add_to_current(Message {
            role: "assistant".to_string(),
            content: MessageContent::Text(system_prompt),
        });

        let response = request.execute_and_return_json().await?;

        self.code_editor_tokens
            .entry("input".to_string())
            .and_modify(|e| *e += response.usage.input_tokens)
            .or_insert(response.usage.input_tokens);
        self.code_editor_tokens
            .entry("output".to_string())
            .and_modify(|e| *e += response.usage.output_tokens)
            .or_insert(response.usage.output_tokens);
        let text = match &response.content[0] {
            ContentItem::Text { text } => text,
            _ => return Err(anyhow!("Invalid response content")),
        };

        info!("Received edit instructions: {}", text);

        let edit_instructions = self.parse_search_replace_blocks(&text).await?;

        self.code_editor_memory.push(format!(
            "Edit Instructions for {}:\n{}",
            file_path,
            text.clone()
        ));
        self.code_editor_files.insert(file_path.to_string());

        Ok(edit_instructions)
    }

    // async fn apply_edits(
    //     &self,
    //     path: &str,
    //     edit_instructions: Vec<Value>,
    //     original_content: &str,
    // ) -> Result<(String, bool, String)> {
    //     // Implement this function based on your requirements
    //     // For now, it returns a placeholder
    //     Ok((original_content.to_string(), false, String::new()))
    // }

    #[async_recursion]
    pub async fn edit_and_apply(
        &mut self,
        path: &str,
        instructions: &str,
        project_context: &str,
    ) -> Result<String> {
        let max_retries = 1;

        let mut file_contents: HashMap<String, String> = HashMap::new();
        let original_content = match file_contents.get(path) {
            Some(content) => content.clone(),
            None => {
                let content = fs::read_to_string(path)?;
                file_contents.insert(path.to_string(), content.clone());
                content
            }
        };

        for attempt in 0..max_retries {
            let edit_instructions_json = self
                .generate_edit_instructions(
                    path,
                    &original_content,
                    instructions,
                    project_context,
                    &file_contents,
                )
                .await?;

            let edit_instructions: Vec<EditInstruction> =
                serde_json::from_str(&edit_instructions_json)
                    .map_err(|e| anyhow::anyhow!("Failed to parse edit instructions: {}", e))?;
            println!(
                "{}",
                format!(
                    "Attempt {}/{}: The following SEARCH/REPLACE blocks have been generated:",
                    attempt + 1,
                    max_retries
                )
            );

            for (i, block) in edit_instructions.iter().enumerate() {
                println!("Block {}:", i + 1);
                println!(
                    "{}",
                    format!("SEARCH:\n{}\n\nREPLACE:\n{}", block.search, block.replace)
                );
            }

            let (edited_content, changes_made, failed_edits) = self
                .apply_edits(path, edit_instructions, &original_content)
                .await?;

            if changes_made {
                file_contents.insert(path.to_string(), edited_content.clone());
                println!(
                    "{}",
                    format!("File contents updated in system prompt: {}", path)
                );

                if !failed_edits.is_empty() {
                    println!("{}", "Some edits could not be applied. Retrying...");
                    let new_instructions = format!(
                        "{}\n\nPlease retry the following edits that could not be applied:\n{}",
                        instructions, failed_edits
                    );
                    return self
                        .edit_and_apply(path, &new_instructions, project_context)
                        .await;
                }

                return Ok(format!("Changes applied to {}", path));
            } else if attempt == max_retries - 1 {
                return Ok(format!("No changes could be applied to {} after {} attempts. Please review the edit instructions and try again.", path, max_retries));
            } else {
                println!(
                    "{}",
                    format!(
                        "No changes could be applied in attempt {}. Retrying...",
                        attempt + 1
                    )
                );
            }
        }

        Ok(format!(
            "Failed to apply changes to {} after {} attempts.",
            path, max_retries
        ))
    }

    pub async fn apply_edits(
        &self,
        file_path: &str,
        edit_instructions: Vec<EditInstruction>,
        original_content: &str,
    ) -> Result<(String, bool, String)> {
        let mut changes_made = false;
        let mut original_content_lines: Vec<String> =
            original_content.lines().map(String::from).collect();
        let mut edited_lines: Vec<String> = original_content_lines.clone();
        let total_edits = edit_instructions.len();
        let mut failed_edits = Vec::new();

        let term = Term::stdout();

        for (i, edit) in edit_instructions.iter().enumerate() {
            let search_lines: Vec<String> = edit
                .search
                .lines()
                .map(|l| self.normalize_whitespace(l))
                .collect();

            let replace_lines: Vec<String> = edit.replace.lines().map(String::from).collect();

            let mut edit_applied = false;

            'outer: for start_index in 0..edited_lines.len() {
                if edited_lines.len() - start_index < search_lines.len() {
                    break;
                }

                let mut match_found = true;
                for (j, search_line) in search_lines.iter().enumerate() {
                    let normalized_edited_line =
                        self.normalize_whitespace(&edited_lines[start_index + j]);
                    if normalized_edited_line != *search_line {
                        match_found = false;
                        break;
                    }
                }

                if match_found {
                    let end_index = start_index + search_lines.len() - 1;
                    let _ = edited_lines
                        .splice(start_index..=end_index, replace_lines)
                        .collect::<Vec<String>>();

                    let edited_file = edited_lines.join("\n");

                    self.generate_and_apply_diff(
                        &original_content_lines.join("\n"),
                        &edited_file,
                        file_path,
                    )?;

                    original_content_lines = fs::read_to_string(file_path)?
                        .lines()
                        .map(String::from)
                        .collect();

                    changes_made = true;
                    edit_applied = true;
                    break 'outer;
                }
            }

            if edit_applied {
                term.write_line(&format!(
                    "Changes applied in {} ({}/{})",
                    file_path,
                    i + 1,
                    total_edits
                ))?;
            } else {
                term.write_line(&format!(
                    "Edit {}/{} not applied: content not found",
                    i + 1,
                    total_edits
                ))?;
                failed_edits.push(format!("Edit {}: {}", i + 1, edit.search));
            }
        }

        let edited_content = edited_lines.join("\n");

        if !changes_made {
            term.write_line(
                "No changes were applied. The file content already matches the desired state.",
            )?;
        } else {
            fs::write(file_path, &edited_content)?;
            term.write_line(&format!("Changes have been written to {}", file_path))?;
        }

        Ok((edited_content, changes_made, failed_edits.join("\n")))
    }

    fn normalize_whitespace(&self, s: &str) -> String {
        s.split_whitespace().collect::<Vec<&str>>().join(" ")
    }

    fn generate_diff(&self, old: &str, new: &str, file_path: &str) -> Result<String> {
        info!("Generating diff for file: {}", file_path);
        let mut diff_output = String::new();

        for diff_result in diff::lines(old, new) {
            match diff_result {
                diff::Result::Left(l) => diff_output.push_str(&format!("-{}\n", l)),
                diff::Result::Both(l, _) => diff_output.push_str(&format!(" {}\n", l)),
                diff::Result::Right(r) => diff_output.push_str(&format!("+{}\n", r)),
            }
        }

        info!(
            "Generated diff for {} with size {} bytes",
            file_path,
            diff_output.len()
        );
        Ok(diff_output)
    }

    fn read_file(&self, path: &str) -> Result<String> {
        fs::read_to_string(path).map_err(|e| anyhow!("Error reading file: {}", e))
    }

    fn list_files(&self, path: &str) -> Result<String> {
        info!("Listing files in directory: {}", path);
        let entries = fs::read_dir(path).map_err(|e| {
            error!("Failed to read directory {}: {}", path, e);
            e
        })?;

        let files: Result<Vec<_>, io::Error> = entries
            .map(|entry| {
                entry.map(|e| {
                    let file_name = e.file_name().into_string().unwrap();
                    trace!("Found file: {}", file_name);
                    file_name
                })
            })
            .collect();

        let file_list = files.map_err(|e| {
            error!("Error collecting file names: {}", e);
            e
        })?;

        let result = file_list.join("\n");
        info!("Listed {} files in directory {}", file_list.len(), path);
        Ok(result)
    }

    async fn fetch_commit_changes(&self, owner: &str, repo: &str, sha: &str) -> Result<String> {
        info!(
            "Fetching commit changes for {}/{} with SHA: {}",
            owner, repo, sha
        );
        match github_tools::fetch_latest_commits(owner, repo, sha).await {
            Ok(commit) => {
                info!("Successfully fetched commit for {}/{}", owner, repo);
                match github_tools::process_commit_changes(commit) {
                    Ok(changes) => {
                        info!("Successfully processed commit changes");
                        Ok(changes)
                    }
                    Err(e) => {
                        error!("Failed to process commit changes: {}", e);
                        Err(e.into())
                    }
                }
            }
            Err(e) => {
                error!("Failed to fetch commit for {}/{}: {}", owner, repo, e);
                Err(e.into())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{env, path::PathBuf};

    use super::*;
    use tempfile::tempdir;
    use tokio;

    #[tokio::test]
    async fn test_apply_edits() {
        let current_file = file!();
        let current_path = PathBuf::from(env::current_dir().unwrap()).join(current_file);
        let current_path_str = current_path.to_str().unwrap();

        let client = Client::new();
        let executor = ToolExecutor::new(client).unwrap();
        let original_content = fs::read_to_string(current_path_str).unwrap();
        let edit_result = executor
            .apply_edits(
                current_path_str,
                vec![EditInstruction {
                    search: r#"fn list_files(&self, path: &str) -> Result<String> {
        dbg!(&path);
        let entries = fs::read_dir(path)?;
        let files: Result<Vec<_>, io::Error> = entries
            .map(|entry| entry.map(|e| e.file_name().into_string().unwrap()))
            .collect();
        Ok(files?.join("\n"))
    }"#
                    .to_string(),
                    replace: r#"REPLACEDDDDD
        "#
                    .to_string(),
                }],
                &original_content,
            )
            .await
            .unwrap();

        dbg!(&edit_result);
    }

    #[test]
    fn test_create_folder() {
        let client = Client::new();
        let executor = ToolExecutor::new(client).unwrap();
        let temp_dir = tempdir().unwrap();
        let folder_path = temp_dir.path().join("test_folder");

        let result = executor
            .create_folder(folder_path.to_str().unwrap())
            .unwrap();
        assert_eq!(
            result,
            format!("Folder created: {}", folder_path.to_str().unwrap())
        );
        assert!(folder_path.exists());
    }

    #[test]
    fn test_create_file() {
        let client = Client::new();
        let executor = ToolExecutor::new(client).unwrap();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_file.txt");
        let content = "Hello, world!";

        let result = executor
            .create_file(file_path.to_str().unwrap(), content)
            .unwrap();
        assert_eq!(
            result,
            format!("File created: {}", file_path.to_str().unwrap())
        );
        assert!(file_path.exists());
        assert_eq!(fs::read_to_string(file_path).unwrap(), content);
    }

    #[test]
    fn test_read_file() {
        let client = Client::new();
        let executor = ToolExecutor::new(client).unwrap();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_read.txt");
        let content = "Test content";
        fs::write(&file_path, content).unwrap();

        let result = executor.read_file(file_path.to_str().unwrap()).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn test_list_files() {
        let client = Client::new();
        let executor = ToolExecutor::new(client).unwrap();
        let temp_dir = tempdir().unwrap();
        fs::write(temp_dir.path().join("file1.txt"), "").unwrap();
        fs::write(temp_dir.path().join("file2.txt"), "").unwrap();

        let result = executor
            .list_files(temp_dir.path().to_str().unwrap())
            .unwrap();
        let files: Vec<&str> = result.split('\n').collect();
        assert_eq!(files.len(), 2);
        assert!(files.contains(&"file1.txt"));
        assert!(files.contains(&"file2.txt"));
    }

    // Note: We can't easily test edit_and_apply in a unit test due to its interactive nature
    // A more comprehensive integration test or mocking the user input would be needed for that
}
