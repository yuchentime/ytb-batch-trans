---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-domain
related:
  - docs/current/domains/media-queue/index.md
  - docs/current/domains/download-engine/index.md
  - docs/current/domains/settings-preferences/data-model.md
  - docs/current/platform/observability.md
last_verified: 2026-09-18
---

# 转录 + 翻译领域

## Purpose

把一批 YouTube 链接变成两份 txt：英文原稿（whisper 逐字，仅机械分段）与中文译稿（DeepSeek 翻译，段落 1:1 对齐）。
本领域是应用当前唯一的产品主线；下载向 UI 已移除（`media-queue` 的入队/卡片仍被复用，见其 index 的状态说明）。

## Current Behavior

### 职责单元

| 单元 | 归属 | 说明 |
| --- | --- | --- |
| 环境门禁 | `commands/transcription/transcription_probe.rs`、`SetupView.vue` | whisper/ffmpeg/ffprobe 必需；CUDA、模型缓存、DeepSeek key、日志路径仅展示；启动时自动探测一次，可手动重检 |
| 入队 | `commands/transcription/transcribe_start.rs` | 清洗 URL → whisper 门禁（缺失 `Err("whisperMissing")`）→ 每 URL 一个 group → `TranscribeRequest::Batch` |
| 音频获取 | `scheduling/transcribe_pipeline.rs` + `runners/ytdlp_args/audio_args.rs` | `-f ba/best` 落到 `.work/audio.%(ext)s`；复用认证/网络参数与 `group_cancel` |
| 分块转录 | `transcribe/chunking.rs`、`runners/ffmpeg_runner.rs`、`runners/whisper_runner.rs`、`transcribe/merge.rs` | 固定边界切块（容器跟随源扩展名）→ whisper GPU（串行 1 路）→ 平移合并 + 覆盖/边界告警 |
| 原稿 | `transcribe/transcript.rs` | 纯机械分段，不引用任何 LLM 模块 |
| 翻译与保真 | `translation/blocks.rs`、`validate.rs`、`deepseek_client.rs` | 段落成块（≤6 段/3000 字符）、契约校验、数字保真告警、块级重试与用量 |
| 产物与续跑 | `transcribe/artifacts.rs`、`scheduling/transcribe_pipeline.rs` | 原子写、`source.json` 零网络跳过、块级续跑、清理、`summary.md` + `batch_summary` |
| 会话状态 | `src/stores/transcription.ts`、`src/tauri/listeners/transcription.ts` | probe、阶段/进度、产物路径、token、批次汇总；不持久化 |

### 任务路由

- 新任务：`transcribe_start(urls[])` → 每个 group 一张卡片，自动开始，无「配置」步骤。
- 播放列表：pipeline 内自动展开全部条目，同一 group 内逐视频任务，集齐后前端拆成「一视频一卡片」。
- 单条取消：`group_cancel(groupId)` 杀进程树；已存在产物时按 `.work/source.json` 零网络跳过。

### 交付物

`<输出根目录>/<消毒后的标题>/transcript.en.txt` 与 `transcript.zh.txt`（UTF-8 无 BOM、LF、无时间戳、段间空行）；
批次汇总 `<输出根目录>/summary.md`；中间产物在 `.work/`（见 `data-model.md`）。

## Boundaries

- 不做说话人分离、词级时间戳、`.srt` 交付、手动改稿后重译、回译校验。
- 不做本地 LLM 运行时、`faster-whisper`/`whisperX`。
- 中文之外的目标语言不产品化（目标固定 `zh-Hans`，文件名固定）。
- 不提供文稿内嵌预览（只用系统程序打开 txt）。

## Contracts

- 命令/事件字段级契约：`api-contract.md`
- 配置与产物结构：`data-model.md`
- 后端模块约束：`backend-behavior.md`；前端：`frontend-behavior.md`
- 错误码与取消语义：`error-handling.md`
- 日志契约：`../platform/observability.md` 的「文件日志」段

## Failure And Edge Cases

- whisper 缺失：启动探测与 `transcribe_start` 双重门禁，禁止入队（AC-01）。
- GPU OOM：`whisperOutOfMemory`，失败保留 `.work` 现场以便续跑；不自动降级 CPU。
- 固定切分可能切词：`chunkBoundaryRisk` 告警（前块末段距边界 <0.3s 或后块首段 ≤0.02s），不阻断。
- 合并覆盖不足：`chunkCoverageGap` 告警（总缺口 >2s 时逐块定位），不阻断。
- 无语音：0 段 → 标记跳过（`done` + 汇总），不写空稿。
- 同标题视频：落同一目录（设计固有风险，`source.json` 仅辅助识别）。
- 跳过路径不发 `media_add`：卡片由入队时的占位 group + `media_complete` 收尾。

## Verification

见 `verification.md`；真机 L3 结果见 `docs/changes/2026-09-17-batch-transcribe-translate/implementation-notes.md` §21。
