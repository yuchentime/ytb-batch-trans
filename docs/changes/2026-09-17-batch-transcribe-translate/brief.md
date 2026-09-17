---
status: draft
layer: change
domain: transcribe-translate
created_at: 2026-09-17
---

# 批量转录 + AI 翻译中文 Brief

## Problem

当前应用是 yt-dlp 的下载外壳：用户拿到的是媒体文件，而不是可读的文稿。
开发者需要把「一批 YouTube 链接」变成「每个视频两份 txt（英文原稿 + 中文译稿）」，
其中译稿由 AI 生成并做可读性排版，但**不得增删语义**。

本机已具备转录条件（openai-whisper CLI + CUDA GPU + ffmpeg），但应用目前没有任何转录或
LLM 集成（`src/`、`src-tauri/src/` 中无相关实现），需要从零新增。

## Decision / Scope

按开发者决策（Q1/Q4/Q11/Q15）：

- 应用**直接改造为转录工具**（覆盖下载用途），不再暴露下载清晰度/编码/字幕/过滤等选项。
- 下载阶段只取 `bestaudio`（`-f ba/best`），转录成功后删除媒体文件。
- 转录用本机 `whisper` CLI（默认 `small` + `cuda` + `fp16`），按 `chunkMinutes` 分块以支持进度与续跑。
- 翻译用 **DeepSeek**（OpenAI 兼容接口，Rust 侧调用），API key 存 stronghold 新键 `ai.apiKey`。
- 每条链接产出 `<输出根目录>/<视频标题>/transcript.en.txt` 与 `transcript.zh.txt`；
  原稿逐字（仅机械分段），译稿按段 1:1 对齐、允许去掉口误/填充词、禁止增删语义。

范围覆盖：配置 schema（新增 transcription/translation、裁剪 downloads 类字段）、Rust 侧新增
whisper runner + 分块 + 翻译客户端 + 流水线、前端新增环境检查页/卡片步骤/设置页、
移除下载类 UI 与参数构造模块、批次汇总与日志/诊断、以及 `docs/current/` 的同步更新。

追加范围（2026-09-17 开发者要求）：**长链路必须有一个 `.log` 文件**持续记录关键事件与错误，方便出错时即时排查。
落地方案：在 `init_tracing` 的现有 fmt/Sentry 两层之上再加一个 `tracing-appender` 滚动文件层
（`<app_dir>/logs/transcribe.log`，5MB × 5），单行格式 + 稳定事件码 + 集中脱敏（新增 `logging.verbose` 开关）。

## Out Of Scope

- 说话人分离（diarization）、词级时间戳、字幕文件交付（`.srt` 仅作中间产物）。
- 手动改稿后重译、译文质量自动评分、回译校验（第二期评估）。
- 本地 LLM 运行时（Ollama/LM Studio）与 `faster-whisper`/`whisperX` 接入（需先装依赖，另行确认）。
- 保留或恢复原有下载功能（按 Q15 明确放弃）。
- 多语言译稿（除中文外）与目标语言可配置之外的翻译策略。
- 日志上传/聚合（Sentry、远端采集、日志归档服务）；文件日志只存本机且不外传。
