use chrono::Local;
use log::{debug, info, trace, warn};
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::fs::File;
use std::io::Write;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: MessageContent,
}

#[derive(Debug, Clone)]
pub struct ConversationManager {
    history: VecDeque<Message>,
    current: Vec<Message>,
    max_history_size: usize,
}

impl ConversationManager {
    pub fn new(max_history_size: usize) -> Self {
        info!(
            "Creating new ConversationManager with max_history_size: {}",
            max_history_size
        );
        Self {
            history: VecDeque::new(),
            current: Vec::new(),
            max_history_size,
        }
    }

    pub fn add_to_history(&mut self, message: Message) {
        if self.history.len() >= self.max_history_size {
            let removed = self.history.pop_front();
            info!("Removed oldest message from history: {:?}", removed);
        }
        info!("Adding message to history: {:?}", message);
        self.history.push_back(message);
    }

    pub fn add_to_current(&mut self, message: Message) {
        info!("Adding message to current conversation: {:?}", message);
        self.current.push(message);
    }

    pub fn clear_current(&mut self) {
        info!("Clearing current conversation");
        self.current.clear();
    }

    pub fn get_combined_conversation(&self) -> Vec<Message> {
        trace!("Getting combined conversation");
        let mut combined = self.history.clone().into_iter().collect::<Vec<Message>>();
        combined.extend(self.current.clone());
        info!("Combined conversation size: {}", combined.len());
        combined
    }

    pub fn commit_current_to_history(&mut self) {
        info!("Committing current conversation to history");
        for message in self.current.clone().drain(..) {
            self.add_to_history(message);
        }
        info!("Current conversation cleared after commit");
    }

    pub fn save_chat(&self) -> std::io::Result<String> {
        info!("Saving chat to file");
        // Generate filename
        let now = Local::now();
        let filename = format!("Chat_{}.md", now.format("%H%M"));
        info!("Generated filename: {}", filename);

        // Format conversation history
        let mut formatted_chat = String::from("# Claude-3-Sonnet Engineer Chat Log\n\n");
        for message in self.get_combined_conversation() {
            match message.role.as_str() {
                "user" => {
                    formatted_chat.push_str("## User\n\n");
                    match message.content {
                        MessageContent::Text(text) => {
                            formatted_chat.push_str(&format!("{}\n\n", text))
                        }
                        MessageContent::ToolUseUser(result) => {
                            for tool_use in result {
                                formatted_chat.push_str(&format!(
                                    "### Tool Use: {}\n\n```json\n{}\n```\n\n",
                                    tool_use.tool_type, tool_use.content
                                ))
                            }
                        }
                        _ => {}
                    }
                }
                "assistant" => {
                    formatted_chat.push_str("## Claude\n\n");
                    match message.content {
                        MessageContent::Text(text) => {
                            formatted_chat.push_str(&format!("{}\n\n", text))
                        }
                        MessageContent::ToolUseAssistant(assistant_tool_uses) => {
                            for tool_use in assistant_tool_uses {
                                formatted_chat.push_str(&format!(
                                    "### Tool Use: {}\n\n```json\n{}\n```\n\n",
                                    tool_use.name, tool_use.input
                                ))
                            }
                        }
                        _ => {}
                    }
                }
                _ => {
                    warn!("Unknown message role: {}", message.role);
                }
            }
        }

        // Save to file
        let mut file = File::create(&filename)?;
        file.write_all(formatted_chat.as_bytes())?;
        info!("Chat saved to file: {}", filename);

        Ok(filename)
    }
}

use super::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_conversation_manager() {
        let cm = ConversationManager::new(5);
        assert_eq!(cm.history.len(), 0);
        assert_eq!(cm.current.len(), 0);
        assert_eq!(cm.max_history_size, 5);
    }
}

