---
status: current
layer: product
domain: product
canonical_for:
  - product-scope
related:
  - docs/current/product/glossary.md
  - docs/current/domains/transcribe-translate/index.md
  - docs/current/domains/media-queue/index.md
last_verified: 2026-09-18
---

# 产品概览

## Purpose

描述 Open Video Downloader（下称 OVD）是什么、为谁服务、当前版本提供哪些能力，以及明确不做什么。
所有领域文档都以本文的范围为边界。

## Current Behavior

- **形态**：跨平台桌面应用（Windows / macOS / Linux），Tauri v2 + Vue 3 前端 + Rust 后端，单窗口主界面 + 系统托盘。
- **版本**：`3.2.1`（`package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 三处保持一致）。
- **核心能力**：把一批 YouTube 链接交给内置 yt-dlp 只取音频，用本机 whisper（GPU）转录为英文原稿，再用 DeepSeek 翻译为中文译稿，产出两份 txt；不做视频文件下载、内容分发或云端中转。
- **维护的运行时依赖**：应用自带 yt-dlp 与 ffmpeg，通过签名清单自动下载与更新（见 `docs/current/domains/toolchain/`）。
- **界面语言**：12 种已注册的前端语言（de/en/es/fr/it/nb/nl/pt-BR/pt-PT/ru/tr/zh-CN）；后端（托盘/通知）同步提供同名语言包。
  已知缺口：`src/locales/ko.json` 与 `src-tauri/locales/ko.json` 已存在，但 `src/i18n.ts` 的 `availableLocales` 未注册 `ko`，因此韩文当前无法在设置中选择（后端 key 已可用）。
- **隐私**：除应用自更新、yt-dlp 清单、以及可选的 Sentry 崩溃/错误上报外，不向自建服务上传用户数据；下载请求直连目标站点（或用户配置的代理）。

### 主要用户任务

| 任务 | 入口 | 领域 |
| --- | --- | --- |
| 环境检查（whisper/CUDA/模型/ffmpeg/DeepSeek key） | `/setup` 页（启动自动探测 + 重新检测） | transcribe-translate |
| 粘贴/拖入/剪贴板/全局快捷键/CSV·TXT 导入链接并入队 | 顶栏输入框、拖放、剪贴板监听、快捷键 | transcribe-translate / media-queue |
| 转录与翻译（阶段进度、取消、重试、跳过） | 队列卡片 + 批次汇总 | transcribe-translate |
| 查看/打开两份 txt 与日志 | 详情页三 tab、`summary.md`、文件日志 | transcribe-translate / observability |
| 配置模型/设备/分块、DeepSeek key/术语表、输出目录 | 设置 → 转录/翻译/输出 | settings-preferences |
| 配置 Cookie / 账号密码 / 请求头 | 认证页 | auth-secrets |
| 更新应用与 yt-dlp/ffmpeg | 更新提示条、安装页 | app-lifecycle / toolchain |

## Boundaries

- OVD 是 yt-dlp 的图形外壳：站点支持范围、格式可用性、DRM/付费内容限制都由 yt-dlp 决定，应用不实现抓取逻辑。
- 不提供账号体系、云端同步、跨设备队列、团队协作；所有状态都在本机（应用数据目录 + 系统钥匙串）。
- 不提供视频/音频文件下载（Q15 覆盖式改造）；yt-dlp 仅用于取音频与元数据，交付物固定为两份 txt（无剪辑、无上传、无字幕文件交付）。
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
