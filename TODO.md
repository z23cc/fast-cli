# TODO: OpenAI 接入与流式改造实施计划（Responses API + Chat Completions）

本文档规划在当前 Rust TUI 中无 Mock 接入 OpenAI，完整支持 Responses API 与 Chat Completions 两条流式协议；仅支持 API Key，并支持自定义 `base_url`；具备断线重试、空闲超时、取消与状态栏反馈，配置语义对齐 reference/codex。

## 目标
- 在 TUI 中实现真实流式对话（文本增量、完成信号、错误反馈）。
- 同时支持 Responses API 与 Chat Completions（可配置/自动回退）。
- 仅支持 API Key 鉴权，支持 `base_url`、超时、代理（从配置与环境读取）。
- UI 保持流畅：可取消、重连去重、状态栏展示 Provider/Model 与基础统计。

## 架构与抽象
- crates/core
  - trait `ModelClient`：
    - `async fn send_chat(&self, msgs: &[Message], opts: ChatOpts) -> Result<ChatResult>`
    - `async fn stream_chat(&self, msgs: &[Message], opts: ChatOpts, wire: ChatWire) -> Result<impl Stream<Item = Result<ChatDelta>>>`
  - 基础类型：
    - `ChatWire = { Chat, Responses, Auto }`
    - `ChatOpts { temperature, top_p, max_tokens, ... }`
    - `ChatDelta { Text(String), RoleStart(Role), Finish(FinishReason), Usage(Option<Usage>) /* ToolCall* 后续扩展 */ }`
    - `ChatResult { text, usage?, finish_reason? }`
    - `ChatError { Auth, RateLimit, Timeout, Network, Decode, Protocol, Canceled, ... }`

- crates/providers/openai
  - `config.rs`：`OpenAiConfig { api_key, base_url, timeout_ms, proxy?, org?, project?, wire_api }` + `from_env_and_file()` 读取。
  - `client.rs`：`OpenAiClient { http: reqwest::Client, cfg: OpenAiConfig }` 实现 `ModelClient`；公共 Header/超时/代理/HTTP2。
  - `wire_chat.rs`：Chat Completions 流式（SSE data: JSON；[DONE] 结束）。
  - `wire_responses.rs`：Responses API 流式（按 event 分发：`response.output_text.delta`/`response.completed`/`response.error`）。
  - `types.rs`：OpenAI <-> core 类型映射（messages、delta、finish）。
  - `error.rs`：OpenAI 错误 JSON / reqwest::Error → `ChatError`。
  - `retry.rs`：指数退避 + 抖动；白名单（429/5xx）；空闲超时重连。

- crates/tui
  - 通过 `Box<dyn ModelClient + Send + Sync>` 持有客户端（初期仅 openai）。
  - `submit()`：spawn 后台任务调用 `stream_chat`，通过 `tokio::sync::mpsc` 把 `ChatDelta` 发送至 UI 线程；UI tick drain、累加到最后一条 assistant 消息、`dirty = true` 重绘。
  - 取消：Esc/Ctrl-C 以 `CancellationToken` 通知后台任务退出。
  - 状态栏：显示 `[OpenAI][{model}]`、滚动/光标、Hist、Ctx、搜索命中；后续可加 tokens/sec/elapsed。

## 配置（对齐 reference/codex）
- 路径：
  - Windows：`%USERPROFILE%/.fast/config.toml`
  - Linux/macOS：`~/.config/fast/config.toml`
- 顶层键：
  - `model = "gpt-4o-mini"`
  - `model_provider = "openai"`
  - `wire_api = "responses" | "chat" | "auto"`（auto 先试 Responses，失败回退 Chat）
  - `approval_policy = "on-request"`（已有语义）
  - `stream_max_retries = 5`
  - `stream_idle_timeout_ms = 300000`
  - `timeout_ms = 30000`
- provider map（可选）：
  - `[model_providers.openai]`
    - `name = "OpenAI"`
    - `base_url = "https://api.openai.com/v1"`（可被 `OPENAI_BASE_URL` 覆盖）
    - `env_key = "OPENAI_API_KEY"`
    - `wire_api = "responses"`（默认）
    - `timeout_ms`、`proxy`（可选）
- 环境变量：
  - 必须：`OPENAI_API_KEY`
  - 可选：`OPENAI_BASE_URL`、`HTTP(S)_PROXY`

## 实施步骤（端到端，无 Mock）
1) 核心抽象（M1）
   - 在 `crates/core` 定义 `ModelClient` 与类型：`ChatWire/ChatOpts/ChatDelta/ChatError/ChatResult`。
   - 要求：编译通过、Clippy 零警告。