#[test]
fn test_add_to_history() {
    let mut cm = ConversationManager::new(3);
    let message1 = Message {
        role: "user".to_string(),
        content: MessageContent::Text("Hello".to_string()),
    };
    let message2 = Message {
        role: "assistant".to_string(),
        content: MessageContent::Text("Hi there".to_string()),
    };
    let message3 = Message {
        role: "user".to_string(),
        content: MessageContent::Text("How are you?".to_string()),
    };
    let message4 = Message {
        role: "assistant".to_string(),
        content: MessageContent::Text("I'm doing well, thanks!".to_string()),
    };

    cm.add_to_history(message1.clone());
    cm.add_to_history(message2.clone());
    cm.add_to_history(message3.clone());
    assert_eq!(cm.history.len(), 3);

    cm.add_to_history(message4.clone());
    assert_eq!(cm.history.len(), 3);
    assert!(matches!(cm.history[0].content, MessageContent::Text(ref s) if s == "Hi there"));
    assert!(matches!(cm.history[1].content, MessageContent::Text(ref s) if s == "How are you?"));
    assert!(
        matches!(cm.history[2].content, MessageContent::Text(ref s) if s == "I'm doing well, thanks!")
    );
}

#[test]
fn test_add_to_current() {
    let mut cm = ConversationManager::new(5);
    let message = Message {
        role: "user".to_string(),
        content: MessageContent::Text("Hello".to_string()),
    };
    cm.add_to_current(message.clone());
    assert_eq!(cm.current.len(), 1);
    assert!(matches!(cm.current[0].content, MessageContent::Text(ref s) if s == "Hello"));
}

#[test]
fn test_clear_current() {
    let mut cm = ConversationManager::new(5);
    let message = Message {
        role: "user".to_string(),
        content: MessageContent::Text("Hello".to_string()),
    };
    cm.add_to_current(message);
    assert_eq!(cm.current.len(), 1);
    cm.clear_current();
    assert_eq!(cm.current.len(), 0);
}

#[test]
fn test_get_combined_conversation() {
    let mut cm = ConversationManager::new(5);
    let history_message = Message {
        role: "user".to_string(),
        content: MessageContent::Text("Past message".to_string()),
    };
    let current_message = Message {
        role: "assistant".to_string(),
        content: MessageContent::Text("Current message".to_string()),
    };
    cm.add_to_history(history_message.clone());
    cm.add_to_current(current_message.clone());

    let combined = cm.get_combined_conversation();
    assert_eq!(combined.len(), 2);
    assert!(matches!(combined[0].content, MessageContent::Text(ref s) if s == "Past message"));
    assert!(matches!(combined[1].content, MessageContent::Text(ref s) if s == "Current message"));
}

#[test]
fn test_commit_current_to_history() {
    let mut cm = ConversationManager::new(5);
    let mut cm = ConversationManager::new(5);
    let message1 = Message {
        role: "user".to_string(),
        content: MessageContent::Text("Hello".to_string()),
    };
    let message2 = Message {
        role: "assistant".to_string(),
        content: MessageContent::Text("Hi there".to_string()),
    };
    cm.add_to_current(message1.clone());
    cm.add_to_current(message2.clone());
    assert_eq!(cm.current.len(), 2);
    assert_eq!(cm.history.len(), 0);

    cm.commit_current_to_history();
    assert_eq!(cm.current.len(), 0);
    assert_eq!(cm.history.len(), 2);
    assert!(matches!(cm.history[0].content, MessageContent::Text(ref s) if s == "Hello"));
    assert!(matches!(cm.history[1].content, MessageContent::Text(ref s) if s == "Hi there"));
}

#[test]
fn test_save_chat() {
    let mut cm = ConversationManager::new(5);
    cm.add_to_current(Message {
        role: "user".to_string(),
        content: MessageContent::Text("Hello, Claude!".to_string()),
    });
    cm.add_to_current(Message {
        role: "assistant".to_string(),
        content: MessageContent::Text("Hello! How can I assist you today?".to_string()),
    });

    let result = cm.save_chat();
    assert!(result.is_ok());
    let filename = result.unwrap();
    assert!(filename.starts_with("Chat_") && filename.ends_with(".md"));

    // You might want to add more assertions here to check the content of the file,
    // but that would require reading the file back, which is beyond the scope of this test.
}
