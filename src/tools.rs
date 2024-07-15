use once_cell::sync::Lazy;
use serde_json::{json, Value};

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
        // {
        //     "name": "tavily_search",
        //     "description": "Perform a web search using Tavily API to get up-to-date information or additional context. Use this when you need current information or feel a search could provide a better answer.",
        //     "input_schema": {
        //         "type": "object",
        //         "properties": {
        //             "query": {
        //                 "type": "string",
        //                 "description": "The search query"
        //             }
        //         },
        //         "required": ["query"]
        //     }
        // }
    ])
});