2) OpenAI 配置与 HTTP 客户端（M2）
   - `OpenAiConfig::from_env_and_file()` 实现 env+toml 合并与 profiles 覆盖。
   - `OpenAiClient::new(cfg)` 构建 `reqwest::Client`（Authorization、超时、代理、HTTP2/Keep-Alive）。
   - 验收：可发非流式 `send_chat` 并拿回完整结果（本地人工验证）。

3) Chat Completions 流式（M3）
   - `wire_chat.rs`：POST `/v1/chat/completions`，`stream: true`，SSE 解析。
   - SSE 实现：
     - bytes_stream + 自写最小 SSE 解析（按 `\r\n\r\n` 切分 event；行级 `event:` / `data:`；`data` 合并 JSON）。
     - 增量：`choices[0].delta.{role?,content?}` → `ChatDelta::{RoleStart,Text}`；`finish_reason` → `Finish`。
     - 终止：`data: [DONE]`。
   - 容错：`stream_idle_timeout_ms` 超时→断开重试（至多 `stream_max_retries`）；重连去重（对已收文本做后缀匹配，丢重复前缀）。

4) Responses API 流式（M4）
   - `wire_responses.rs`：POST `/v1/responses`，`stream: true`。
   - 事件分发：
     - `response.output_text.delta` → `ChatDelta::Text`
     - `response.completed` → `Finish`（可带 `Usage`）
     - `response.error` → `ChatError`
   - 与 M3 共享 SSE/重试/去重基础设施。

5) TUI 接线与状态（M5）
   - 在 `tui` 中引入 `tokio::Runtime` 与 `OpenAiClient`（通过 trait 依赖）。
   - `submit()`：改为 spawn 后台流式任务 + mpsc；UI drain 合并增量，`dirty = true` 重绘；Esc/Ctrl-C 取消。
   - 状态栏：追加 `[OpenAI][{model}]` 展示；错误 toast/重试次数短提示。

6) 配置落地与回退策略（M6）
   - `wire_api = auto`：优先 Responses，遇 404/未启用/协议错误回退 Chat。
   - `base_url` 与 `timeout_ms/代理` 全链路生效；命令面板保留 profile/provider/model 切换入口（可后续实现）。

7) 完善与加固（M7）
   - 断线重试日志/统计（tokens/sec/elapsed 后续加入）。
   - 错误映射与 UI 提示完善（RateLimit/Network/Timeout 等）。
   - 代码规范：Clippy 零警告，统一日志与错误上下文。

## 关键技术细节
- SSE 解析：
  - 维护 `buffer: BytesMut`；每次追加网络 chunk 后循环提取下一个完整 event（以 `\r\n\r\n` 分隔）。
  - 单个 event 内多行 `data:` 合并，`event:` 可选；空行/跨 chunk 断裂需缓冲。
- 去重策略：
  - 重连时取已收文本 `acc_text` 与新流第一段 `delta` 做后缀匹配，剔除重复部分，仅追加差异；简单有效，极端情况下允许少量重复字符。
- 重试与超时：
  - 429/5xx 白名单重试；指数退避 + 抖动；空闲超时触发重连。
- 鉴权与安全：
  - 仅从 ENV 读取 `OPENAI_API_KEY`；不持久化、不打印日志；`base_url` 可由 ENV 或配置覆盖。

## 测试与验证
- 手工：设置 `OPENAI_API_KEY`（及 `OPENAI_BASE_URL` 如需），逐步验证 M3/M4 流式；断网 10s 后恢复观察自动重试与去重；长输出折叠展开与滚动一致。
- Gated 集成测试：标记 `#[ignore]`，仅本地有 Key 时运行（CI 默认跳过），避免泄密。

## 风险与应对
- Responses 与 Chat 差异较大：先实现文本增量 + 完成，后续扩展函数/工具调用。
- Azure/兼容服务路径差异：先支持标准 OpenAI base_url；后续引入 provider_kind 分支路径。
- SSE 解析健壮性：注意空行/跨 chunk 断裂与 JSON 容错；必要时基于成熟 SSE crate，但自实现可控性更强。

---

以上为完整实施路线图。建议按 M1→M2→M3→M4→M5→M6 分阶段推进，每阶段保证可运行、日志可观测、Clippy 零警告；完成后再做体验增强（tokens/sec/elapsed、命令面板切换 provider/model、MCP 等）。

