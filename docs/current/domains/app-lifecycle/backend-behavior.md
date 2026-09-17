---
status: current
layer: domain
domain: app-lifecycle
canonical_for:
  - app-lifecycle-backend-behavior
related:
  - docs/current/platform/backend-runtime.md
  - docs/current/platform/storage.md
last_verified: 2026-09-17
---

# app-lifecycle 后端行为

## Purpose

说明 `lib.rs` 的装配顺序与各系统集成模块的实现边界，供修改启动时序/系统集成时评估影响面。

## Current Behavior

### 插件与 managed state（`lib.rs`）

插件（顺序即注册顺序）：`notification`、`autostart::Builder`、`updater`、`store`、`dialog`、`clipboard-manager`、
`keyring`、`shell`、`opener`、`global-shortcut`、`autostart::init(LaunchAgent, ["--auto-start"])`、`single-instance`。

`setup` 中按顺序 manage 的状态：

| 顺序 | 状态 | 用途 |
| --- | --- | --- |
| 1 | `ClientInitGuard`（Sentry） | 保持 Sentry 客户端存活 |
| 2 | `PathsManager` | 数据/二进制目录与环境形态 |
| 3 | `Arc<ConfigHandle>` | 设置快照 |
| 4 | `Arc<PreferencesHandle>` | 偏好快照 |
| 5 | 主窗口（`create_main_window`） | 见下 |
| 6 | `I18nManager` | 内嵌后端 locale |
| 7 | `TrayState` | 托盘句柄 |
| 8 | `Mutex<UpdateStore>` | 更新对象与字节缓存 |
| 9 | `LogStoreState` | 分组日志环形缓冲 |
| 10 | `DownloadLimiter` / `FetchLimiter` | 两个 `DynamicSemaphore`（初值 = `maxConcurrency`） |
| 11 | `FetchSender` / `DownloadSender` | 两个调度器通道 |
| 12 | `BinariesState` / `BinariesManager` | 二进制安装 |
| 13 | `StrongholdState` | 保险库 |

监听/钩子注册：`restore_main_window`、`track_main_window`、`setup_close_behaviour`、`create_tray`、
`init_autostart`、`setup_menu`、`register_shortcuts`。

`invoke_handler` 注册 26 个命令（`app_ready`、media/scheduling、logging、config、preferences、binaries、
updater、stronghold、platform、notify）。新增命令必须同时加入此处与 `src-isolation/main.ts` 白名单，否则前端调用会被隔离钩子拦截。

### 窗口创建（`create_main_window`）

- 使用 `tauri.conf.json` 的 `app.windows[0]` 配置（800×900，最小 750×650，`visible: false`，`dragDropEnabled: false`）。
- Windows 便携版：`data_directory(app_dir/webview)`（WebView2 数据与便携目录一起走）。
- 其它平台忽略 `paths` 参数（`#[cfg(not(windows))]`）。

### 托盘（`tray.rs`）

- `create_tray` 幂等（`TrayState.tray` 已存在即返回），且仅在 `system.trayEnabled` 为真时创建。
- 菜单文案通过 `I18nManager.t()` 渲染；快捷键提示文案随平台。
- 托盘事件：`quit` → `app.exit(0)`；`hide_toggle` → 显示/隐藏主窗口；其余三项发 `shortcut_action`。
- `destroy_tray` 按 id 移除；配置切换时由 `on_updated` 调用。

### 菜单（`menu.rs`）

- macOS：App 子菜单（关于元数据：名称/版权/AGPL-3.0/官网 + 设置 ⌘, + 服务/隐藏/退出）与 Edit 子菜单；
  设置项发 `navigate{route:"settings"}`。
- Windows/Linux：空菜单（保留函数以便未来扩展）。
- 三个平台函数都有 `#[allow(dead_code)]`，因此不同平台编译时不会告警。

### 快捷键（`commands/shortcuts.rs`）

