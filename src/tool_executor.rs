use anyhow::{anyhow, Result};
use serde_json::Value;
use similar::{ChangeTag, TextDiff};
use std::fs;
use std::io::{self, Read, Write};
use std::path::Path;
use syntect::easy::HighlightLines;
use syntect::highlighting::{Style, ThemeSet};
use syntect::parsing::SyntaxSet;
use syntect::util::{as_24_bit_terminal_escaped, LinesWithEndings};

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
            // "tavily_search" => {
            //     self.tavily_search(
            //         tool_input["query"]
            //             .as_str()
            //             .ok_or(anyhow!("Missing query"))?,
            //     )
            //     .await
            // }
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

    // async fn tavily_search(&self, query: &str) -> Result<String> {
    //     let response = self.tavily.answer(query).await?;
    //     Ok(serde_json::to_string_pretty(&response)?)
    // }
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
