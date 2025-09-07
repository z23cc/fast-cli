tui/ (Interface) Design Notes
---------------------------

Stage: Second stage launch, prioritize ensuring CLI availability.

Responsibilities
- Provide Ratatui interface: chat area, input box, sidebar (sessions/config).
- Async event loop: tokio + mpsc, AI streaming output incremental rendering; support Stop/Retry.

Component Division
- app.rs         Global state (current session, messages, generation state, queue).
- ui/chat.rs     Message rendering (scroll/pagination); large text on-demand rendering.
- ui/input.rs    Input component (multi-line, history, shortcuts).
- ui/sidebar.rs  Session list/config; switch/create/delete.
- events.rs      Key mapping and command mode; Ctrl-C cancel, Tab focus switch.
- terminal.rs    Terminal initialization/restore; cross-platform compatibility.

Technical Points
- Windows Chinese input and wide characters: unicode-width; disable complex rendering first for stability when necessary.
- Markdown rendering: pulldown-cmark parse to custom widgets; code highlighting (syntect optional).
- Performance: avoid rendering all text at once; pagination/virtualization; async pipeline backpressure.

Testing
- Frame snapshot testing: layout, scroll boundaries, long text performance benchmarks.

