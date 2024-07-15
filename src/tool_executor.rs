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
        let mut h = HighlightLines::new(syntax, &ts.themes["base16-monokai.dark"]);

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

        fs::write(path, new_content)?;

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
