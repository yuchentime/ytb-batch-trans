---
status: current
layer: product
domain: product
canonical_for:
  - product-scope
related:
  - docs/current/product/glossary.md
  - docs/current/domains/media-queue/index.md
last_verified: 2026-09-17
---

# 产品概览

## Purpose

描述 Open Video Downloader（下称 OVD）是什么、为谁服务、当前版本提供哪些能力，以及明确不做什么。
所有领域文档都以本文的范围为边界。

## Current Behavior

- **形态**：跨平台桌面应用（Windows / macOS / Linux），Tauri v2 + Vue 3 前端 + Rust 后端，单窗口主界面 + 系统托盘。
- **版本**：`3.2.1`（`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 三处保持一致）。
- **核心能力**：把 URL（单视频、播放列表、多站点）交给内置的 yt-dlp 完成下载，产出视频/音频文件，可选字幕、元数据、缩略图与 SponsorBlock 处理；不做内容解析、转码服务或云端中转。
- **维护的运行时依赖**：应用自带 yt-dlp 与 ffmpeg，通过签名清单自动下载与更新（见 `docs/current/domains/toolchain/`）。
- **界面语言**：12 种已注册的前端语言（de/en/es/fr/it/nb/nl/pt-BR/pt-PT/ru/tr/zh-TW）；后端（托盘/通知）同步提供同名语言包。
  已知缺口：`src/locales/ko.json` 与 `src-tauri/locales/ko.json` 已存在，但 `src/i18n.ts` 的 `availableLocales` 未注册 `ko`，因此韩文当前无法在设置中选择（后端 key 已可用）。
- **隐私**：除应用自更新、yt-dlp 清单、以及可选的 Sentry 崩溃/错误上报外，不向自建服务上传用户数据；下载请求直连目标站点（或用户配置的代理）。

### 主要用户任务

| 任务 | 入口 | 领域 |
| --- | --- | --- |
| 粘贴/拖入/剪贴板捕获 URL 入队 | 顶栏输入框、拖放、剪贴板监听、全局快捷键、CSV/TXT 导入 | media-queue |
| 选择播放列表条目并拆分/合并队列 | 队列卡片"播放列表选择"步骤 | media-queue |
| 调整分辨率、帧率、编码、音轨、字幕、SponsorBlock | 队列卡片 + 底部全局选择条 + 偏好页 | download-engine |
| 控制下载（开始/暂停/继续/删除/重试、全部开始） | 队列卡片操作列、底部队列菜单、托盘、快捷键 | media-queue |
| 配置下载位置与文件名模板 | 下载位置页、设置 → 下载 | settings-preferences |
| 配置 Cookie / 账号密码 / 请求头 | 认证页 | auth-secrets |
| 查看进度、日志、错误诊断并上报 | 卡片进度、媒体详情页（Metadata/Logs） | download-engine / observability |
| 更新应用与 yt-dlp/ffmpeg | 更新提示条、安装页 | app-lifecycle / toolchain |

## Boundaries

- OVD 是 yt-dlp 的图形外壳：站点支持范围、格式可用性、DRM/付费内容限制都由 yt-dlp 决定，应用不实现抓取逻辑。
- 不提供账号体系、云端同步、跨设备队列、团队协作；所有状态都在本机（应用数据目录 + 系统钥匙串）。
- 不负责媒体播放或转码流水线以外的编辑（没有剪辑、字幕翻译、上传功能）。
- 队列不落盘：应用退出后队列、进度、日志缓冲区全部丢弃（只有设置、偏好、密钥、二进制持久化）。
- 不承担法律合规判断；README 明确要求用户自行遵守所在地法律与平台条款。

## Contracts

- 用户可见功能与对应实现入口的映射见 `docs/current/product/user-journeys.md`。
- 术语一律以 `docs/current/product/glossary.md` 为准，跨文档不得混用同义词（例如 `Group` 不等同于 `Item`）。
- 能力变更必须同步更新本文件、`docs/current/product/acceptance.md` 与 `docs/index.md` 的领域表。

## Failure And Edge Cases

- 缺少 yt-dlp/ffmpeg 时应用会强制进入安装页（`/install`），安装失败允许部分成功并提示继续；此时下载必然失败。
- Microsoft Store 版、便携版（`ovd-portable` 目录存在）与 Snap 版不参与应用自更新；便携版与 MS Store 版还会使用不同数据目录。
- 直播（`is_live`）当前不支持，入队后直接产生 fatal 错误。
- 播放列表被站点限制为私有/不存在时，仅在日志与诊断中体现，不进入队列。

## Verification

- 上游功能清单：`README.md` 的 Features 段落。
- 领域级验收规则：`docs/current/product/acceptance.md`。
- 每个领域的可执行验证说明：`docs/current/domains/<domain>/verification.md`。
