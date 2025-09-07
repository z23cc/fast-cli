core/ Design Notes
--------------

Responsibilities
- Define core abstractions and data models: AiClient/Tool/Conversation/Message.
- Provide conversation storage interface and default implementation (file storage, later rusqlite).
- Manage configuration and key reading; unified error and logging.
- Provide tool (file/network) sandbox and parameter validation capabilities.

Key Modules (Suggested)
- ai_client/         Provider abstractions and shared types (streaming/non-streaming, capability detection).
- tools/             Tool trait, registry, runtime context, built-in tool implementations.
- convo/             Conversation/Message/Metadata and trimming strategies.
- store/             ConversationStore trait + file_store default implementation.
- config/            Configuration and loading merge; path and keyring support.
- errors/            FastError; thiserror derive; miette/anyhow only used at application layer.
- logging/           tracing initialization (library only emits events, application decides subscription).

Abstract Overview
- trait AiClient {
  }
- trait Tool {
  }
- trait ConversationStore {}

Dependency Suggestions (core)
- tokio, reqwest(+eventsource, middleware, retry), serde/schemars/jsonschema
- tracing/thiserror, figment or config, directories, keyring
- cap-std, unicode-width, dunce/path-absolutize, backoff

MVP Output
- File storage implementation, two built-in tools (file_read/web_fetch), mock provider, OpenAI adapter skeleton.

