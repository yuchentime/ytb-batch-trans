---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-verification
related:
  - docs/current/shared/testing-strategy.md
  - docs/current/platform/build-test-lint.md
last_verified: 2026-09-18
---

# 转录 + 翻译验证

## Purpose

本领域的分层验证入口、必须不破坏项与回归矩阵。

## Current Behavior

### 分层

| 层 | 入口 | 覆盖 |
| --- | --- | --- |
| L1 | `cd src-tauri && cargo test`；`npm run test:unit` | argv、分块/合并、机械分段、block 组装与校验、配置兼容、错误映射、日志格式化/脱敏/轮转/降级、进程取消语义 |
| L2（规划中，Phase C 第 18 项） | `TRANSCRIBE_E2E=1 cargo test --test transcribe_e2e` | fake whisper/ffmpeg + mock DeepSeek + 真实文件系统：跳过/覆盖/清理/取消/并发/重试/汇总/日志 |
| E2E | `npm run test:e2e` | `/setup` 渲染、入队与阶段事件、门禁禁用；Playwright mock IPC |
| L3 | implementation-notes §21 的记录方式（应用自身入口 + 真机工具/API） | 真实 whisper GPU、真实 DeepSeek、日志实战排查 |

### 已执行的真实结果（2026-09-18）

`https://www.youtube.com/watch?v=nIABz0Z4IRA`：`run.end done=1 failed=0 skipped=0`；
`transcript.en.txt` 27 段 / 8836 chars，`transcript.zh.txt` 27 段 / 2718 chars（段落 1:1），tokens 4276/12785。

### 必须不破坏（Must-Not-Break）

- 受限视频仍可通过 Cookie/浏览器 Cookie/stronghold 凭据下载音频（认证键名不得改名）。
- 代理 / impersonate / extractor-args 仍作用于 yt-dlp。
- 旧配置文件可加载；设置保存/重置/窗口几何行为不变。
- `ai.apiKey`/Cookie/Bearer/请求头不出现在日志、事件、Sentry 与配置文件中。
- 托盘、关闭行为、全局快捷键、通知策略、语言切换、自更新不回归。
- 现有内存分组日志与 Sentry 行为不变；文件日志只是新增通道。

### 回归矩阵（本领域）

| 场景 | 预期 | 层 |
| --- | --- | --- |
| whisper 未安装 | 输入禁用 + `whisperMissing` | L1/E2E |
| 3 条同时入队 | 下载 ≤2、whisper 串行、翻译 ≤2 | L2 |
| 已存在两份 txt | 零网络跳过 + `done` + 汇总标记 | L2 |
| 翻译块契约失败 | 重试后失败、不写 zh txt、保留现场 | L2 |
| 429 限流 | 退避后成功 | L2 |
| 转录中取消 | 进程被杀、不写产物、保留现场 | L2/L3 |
| 25 分钟视频分块 | 2 块、边界告警可观测、无文本丢失 | L1/L3 |
| whisper OOM | `whisperOutOfMemory` + 日志 ERROR 行 + 可续跑 | L2/L3 |
| 日志目录只读 | 转录仍成功，仅一次 `logWriteFailed` | L2 |
| 日志超 5MB | 轮转为 `.1`…`.4`，总量受限 | L1/L2 |

## Boundaries

- L1 不证明译文语义；语义质量属 L3 人工抽验。
- 不在 CI 真实调用 DeepSeek；L2 使用 mock HTTP。

## Contracts

测试命令与 fixture 位置见 `../platform/build-test-lint.md`；分层原则见 `../../shared/testing-strategy.md`。

## Failure And Edge Cases

- L2 harness 需要为 pipeline 提供可注入的运行时面（当前绑定 `AppHandle<Wry>`）；在 harness 落地前，取消/跳过/清理的联动只用 L3 真机覆盖。
- `TRANSCRIBE_E2E` 默认不启用，避免 CI 依赖真实进程与网络。

## Verification

```bash
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test
npm run lint:fix && npm run test:unit && npm run test:e2e && npm run build
python docs/scripts/check_doc_runtime.py docs
```
