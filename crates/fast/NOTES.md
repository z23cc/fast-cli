fast/ (Binary) Design Notes
-------------------------

Responsibilities
- Provide CLI entry and subcommand parsing (clap).
- Call AiClient/Store/Tools in core, complete terminal interaction (non-TUI).
- Responsible for log subscription and human-readable error printing.

Subcommands (Initial)
- ask "question"             Single-turn Q&A; --model/--provider/--no-stream/--timeout.
- chat                      Multi-turn conversation REPL; history, retry, cancel (Ctrl-C); support pipe input.
- tools run <tool> --args   Run tool, output JSON; default enable file_read, optional web_fetch.
- sessions [list|create|use|delete]  Session management; support title/model/provider metadata.
- config [get|set|path]     Config read/write and validation; sensitive field masking; --config-path override.

Behavior Details
- Streaming output: print by chunks; verify Chinese wide character alignment and colors on Windows console.
- Cancel and timeout: tokio-util CancellationToken + tokio::time::timeout.
- Retry and backoff: unified to core's HTTP client middleware; CLI side only displays.
- Exit codes: 0 success; non-0 indicates error (distinguish params/network/throttling/permissions etc.).

Dependency Suggestions (bin)
- clap、anyhow/miette、tracing-subscriber、rustyline（chat REPL）。

MVP Output
- fast ask (mock provider → async-openai), fast chat (streaming+cancel), config/sessions basic capabilities.

