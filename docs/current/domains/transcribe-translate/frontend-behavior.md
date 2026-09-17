---
status: current
layer: domain
domain: transcribe-translate
canonical_for:
  - transcribe-translate-frontend-behavior
related:
  - docs/current/domains/media-queue/frontend-behavior.md
  - docs/current/domains/transcribe-translate/flow.md
last_verified: 2026-09-18
---

# 转录 + 翻译前端行为

## Purpose

本领域前端的状态机、卡片步骤、页面与入口（环境门禁、设置、详情、快捷键）。

## Current Behavior

### 状态与卡片

`MediaState` 转录态：`fetching` → `downloadingAudio` → `transcribing` → `translating` → `writing` → `done` / `error`；取消走暂停态。
「已存在跳过」不新增状态：`done` + 汇总标记。

| 状态 | 卡片步骤组件 | 展示 |
| --- | --- | --- |
| `fetching` | `FetchStep` | 元数据抓取 |
| `downloadingAudio` | `AudioDownloadStep` | 不确定进度（音频下载无字节级进度） |
| `transcribing` | `TranscribeStep` | 块 i/n（整体百分比）+ 当前模型 |
| `translating` | `TranslateStep` | 块 i/n + 累计 token |
| `writing` | `TranslateStep`（复用） | 全部块完成、写中文稿中 |
| `done` | `MediaDoneStep` | 完成/跳过标记 |
| `error` | `MediaErrorStep` | 错误码本地化 + 重试 |

卡片动作：转录卡片只有 删除（= `group_cancel`）/ 外链 / 错误重试 / 文稿入口；无下载/暂停/恢复按钮。

### 页面

| 视图 | 行为 |
| --- | --- |
| `/`（首页） | 粘贴/拖放/`.txt`/`.csv` 导入/剪贴板 → `transcribe_start`，每个链接一张卡片，自动开始 |
| `/setup` | 一次 `transcription_probe` 展示必需/可选项；启动时自动探测 + 「重新检测」 |
| 设置 | 页签：转录 / 翻译 / 输出 / 网络 / 系统 / 关于；翻译页含 API 密钥（密码输入，随全局「保存」写入 stronghold，不回显）、baseUrl、模型、温度、并发、重试、术语表 |
| 详情 (`group/:groupId`) | 三 tab：英文原稿 / 中文译稿 / 日志；文稿页提供「打开输出目录」「用系统程序打开」；日志页提供「打开日志文件」 |

### 入口与门禁

- `whisperFound && ffmpegPath && ffprobePath` 任一缺失（且 probe 已加载）→ 首页输入与 Add 禁用，提交会 toast 并跳 `/setup`。
- 全局快捷键（Windows：`Alt+Shift+V`；macOS：`Ctrl+Shift+V`）= 从剪贴板入队；`Alt+Shift+D`/`Alt+Shift+Enter` 同义。
- 剪贴板监听（需开启 Watch clipboard）与文件导入同样走转录入队。

### 会话状态

`src/stores/transcription.ts`：probe、阶段、块/翻译进度、artifact 路径、token 用量、批次汇总；不持久化。
`src/stores/stronghold.ts`：`aiApiKeyDraft`/`aiApiKeyDirty` 承载密钥草稿；`setAiApiKey` 只写不回读（vault 未解锁时先 `stronghold_init`）。

### 已移除

下载向：`configure` 步骤与 `MediaConfigureStep`、体积展示、质量/字幕/过滤设置页、清晰度/编码/音轨参数 UI、播放列表选择 UI/弹窗。

## Boundaries

- 前端不读磁盘、不解析日志内容；跳过/已存在判定由后端返回。
- 事件监听统一在 `src/tauri/listeners/*` 并在 `plugins/tauriListeners.ts` 注册。

## Contracts

事件载荷见 `api-contract.md`；配置绑定见 `data-model.md`。

## Failure And Edge Cases

- 跳过路径不发 `media_add`：卡片由入队占位 group + `media_complete` 收尾。
- 播放列表落盘后（`processed === total`）前端拆成「一视频一卡片」，并按条目 URL 排序。
- 重启后详情页无可回放的产物路径（会话内状态）。

## Verification

Vitest（store/组件）+ Playwright E2E（`/setup`、入队与阶段渲染）；真实快捷键路径见 implementation-notes §21。
