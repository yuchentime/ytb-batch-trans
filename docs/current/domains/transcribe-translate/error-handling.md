---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-error-handling
related:
  - docs/current/domains/transcribe-translate/api-contract.md
  - docs/current/platform/observability.md
last_verified: 2026-09-18
---

# 转录 + 翻译错误处理

## Purpose

稳定错误码、影响、用户可见行为、上报与取消语义。

## Current Behavior

| 错误码 | 触发 | 状态影响 | 上报 |
| --- | --- | --- | --- |
| `whisperMissing` | 探测不到 whisper | 全局门禁，禁止入队；单条阶段则 `error` | 否 |
| `whisperOutOfMemory` | stderr 含 `CUDA out of memory` | 该条 `error`，保留音频 | 是 |
| `whisperFailed` | 退出码非 0 或 JSON 缺失 | 该条 `error` | 是 |
| `ffmpegMissing` / `ffmpegChunkFailed` | 工具缺失 / 切块失败 | 该条 `error` | 是（ChunkFailed） |
| `durationUnknown` | 元数据与 ffprobe 都拿不到时长 | 该条 `error` | 否 |
| `chunkCoverageGap` | 合并覆盖不足 | warning，继续 | 是 |
| `chunkBoundaryRisk` | 边界疑似切词 | warning，继续 | 否 |
| `noSpeechDetected` | whisper 输出 0 段 | 跳过标记（`done` + 汇总） | 否 |
| `deepseekAuthFailed` | 401/403 或无 key | 该条 `error` | 否 |
| `deepseekRateLimited` | 429 退避耗尽 | 该条 `error` | 否 |
| `deepseekServerError` / `deepseekTimeout` | 5xx / 超时（含其余 4xx） | 重试后 `error` | 是（5xx） |
| `translationContractViolation` | id/顺序/非空校验失败 | 重试后 `error`，不写 zh txt | 是 |
| `translationFidelityWarning` | 数字保真未通过 | warning，继续 | 否 |
| `outputWriteFailed` | 原子写失败/磁盘满 | 该条 `error` | 是 |
| `fetchFailed` / `downloadFailed` | 元数据/音频获取失败 | 该条 `error` | 部分 |
| `logWriteFailed` | 日志目录不可写/轮转失败 | warning，**不阻断**；只告警一次 | 否 |

所有错误码在文件中至少一行 ERROR 级日志（含 `run`/`group`/`stage`/`errCode`/原因摘要），同时走 IPC 事件与 UI（AC-24）。

### 取消语义

- `group_cancel` 置分组取消标志并从三个调度器清理；转录中杀死当前 yt-dlp/ffmpeg/whisper 进程树。
- 在途 DeepSeek 请求允许完成，但落盘前检查取消标志（不写中文稿）。
- 失败/取消保留 `.work` 现场（音频与中间产物），支持续跑。
- 开放项（W009）：队列中尚未派发的条目在取消时被调度器丢弃且不上报，批次可能不写 `summary.md`；运行中的条目正常上报 `cancelled`。

## Boundaries

- 业务失败不上报 Sentry（只有应用内部错误）；密钥/Cookie 永不出现在错误消息、事件或日志。
- 错误文案由前端 `errors.transcribe.*` 本地化；后端只给稳定码。

## Contracts

错误码字符串即契约（design 表逐字一致）；新增码需同时更新本文件、`api-contract.md` 与前端 i18n。

## Failure And Edge Cases

- 401 不重试；429/5xx/超时/契约失败按 `maxRetries` 重试；`maxRetries=0` 时只尝试一次。
- 汇总（`summary.md`）记录每条链接的 `status`/`errorCode`/`skipped` 与 token 合计。

## Verification

Rust 单测覆盖错误码映射；L2（规划中）逐码断言文件日志行；L3 手工 grep 日志与密钥检查。
