---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-backend-behavior
related:
  - docs/current/domains/download-engine/backend-behavior.md
  - docs/current/domains/transcribe-translate/flow.md
last_verified: 2026-09-18
---

# 转录 + 翻译后端行为

## Purpose

本领域后端模块的职责与硬约束（模块落点、进程、并发、原子写）。

## Current Behavior

| 模块 | 职责 | 关键约束 |
| --- | --- | --- |
| `scheduling/transcribe_pipeline.rs` | 批/视频编排：fetch → 音频 → 切块 → 转录 → 合并 → 原稿 → 翻译 → 写盘 → 清理 → 汇总；批次计数、取消、`TranscribeRequest::{Batch, Expand}` | whisper 用 `TranscribeLimiter`（固定 1）串行；翻译每块取 `TranslateLimiter`；视频槽复用 `DownloadLimiter`；失败/取消保留 `.work` |
| `runners/whisper_runner.rs` | argv 构造、spawn、stdout 段行进度、退出码/OOM 映射、`<chunk>.json` 路径 | **必须复用 `ytdlp_process` 的进程封装**（隐藏窗口、进程组/Job Object、取消杀树，W001）；子进程 PATH 前置 `bin_dir`；OOM 识别 `CUDA out of memory` |
| `runners/ffmpeg_runner.rs` | `ffprobe` 时长、`-ss <start> -i <in> -t <len> -c copy` 切块 | 同一进程封装；不使用 `silencedetect`；**块容器跟随源扩展名**（webm/opus 不能写 `.m4a`） |
| `transcribe/chunking.rs` | 固定边界 `plan_chunks`（`k * chunkMinutes`，`0` = 单块） | 拒绝 0/负/NaN/inf/超长；覆盖并集严格 `[0, duration]`，误差 ≤0.5s |
| `transcribe/merge.rs` | chunk 段平移/排序/去重/覆盖与边界风险判定 | 输出时间单调不减；覆盖缺口与边界风险只告警，不阻断 |
| `transcribe/transcript.rs` | 机械分段（1.2s 间隔 / 700 渲染字符 / 3 句末标点三规则） | **纯函数；不得引用 `translation::*`**（原稿零 LLM 护栏） |
| `transcribe/artifacts.rs` | `.tmp` → rename 原子写、完整性判定、目录名消毒、`source.json`、跳过计划、zh 组装、用量合计 | 半截 JSON 永不视为完成 |
| `translation/blocks.rs` | 段落成块（≤6 段且 ≤3000 字符）、纯文本术语表、system/user prompt、上下文 | 段落永不切分；prompt 集中在 `blocks.rs` |
| `translation/validate.rs` | id 集合/数量/顺序/非空契约校验；数字保真 | 校验不过不落盘；保真只告警 |
| `translation/deepseek_client.rs` | HTTP、JSON 模式、重试/退避、用量；key 由调用方传入 | 120s 超时；key 只在 `Authorization` 头，不落日志；`with_base_delay` 是 L1/L2 试验接缝 |
| `commands/transcription/*` | `transcription_probe`、`transcribe_start` 薄命令 | 门禁不 spawn；URL 合法性由前端保证 |
| `logging` 文件层 | 事件码常量 + 单行格式化 + 脱敏 + 5MB×5 轮转 | 旁路：写失败只告警一次，不阻断流水线 |

## Boundaries

- 命令不感知 UI 状态；pipeline 只通过 IPC 事件与前端交互。
- 不新增并发原语（`DynamicSemaphore` 统一限流）。
- 转录阶段必须串行（单卡 + 显存受限）。

## Contracts

- 进程封装契约：`false` = 已取消（watch），进程输出排空 `run_streaming`。
- 产物与续跑契约：见 `data-model.md`；事件字段见 `api-contract.md`。

## Failure And Edge Cases

- 取消：杀当前进程树；在途 DeepSeek 请求完成后不落盘。
- whisper 失败：退出码非 0 或 JSON 缺失 → `whisperFailed`；OOM 单独映射。
- 批次计数：播放列表展开时「父条目槽位」替换为 N 个子视频（`expand_batch`）。

## Verification

`cargo fmt/clippy/test`（259+）；L2 harness 见 `verification.md`（规划中）。
