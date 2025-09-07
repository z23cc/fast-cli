pub mod llm {
    use futures::Stream;
    use serde::{Deserialize, Serialize};
    use thiserror::Error;

    #[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
    pub enum Role {
        User,
        Assistant,
        System,
    }

    #[derive(Clone, Debug, Serialize, Deserialize)]
    pub struct Message {
        pub role: Role,
        pub content: String,
    }

    #[derive(Clone, Debug)]
    pub struct ChatOpts {
        pub model: String,
        pub temperature: Option<f32>,
        pub top_p: Option<f32>,
        pub max_tokens: Option<u32>,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub enum ChatWire {
        Chat,
        Responses,
        Auto,
    }

    #[derive(Clone, Debug)]
    pub enum ChatDelta {
        RoleStart(Role),
        Text(String),
        Finish(Option<String>),
        Usage { prompt_tokens: Option<u32>, completion_tokens: Option<u32> },
    }

    #[derive(Clone, Debug)]
    pub struct ChatResult {
        pub text: String,
        pub finish_reason: Option<String>,
        pub prompt_tokens: Option<u32>,
        pub completion_tokens: Option<u32>,
    }

    #[derive(Error, Debug)]
    pub enum ChatError {
        #[error("auth error: {0}")] Auth(String),
        #[error("rate limit: {0}")] RateLimit(String),
        #[error("timeout: {0}")] Timeout(String),
        #[error("network: {0}")] Network(String),
        #[error("decode: {0}")] Decode(String),
        #[error("protocol: {0}")] Protocol(String),
        #[error("canceled")] Canceled,
        #[error("other: {0}")] Other(String),
    }

    pub type ChatStream<'a> = Pin<Box<dyn Stream<Item = Result<ChatDelta, ChatError>> + Send + 'a>>;

    use std::pin::Pin;

    #[allow(async_fn_in_trait)]
    pub trait ModelClient: Send + Sync {
        async fn send_chat(&self, msgs: &[Message], opts: &ChatOpts) -> Result<ChatResult, ChatError>;
        async fn stream_chat<'a>(
            &'a self,
            msgs: Vec<Message>,
            opts: ChatOpts,
            wire: ChatWire,
        ) -> Result<ChatStream<'a>, ChatError>;
    }
}

pub fn ping() -> &'static str { "core-ok" }
