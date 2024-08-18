# claude-engineer-rs

## Installation

### Mac/Linux
```bash
cargo build --release
cargo install --path .
```

## Usage
In your desired repository, run:
```bash
claude-engineer-rs
```

## Project Overview

This project, `claude-engineer-rs`, is a Rust implementation inspired by the original [claude-engineer](https://github.com/Doriandarko/claude-engineer) project. It was developed with the dual purpose of enhancing my understanding of Rust and exploring the possibilities of AI-assisted software engineering. It takes a more targeted approch to development with similarties to [omni-engineer](https://github.com/Doriandarko/omni-engineer).

Key aspects of the project include:

1. **Inspiration from claude-engineer:** The project builds upon the concepts introduced in the original claude-engineer, adapting them to a Rust environment.

2. **Targeted task execution:** The tool is designed to assign specific tasks to Claude (an AI model) and incorporate feedback after task completion, aiming to streamline the development process.

3. **Learning and growth:** Through the development of this project, I have gained valuable insights into working with Large Language Models (LLMs) in the context of software development tools.

4. **Claude tools integration:** The project includes logic for interacting with Claude tools. You can view this implementation in the [tools.rs](https://github.com/tim-schultz/claude-engineer-rs/blob/main/src/tools.rs) file.

## Acknowledgements

A special thanks goes to [@Doriandarko](https://github.com/Doriandarko) for open-sourcing both omni-engineer and claude-engineer. These projects have been instrumental in advancing my understanding of LLM-based development tools and have significantly influenced the direction of this project.
