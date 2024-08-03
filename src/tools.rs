use anyhow::{anyhow, Result};
use serde_json::{json, Value};
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::io;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

use crate::github_tools;

use once_cell::sync::Lazy;

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
    version: i8,
}

impl ToolExecutor {
    pub fn new() -> Result<Self> {
        Ok(Self { version: 0 })
    }

    pub async fn execute_tool(&self, tool_name: &str, tool_input: &Value) -> Result<String> {
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
            "edit_and_apply" => self.edit_and_apply(
                tool_input["path"].as_str().ok_or(anyhow!("Missing path"))?,
                tool_input
                    .get("new_content")
                    .and_then(|c| c.as_str())
                    .ok_or(anyhow!("Missing new_content"))?,
            ),
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

    fn edit_and_apply(&self, path: &str, new_content: &str) -> Result<String> {
        let original_content = fs::read_to_string(path)?;
        if new_content != original_content {
            self.generate_and_apply_diff(&original_content, new_content, path)
        } else {
            Ok(format!("No changes needed for {}", path))
        }
    }

    fn read_file(&self, path: &str) -> Result<String> {
        fs::read_to_string(path).map_err(|e| anyhow!("Error reading file: {}", e))
    }

    fn list_files(&self, path: &str) -> Result<String> {
        dbg!(&path);
        let entries = fs::read_dir(path)?;
        let files: Result<Vec<_>, io::Error> = entries
            .map(|entry| entry.map(|e| e.file_name().into_string().unwrap()))
            .collect();
        Ok(files?.join("\n"))
    }

    async fn fetch_commit_changes(&self, owner: &str, repo: &str, sha: &str) -> Result<String> {
        let commit = github_tools::fetch_latest_commits(owner, repo, sha).await?;
        let changes = github_tools::process_commit_changes(commit)?;
        Ok(changes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_create_folder() {
        let executor = ToolExecutor::new().unwrap();
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
        let executor = ToolExecutor::new().unwrap();
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
        let executor = ToolExecutor::new().unwrap();
        let temp_dir = tempdir().unwrap();
        let file_path = temp_dir.path().join("test_read.txt");
        let content = "Test content";
        fs::write(&file_path, content).unwrap();

        let result = executor.read_file(file_path.to_str().unwrap()).unwrap();
        assert_eq!(result, content);
    }

    #[test]
    fn test_list_files() {
        let executor = ToolExecutor::new().unwrap();
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
