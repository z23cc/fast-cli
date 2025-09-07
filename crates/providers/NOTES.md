providers/ Design Notes
-----------------------

Strategy
- First implement Provider abstraction and one adapter (mock/OpenAI) within core, later split to independent crate.
- Unified capability layer: streaming/non-streaming, function calls (tools), model parameters (temperature/max tokens etc.).

Adapter Suggestions
- OpenAI: Priority async-openai (streaming/function calls mature), wrap as unified interface when necessary.
- Ollama: reqwest direct REST connection; event/streaming structure differences large, parse to Chunk manually.
- Gemini: reqwest + SSE; according to official/changing API, adapt to unified Chunk.

HTTP Client
- Shared reqwest Client (rustls), enable middleware: retry (reqwest-retry), tracing chain, timeout and proxy configuration.

Capability Detection
- supports_streaming / supports_tools / model_capabilities, provided through detection or configuration, UI/CLI can do degradation.

Testing
- Use mockserver or local simulator; replay testing for streaming chunks and error codes; throttling/retry coverage.

