---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-api-contract
related:
  - docs/current/shared/ipc-conventions.md
  - docs/current/domains/transcribe-translate/data-model.md
last_verified: 2026-09-18
---

# 转录 + 翻译 API 契约

## Purpose

本领域新增/复用的 IPC 命令与事件、以及 DeepSeek HTTP 契约的字段级说明。

## Current Behavior

### 命令

#### `transcription_probe`

无参数，只读，不写配置、不下载模型。

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `whisperPath` | `string?` | 解析到的可执行文件路径；`transcription.whisperPath` 优先，其次 `bin_dir`、`PATH` |
| `whisperVersion` | `string?` | 来自 `python -m pip show openai-whisper`（CLI 无 `--version`） |
| `whisperFound` | `boolean` | 门禁必需项 |
| `cudaAvailable` | `boolean` | `nvidia-smi` 驱动级探测（不加载 torch） |
| `cudaDevice` | `string?` | 第一块 GPU 名称 |
| `model` | `string` | 配置中的模型名（`small`/`medium`/`large-v3`） |
| `modelCached` | `boolean` | `~/.cache/whisper/<model>.pt` 是否存在 |
| `modelPath` | `string?` | 缓存文件路径 |
| `modelSizeBytes` | `number?` | 缓存文件大小 |
| `ffmpegPath` / `ffprobePath` | `string?` | 门禁必需项（分块与时长探测） |
| `apiKeyConfigured` | `boolean` | 只读 stronghold（`ai.apiKey` 是否存在），不验证连通性 |
| `logDir` / `logFile` | `string` | 文件日志目录与当前文件路径 |
| `logSizeBytes` | `number` | 当前日志文件大小 |

失败：无法定位 whisper/ffmpeg/ffprobe 时返回对应字段为空/`false`，命令本身仍成功（`Result<TranscriptionProbe, String>`）。

#### `transcribe_start`

| 参数 | 类型 | 说明 |
| --- | --- | --- |
| `urls` | `string[]` | 入队链接；空字符串被过滤，全空返回 `Err("no urls provided")` |

返回：`string[]`（与输入顺序一致的一组 groupId；播放列表链接对应同一个 group）。

错误：`Err("whisperMissing")`（环境门禁，不 spawn 任何进程）、`Err("output root directory is not configured")`、通道关闭错误。

#### `group_cancel`（复用）

`groupId: string`；置分组取消标志、向 fetch/download/transcribe 调度器发 `Cleanup`、清空内存日志。转录中会杀死当前 yt-dlp/ffmpeg/whisper 进程树。

#### 已移除

`media_size` 命令与 `media_size` 事件（体积查询随下载 UI 一起移除）。

### 事件

| 事件 | 载荷 | 触发 |
| --- | --- | --- |
| `transcribe_stage` | `{ id, groupId, stage }`；`stage ∈ downloadingAudio \| transcribing \| translating \| writing` | 阶段切换 |
| `transcribe_progress` | `{ id, groupId, chunkIndex, chunkTotal, percent }` | whisper stdout 段行（块内百分比） |
| `translate_progress` | `{ id, groupId, blockIndex, blockTotal, promptTokens, completionTokens }` | 单个 block 翻译成功 |
| `artifact_written` | `{ id, groupId, kind: "en" \| "zh", path }` | 原稿/译稿原子写成功 |
| `batch_summary` | `{ path, items: BatchSummaryItem[] }` | 批次归零、写 `summary.md` 之后 |
| `media_add`（复用） | `{ groupId, total, item }`；单视频 = `ParsedSingleVideo`，播放列表 leader = `ParsedPlaylist` | fetch 元数据 |
| `media_complete` / `media_fatal`（复用） | 既有载荷 | 单条终态（跳过也发 `media_complete`） |
| `media_diagnostic`（复用） | 既有载荷 | yt-dlp/whisper/ffmpeg 诊断行 |

`BatchSummaryItem`：`{ groupId, url, status: "done" \| "failed" \| "cancelled", errorCode?, outputs[], skipped, promptTokens, completionTokens }`。

### DeepSeek HTTP 契约

- 请求：`POST {translation.baseUrl}/chat/completions`，头 `Authorization: Bearer <ai.apiKey>`；体 `{ model, temperature, response_format: { type: "json_object" }, messages: [system, user] }`。
- 响应：`choices[0].message.content` 为 JSON 字符串 `{ "translations": [{ "id": <number>, "zh": <string> }] }`；`usage.prompt_tokens`/`usage.completion_tokens` 累计。
- 错误：401/403 → 不重试（`deepseekAuthFailed`）；429 → `Retry-After`（≤60s）否则指数退避，可重试；5xx/超时/网络/契约失败 → 可重试；其余 4xx → 不重试（归 `deepseekServerError`）。尝试次数 = `maxRetries + 1`。
- 密钥每块从 stronghold 读取一次，不缓存、不进日志；请求体不得包含密钥。

## Boundaries

- 事件名与阶段值固定；新增阶段需同时更新本文件与 `shared/ipc-conventions.md`。
- 命令层只做参数适配与状态获取，业务在 `scheduling`/`transcribe`/`translation`。

## Failure And Edge Cases

- `transcribe_start` 返回 `groupId[]` 后，第一个 `media_add` 之前卡片处于 `fetching`；跳过路径不发 `media_add`，由 `media_complete` 收尾。
- `batch_summary` 只在批次所有已派发条目都上报后发出；队列中取消的条目当前不上报（W009）。

## Verification

Rust 单测覆盖载荷序列化与门禁；IPC 三处同步（命令文件、`lib.rs` handler、`src-isolation/main.ts` 白名单）由 `npm run build` 与 E2E mock 校验。