- 组合键由 `shortcut_mods()` 决定：macOS `Control|Shift`，其它 `Alt|Shift`。
- 注册前去重（`gs.is_registered`），失败仅 `tracing::warn`。
- 回调只在 `ShortcutState::Pressed` 时发事件（避免按下/抬起双触发）。
- `unregister_shortcuts` 用 `unregister_all()`（会注销所有全局快捷键，包括未来新增的）。

### i18n（`i18n.rs`）

- `include_dir!("./locales")` 把后端 locale 编进二进制（无需运行时资源文件）。
- `set_locale`：规范化代码 → 精确匹配 → 退化为语言主码 → 失败返回 false（不改变当前 locale）。
- `unset_locale()`：按系统语言重新解析（用于 `appearance.language == "system"`）。
- `t(key)`：当前 locale 查不到时回退 `en`。
- `t_with(key, params)`：支持 `{name}` 占位符替换（通知参数）。

### 日志与追踪（`init_tracing`）

- `fmt` 层：debug 构建 `DEBUG`，release 为 `INFO`；屏蔽 `tauri_plugin_updater`、`tao` event loop、`h2`、`hyper_util` 噪声。
- Sentry 层：`traces_sample_rate = 0.05`、`sample_rate = 0.25`、release 名来自 crate 版本。
- 前端 Sentry 在 `src/sentry.ts` 单独初始化（见 `../platform/observability.md`）。

### 环境形态影响

| 形态 | 判定 | 影响 |
| --- | --- | --- |
| snap | 存在 `SNAP_USER_DATA` | `app_dir = $SNAP_USER_DATA/<identifier>`，`bin_dir = $SNAP_USER_COMMON/bin`，禁用应用自更新 |
| portable | 可执行文件旁存在 `ovd-portable/` | `app_dir = exe_dir/ovd-portable`，`bin_dir = exe_dir/bin`，禁用自更新，WebView 数据在 `app_dir/webview`（仅 Windows） |
| microsoft-store | `exe_dir/bin` 存在 | `bin_dir = exe_dir/bin`，禁用自更新 |
| installed | 以上都不满足 | `app_dir = app_data_dir`，`bin_dir = app_dir/bin`，允许自更新 |

snap 优先于 portable（同时命中时按 snap 处理）。

## Boundaries

- 启动流程中没有"健康检查"或"安全模式"：任何 manage/setup 失败（除少数被忽略的 warning）都会让应用启动失败。
- 托盘与菜单不提供下载队列的细粒度控制（只有三个动作与显示/退出）。
- 更新检查不会自动重试，也不做后台定时轮询（每次启动一次 + 手动）。

## Contracts

- 命令/事件：`api-contract.md`。
- 状态归属与环境形态：`../platform/storage.md`、`../shared/data-ownership.md`。
- 隔离白名单：`../platform/frontend-runtime.md`。

## Failure And Edge Cases

- `ConfigHandle::init` 失败会让 `setup` 返回错误（`?`），应用直接退出；这是配置文件损坏的最终表现。
- `create_main_window` 依赖 `app.config().app.windows.first()`，配置缺失会 panic（`expect`）。
- `register_shortcuts` 在启动时调用一次；若此时配置为禁用则不会注册，`on_updated` 打开后立即注册。
- `unregister_all()` 的宽泛语义意味着"关闭快捷键"会注销全部已注册快捷键；如果未来有第三方插件注册快捷键，会被一并注销。
- `app.exit(0)` 不等待窗口几何 flush（`CloseRequested` 不会被触发），因此通过托盘退出时最后一次移动可能未持久化（最多丢失最近 300ms 的几何变化）。
- Linux 通知实现使用 `notify_rust`，在缺少通知守护进程的环境会返回错误（`notify` 命令返回 `Err`，前端只 warn）。

## Verification

见 `docs/current/domains/app-lifecycle/verification.md`。
