//! This module defines a static array of tool configurations used throughout the project.
//! It provides a centralized location for defining and managing various file and project
//! management tools, each represented as a JSON object with properties such as name,
//! description, and input schema.
//!
//! The tools defined here are used for operations like creating folders and files,
//! searching within files, applying AI-powered edits, reading file contents, listing
//! directory contents, and fetching commit changes from GitHub repositories.
//!
//! This centralized approach allows for easy addition, modification, and maintenance
//! of tool configurations, ensuring consistency across the project.

use once_cell::sync::Lazy;
use serde_json::json;
use serde_json::Value;

/// A static array of tool configurations, lazily initialized using `once_cell::sync::Lazy`.
/// 
/// The use of `Lazy` allows for efficient, thread-safe initialization of the static variable.
/// The tool configurations are only computed when first accessed, reducing startup time
/// and memory usage if the tools are not immediately needed.
///
/// The `serde_json::json!` macro is used to create the JSON structure inline,
/// providing a convenient and readable way to define the complex nested structure
/// of the tool configurations.
pub static TOOLS: Lazy<Value> = Lazy::new(|| {
    // Initialize the tools array with JSON representations of each tool
    json!([
        {
            "name": "create_folder",
            "description": "Create a new folder at the specified path. Use this when you need to create a new directory in the project structure. For example, use this to create subdirectories for organizing your project files, such as 'src/', 'tests/', or 'docs/'.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path where the folder should be created, e.g., 'src/components' or 'tests/integration'"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "create_file",
            "description": "Create a new file at the specified path with content. Use this when you need to create a new file in the project structure. This is useful for initializing new source files, configuration files, or documentation files.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path where the file should be created, e.g., 'src/main.rs' or 'config/settings.json'"
                    },
                    "content": {
                        "type": "string",
                        "description": "The initial content of the file, which can be empty or contain starter code"
                    }
                },
                "required": ["path", "content"]
            }
        },
        {
            "name": "search_file",
            "description": "Search for a specific pattern in a file and return the line numbers where the pattern is found. Use this to locate specific code or text within a file. This is particularly helpful for finding function definitions, variable declarations, or specific comments in large files.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path of the file to search, e.g., 'src/lib.rs' or 'tests/unit_tests.rs'"
                    },
                    "search_pattern": {
                        "type": "string",
                        "description": "The pattern to search for in the file, which can be a simple string or a regular expression"
                    }
                },
                "required": ["path", "search_pattern"]
            }
        },
        {
            "name": "edit_and_apply",
            "description": "Apply AI-powered improvements to a file based on specific instructions and detailed project context. This function reads the file, processes it in batches using AI with conversation history and comprehensive code-related project context. It generates a diff and allows the user to confirm changes before applying them. The goal is to maintain consistency and prevent breaking connections between files. Use this for complex code modifications that require understanding of the broader project context, such as refactoring, implementing new features, or updating code to follow new design patterns.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The absolute or relative path of the file to edit. Use forward slashes (/) for path separation, even on Windows systems. For example, 'src/models/user.rs' or '/home/project/config.toml'."
                    },
                    "instructions": {
                        "type": "string",
                        "description": "Detailed instructions for the AI to follow when editing the file. Include specific changes, coding standards, and any other relevant information. The AI will use these instructions to guide its modifications to the file."
                    },
                    "project_context": {
                        "type": "string",
                        "description": "Comprehensive context about the project, including recent changes, new variables or functions, interconnections between files, coding standards, and any other relevant information that might affect the edit. This context helps the AI understand the broader implications of the changes it's making."
                    }
                },
                "required": ["path", "instructions", "project_context"]
            }
        },
        {
            "name": "read_file",
            "description": "Read the contents of a file at the specified path. Use this when you need to examine the contents of an existing file without making changes. This is useful for code review, understanding the current state of a file, or preparing for edits.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path of the file to read, e.g., 'src/main.rs' or 'Cargo.toml'"
                    }
                },
                "required": ["path"]
            }
        },
        {
            "name": "list_files",
            "description": "List all files and directories in the specified folder. Use this when you need to see the contents of a directory. This is helpful for understanding the project structure, finding specific files, or verifying the presence of expected files and folders.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "path": {
                        "type": "string",
                        "description": "The path of the folder to list (default: current directory). For example, 'src/' or 'tests/integration/'"
                    }
                }
            }
        },
        {
            "name": "read_multiple_files",
            "description": "Read the contents of multiple files at the specified paths. This tool should be used when you need to examine the contents of multiple existing files at once. It will return the status of reading each file, and store the contents of successfully read files in the system prompt. If a file doesn't exist or can't be read, an appropriate error message will be returned for that file. This is particularly useful when working on features that span multiple files or when performing bulk operations.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "paths": {
                        "type": "array",
                        "items": {
                            "type": "string"
                        },
                        "description": "An array of absolute or relative paths of the files to read. Use forward slashes (/) for path separation, even on Windows systems. For example: ['src/main.rs', 'src/lib.rs', 'Cargo.toml']"
                    }
                },
                "required": ["paths"]
            }
        },
        {
            "name": "fetch_commit_changes",
            "description": "Fetch the given commit's changes from a GitHub repository. Use this when you need to see the changes made in an external repository. This is helpful for reviewing code changes, understanding updates in dependencies, or tracking the evolution of a project over time.",
            "input_schema": {
                "type": "object",
                "properties": {
                    "owner": {
                        "type": "string",
                        "description": "The owner of the repository, e.g., 'rust-lang' for the Rust language repository"
                    },
                    "repo": {
                        "type": "string",
                        "description": "The name of the repository, e.g., 'rust' for the Rust language repository"
                    },
                    "sha": {
                        "type": "string",
                        "description": "The SHA of the commit to fetch, e.g., '1a2b3c4d5e6f7g8h9i0j'"
                    }
                },
                "required": ["owner", "repo", "sha"]
            }
        }
    ])
});
