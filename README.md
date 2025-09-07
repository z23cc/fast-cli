fast
====

This repository plans a fast, robust, and scalable local AI assistant, providing CLI and optional TUI interface later. Current stage focuses on setting up directory structure and recording design thoughts (NOTES).

Directory Overview (Planned)
- crates/core        Core logic (conversations, AI abstractions, tools, config, errors)
- crates/fast        Binary entry (commands: ask/chat/tools/config/sessions)
- crates/tui         Ratatui interface (second stage)
- crates/providers   Provider adapters (can be split later/enabled by feature)
- docs/              Design notes, research and decisions (UTF-8)

Goals
- Stable cross-platform (Windows/macOS/Linux), friendly to Chinese/wide characters
- CLI first then TUI, core and interface decoupled; pluggable Provider/tools
- Good streaming output, cancellation, retry, logging and error experience

Status
- Only set up directory and NOTES documents, used to record design decisions and future implementation paths.

