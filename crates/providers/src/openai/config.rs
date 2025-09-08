use directories::BaseDirs;
use serde::Deserialize;
use std::{env, fs, path::PathBuf, time::Duration};

#[derive(Clone, Debug, Deserialize)]
pub struct OpenAiFileConfig {
    pub model: Option<String>,
    pub model_provider: Option<String>,
    pub wire_api: Option<String>,
    pub stream_max_retries: Option<u32>,
    pub stream_idle_timeout_ms: Option<u64>,
    pub timeout_ms: Option<u64>,
    pub model_providers: Option<serde_json::Value>,
}

#[derive(Clone, Debug)]
pub struct OpenAiConfig {
    pub api_key: String,
    pub base_url: String,
    pub model: String,
    pub wire_api: String, // "responses" | "chat" | "auto"
    pub timeout: Duration,
    pub stream_max_retries: u32,
    pub stream_idle_timeout: Duration,
    pub proxy: Option<String>,
}

impl OpenAiConfig {
    pub fn from_env_and_file() -> anyhow::Result<Self> {
        let api_key =
            env::var("OPENAI_API_KEY").map_err(|_| anyhow::anyhow!("OPENAI_API_KEY not set"))?;
        let base_url =
            env::var("OPENAI_BASE_URL").unwrap_or_else(|_| "https://api.openai.com/v1".to_string());

        let mut model = "gpt-5".to_string();
        let mut wire_api = "responses".to_string();
        let mut timeout_ms = 30_000u64;
        let mut stream_max_retries = 5u32;
        let mut stream_idle_timeout_ms = 300_000u64;

        if let Some(path) = Self::config_path() {
            if path.exists() {
                if let Ok(toml) = fs::read_to_string(&path) {
                    if let Ok(file_cfg) = toml::from_str::<OpenAiFileConfig>(&toml) {
                        if let Some(m) = file_cfg.model {
                            model = m;
                        }
                        if let Some(w) = file_cfg.wire_api {
                            wire_api = w;
                        }
                        if let Some(t) = file_cfg.timeout_ms {
                            timeout_ms = t;
                        }
                        if let Some(r) = file_cfg.stream_max_retries {
                            stream_max_retries = r;
                        }
                        if let Some(idle) = file_cfg.stream_idle_timeout_ms {
                            stream_idle_timeout_ms = idle;
                        }
                    }
                }
            }
        }

        let proxy = env::var("HTTPS_PROXY")
            .ok()
            .or_else(|| env::var("HTTP_PROXY").ok());

        Ok(OpenAiConfig {
            api_key,
            base_url,
            model,
            wire_api,
            timeout: Duration::from_millis(timeout_ms),
            stream_max_retries,
            stream_idle_timeout: Duration::from_millis(stream_idle_timeout_ms),
            proxy,
        })
    }

    fn config_path() -> Option<PathBuf> {
        let base = BaseDirs::new()?;
        let p = if cfg!(target_os = "windows") {
            base.home_dir().join(".fast").join("config.toml")
        } else {
            base.config_dir().join("fast").join("config.toml")
        };
        Some(p)
    }
}
