---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-data-model
related:
  - docs/current/domains/settings-preferences/data-model.md
  - docs/current/platform/storage.md
last_verified: 2026-09-18
---

# 转录 + 翻译数据模型

## Purpose

本领域的持久化配置、输出目录结构与中间产物格式。

## Current Behavior

### 配置（`config.store.json`，`Config`）

```text
transcription: {
  model: "small" | "medium" | "large-v3",   // 默认 small
  device: "cuda" | "cpu",                   // 默认 cuda
  fp16: bool,                               // 默认 true（cuda 生效）
  language: "en" | "auto",                  // 默认 en
  chunkMinutes: u32,                        // 默认 20；0 = 整段一次
  keepAudio: bool,                          // 默认 false（成功后删 .work/audio.*）
  whisperPath: Option<String>,              // 可选覆盖
  conditionOnPreviousText: bool             // 默认 false
}
translation: {
  baseUrl: String,                          // 默认 https://api.deepseek.com
  model: String,                            // 默认 deepseek-chat；实测 deepseek-flash 可用
  temperature: f32,                         // 默认 0.3
  concurrency: usize,                       // 默认 2
  maxRetries: u32,                          // 默认 2
  maxSegmentsPerBlock: u32,                 // 默认 6
  maxCharsPerBlock: u32,                    // 默认 3000
  glossary: String,                         // 每行 `原文=译文`
  dropFillers: bool                         // 默认 true
}
output: {
  rootDir: Option<String>,                  // 默认 <系统下载目录>/ovd-transcripts（before_initialized 填）
  overwrite: bool,                          // 默认 false
  restrictFilenames: bool                   // 默认 false（仅影响目录名）
}
logging: { verbose: bool }                  // 默认 false
```

旧配置兼容：`JsonBackedState` 深合并 + 未知键忽略，被删除的下载向字段不再生效（AC-18）。

### 交付物

```text
<output.rootDir>/<sanitize_directory_name(标题)>/
  transcript.en.txt      # whisper 逐字，仅机械分段，UTF-8 无 BOM + LF，段间空行
  transcript.zh.txt      # 译稿，与原文段落 1:1
  .work/                 # 中间产物，非交付物，失败时保留
<output.rootDir>/summary.md  # 批次汇总（表格 + Totals；固定写在根目录）
```

目录名：非法字符（`< > : " / \ | ? *`、控制符）替换为 `_`；长度截断至 120 字符；Windows 保留名加 `_` 前缀；空标题回落 `untitled`。

### `.work/` 产物

| 文件 | 格式 | 用途 |
| --- | --- | --- |
| `audio.<ext>` | yt-dlp 原始音频（webm/m4a…） | 成功双 txt 后按 `keepAudio` 删除 |
| `chunks/audio.NNN.<ext>` | `-c copy` 切块，扩展名跟随源音频 | whisper 输入 |
| `chunks/audio.NNN.json` | whisper `--output_format json`（`text`/`segments`/`language`） | 块完成权威标志；段字段 `start/end/text` |
| `segments.json` | `MergedSegment[]`：`{ id, start, end, text }` | 合并结果（时间已平移、单调、id 稳定 `{chunk}:{seg}`） |
| `zh.blocks.json` | `{ "<blockIndex>": { ids, zh[], usage } }` | 块级翻译落盘，可续跑 |
| `source.json` | `{ url, title }` | 重跑时零网络定位已有输出（跳过判定） |

原子写：所有 JSON/txt 先写 `.tmp` 再 rename；`.tmp` 残留永不视为完成。

### 会话内（不持久化）

`src/stores/transcription.ts`：`probe`、逐 item 阶段与块/翻译进度、artifact `{en, zh}`、token 累计、`summaries`。
`MediaState`：`fetching → downloadingAudio → transcribing → translating → writing → done | error`（另有 `paused*` 历史态）。

## Boundaries

- 交付文件名与目标语言固定，不可配置。
- `.work` 仅为中间产物；不得作为交付路径。
- 不新增每视频日志文件（单链路靠文件日志的 `group` 字段检索）。

## Failure And Edge Cases

- 损坏/空文件视为缺失并重算（`segments.json`、`zh.blocks.json`、txt）。
- 同标题视频落同一目录：`source.json` 仅辅助识别，不解决冲突。
- 磁盘满/写入失败 → `outputWriteFailed`，保留现场。

## Contracts

命令与事件字段见 `api-contract.md`；文件日志路径与轮转见 `../platform/observability.md`。

## Verification

Rust 单测覆盖默认值、旧配置兼容、原子写与完整性判定、跳过计划、汇总渲染。
