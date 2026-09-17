---
status: current
layer: platform
domain: storage
canonical_for:
  - storage-layout
related:
  - docs/current/shared/data-ownership.md
  - docs/current/platform/env.md
last_verified: 2026-09-18
---

# 存储与环境形态

## Purpose

说明应用在磁盘上写了什么、写在哪里、以及四种运行形态（installed/portable/snap/MS Store）如何改变这些路径。
任何"用户数据找不到""便携版不更新"类问题都先查这里。

## Current Behavior

### 环境形态探测（`src-tauri/src/paths.rs`）

优先级：**snap > portable > microsoft-store > installed**。

| 形态 | 判定条件 | `app_dir` | `bin_dir` | 应用自更新 |
| --- | --- | --- | --- | --- |
| `snap` | 环境变量 `SNAP_USER_DATA` 非空 | `$SNAP_USER_DATA/<identifier>` | `$SNAP_USER_COMMON/bin`（为空则回落） | 禁用 |
| `portable` | 可执行文件同级目录存在 `ovd-portable/` | `<exe_dir>/ovd-portable` | `<exe_dir>/bin` | 禁用 |
| `microsoft-store` | `<exe_dir>/bin` 存在（且非 snap/portable） | `<app_data_dir>` | `<exe_dir>/bin` | 禁用 |
| `installed` | 以上都不满足 | `<app_data_dir>` | `<app_dir>/bin` | 允许 |

`app_data_dir` 由 Tauri 的 `app.path().app_data_dir()` 提供（Windows `%APPDATA%/<identifier>`、macOS `~/Library/Application Support/<identifier>`、Linux `~/.local/share/<identifier>`）。
`identifier = com.jelleglebbeek.youtube-dl-gui`（`tauri.conf.json`）。

启动时会 `tracing::debug!` 输出探测到的环境类型与两个目录，是排障的第一手信息。

### 数据目录内容

| 路径 | 内容 | 写入者 |
| --- | --- | --- |
| `<app_dir>/config.store.json` | 键 `config`：全部设置 | `ConfigHandle` |
| `<app_dir>/preferences.store.json` | 键 `preferences`：目录/最近使用/窗口几何 | `PreferencesHandle` |
| `<app_dir>/vault.hold` | stronghold 加密快照（凭据） | `StrongholdState` |
| `<app_dir>/webview/` | WebView2 用户数据（**仅 Windows 便携版**） | Tauri |
| `<bin_dir>/<tool>` / `<tool>.exe` | yt-dlp / ffmpeg 可执行文件 | `BinariesManager` |
| `<bin_dir>/metadata.json` | `{ versions, is_locked }` | `BinariesManager` |
| `<bin_dir>/<archive>` / `<archive>.tmp` | 下载中的归档与临时文件 | `BinariesManager` |
| 用户下载目录 / 自定义目录 | 下载产物（视频/音频/字幕/缩略图） | yt-dlp |

系统位置（不属应用目录）：

| 位置 | 内容 |
| --- | --- |
| 系统钥匙串：service `com.jelleglebbeek.youtube-dl-gui`，account `master_key` | 保险库主密钥（base64 的 32 字节） |
| 系统下载目录 | `output.downloadDir` 的首次默认值 |
| 自启动项（LaunchAgent / 注册表 / systemd） | `--auto-start` 启动项（由 autostart 插件管理） |

### 不落盘的数据

| 数据 | 生命周期 |
| --- | --- |
| 下载队列（Group/Item）、状态机、进度、体积缓存、诊断、目标路径 | 应用退出即丢失 |
| 分组日志环形缓冲 | 进程内（超出上限丢弃最旧行；`group_cancel` 时清理该 group） |
| 调度器编号（`autonumber`）与运行状态表 | 进程内 |
| 更新下载的字节 | 进程内（`UpdateStore.bytes`） |

### 用户可手工干预的文件

| 操作 | 效果 |
| --- | --- |
| 删除 `config.store.json` | 设置回默认（下载目录重新填系统下载目录） |
| 删除 `preferences.store.json` | 偏好回默认（窗口尺寸回 800×900） |
| 删除 `metadata.json` | 二进制版本表清空 → 下次启动重装全部工具 |
| 改 `metadata.json` 的 `is_locked = true` | 跳过二进制检查与安装（离线/锁定场景） |
| 删除 `vault.hold` | 凭据丢失；若钥匙串仍有主密钥，下次 `stronghold_init` 会用该密钥新建快照 |
| 删除 `bin_dir` 内容 | 下次启动重新下载（除非 `is_locked`） |
| 放入 `ovd-portable/` 目录 | 切换为便携形态（数据与二进制都在应用旁） |

### 备份建议（用户视角）

需要备份的是：`config.store.json`、`preferences.store.json`、`vault.hold`（+ 系统钥匙串中的主密钥）。
`bin/` 与 WebView 数据可重新生成，不必备份。

### 转录输出目录（`output.rootDir`，默认 `<系统下载目录>/ovd-transcripts`）

| 路径 | 内容 | 生命周期 |
| --- | --- | --- |
| `<root>/<消毒后的标题>/transcript.en.txt`、`transcript.zh.txt` | 交付物（UTF-8 无 BOM + LF） | 永久；`overwrite=false` 时存在即跳过 |
| `<root>/<消毒后的标题>/.work/` | 中间产物：`audio.<ext>`、`chunks/`、`segments.json`、`zh.blocks.json`、`source.json` | 成功写双 txt 后删除音频（`keepAudio=true` 保留）；失败/取消保留现场 |
| `<root>/summary.md` | 批次汇总（表格 + Totals） | 每次批次结束时覆盖写 |
| `<app_dir>/logs/transcribe.log`(+`.1`…`.4`) | 文件日志（5MB × 5） | 轮转自管理，见 `observability.md` |

## Boundaries

- 没有迁移机制：更换 identifier 或目录形态（例如从安装版改成便携版）不会搬运旧数据。
- 不写注册表（除自启动与 Windows 卸载信息，由安装器/插件管理）。
- 日志不落文件（只有进程内缓冲与 stdout）。

## Contracts

- 归属与生命周期规则：`../shared/data-ownership.md`。
- 配置/偏好字段：`../domains/settings-preferences/data-model.md`。
- 二进制元数据：`../domains/toolchain/backend-behavior.md`。
- 相关环境变量：`env.md`。

## Failure And Edge Cases

- MS Store 版把 `bin_dir` 放在安装目录（`<exe_dir>/bin`），该目录可能不可写，导致二进制安装失败。
- Snap 版 `bin_dir` 在 `$SNAP_USER_COMMON/bin`，与 `app_dir` 不同盘符/不共享；删除 snap 数据时两者可能不同步。
- 便携版忽略系统下载目录默认值以外的差异，但 `app_dir` 旁的 `webview/` 目录随 Windows 便携场景一起移动（体积可能较大）。
- 配置 JSON 损坏会让应用无法启动（唯一需要用户手工删文件的故障）。
- `vault.hold` 与钥匙串主密钥必须成对存在：只丢一个都会走"静默重建"（凭据丢失）。

## Verification

- `cd src-tauri && cargo test`（`paths.rs` 的 6 个目录解析单测覆盖 snap/portable/ms-store/默认）。
- 手工：切换形态（放 `ovd-portable/`）后启动，观察日志中的环境类型与目录，并确认设置/偏好/二进制位置随之变化。
