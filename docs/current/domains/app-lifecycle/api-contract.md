---
status: current
layer: domain
domain: app-lifecycle
canonical_for:
  - app-lifecycle-api-contract
related:
  - docs/current/shared/ipc-conventions.md
  - docs/current/domains/settings-preferences/api-contract.md
last_verified: 2026-09-17
---

# app-lifecycle 接口契约

## Scope

- 本领域拥有的命令：`app_ready`、`get_platform`、`updater_check`、`updater_download`、`updater_install`。
- 共享命令：`notify`（设置驱动，详见 `../settings-preferences/api-contract.md`）。
- 本领域发出的事件：`navigate`、`shortcut_action`、`updater_download_progress`、`updater_finished`、`updater_error`。

## app_ready

- **功能职责**：前端初始化完成后通知后端"可以显示窗口"，并处理自启动最小化。
- **参数**：无。
- **返回**：无（`()`）。
- **行为**：读取进程参数；若 `system.autoStartMinimised` 且存在 `--auto-start` 则**不显示**窗口并返回；
  否则 `show()`（macOS 额外 `set_focus()`）。
- **前置**：主窗口必须存在，否则 `unwrap()` panic（仅配置错误时会触发）。

## get_platform

- **功能职责**：返回运行平台，供前端做条件渲染（快捷键文案、路径提示等）。
- **参数**：无。
- **返回**：`"windows" | "macos" | "linux" | "unknown"`（编译期 `cfg!` 判定）。

## updater_check

- **功能职责**：检查应用是否有新版本，并把可更新对象缓存在后端。
- **参数**：无。
- **返回**：

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `available` | `boolean` | 是否存在可用更新 |
| `currentVersion` | `string` | 当前版本（`app.package_info().version`） |
| `availableVersion` | `string?` | 可用版本；无更新时缺省 |

- **行为细节**：
  - `is_microsoft_store_app`/`is_portable_app`/`is_snap_app` 任一为真 → 直接返回 `available=false`（不发请求）。
  - 命中更新时把 `Update` 存入 `UpdateStore` 并把 `bytes` 清空。
- **失败**：`Err(updater error)`，前端 `updaterStore.check()` 由 `App.vue` 的 try/catch 吞掉。

## updater_download

- **功能职责**：下载已发现的更新到内存。
- **参数**：无。
- **返回**：`Ok(())`；无可用更新时 `Err("no update available")`。
- **事件**：`updater_download_progress{received, total}`（每个 chunk）；失败时 `updater_error{message}`。

## updater_install

- **功能职责**：安装已下载的更新并请求重启。
- **参数**：无。
- **返回**：`Ok(())`；缺更新或字节时 `Err("no update available")` / `Err("no update downloaded")`。
- **事件**：成功安装后 `updater_finished`，随后 `app.request_restart()`。
- **副作用**：`bytes` 被 `take()`（不能重复安装）。

## 事件：navigate

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `route` | `string` | vue-router 的 `name`；当前只由 macOS 设置菜单发送 `"settings"` |

前端处理：`router.push({ name: payload.route })`（未知名称会报路由警告）。

## 事件：shortcut_action

| 字段 | 类型 | 说明 |
| --- | --- | --- |
| `action` | `"media_add" \| "media_add_and_download" \| "download_all"` | 动作类型 |

来源：全局快捷键与托盘菜单（两者语义一致）。前端处理：

| action | 前端行为 |
| --- | --- |
| `media_add` | 读剪贴板 → 校验 URL → `dispatchMediaInfoFetch(url, fromShortcut=true)` |
| `media_add_and_download` | 同上 → `addAndDownload(url, true)` |
| `download_all` | `downloadAllGroups(true)` |

非法 URL（`isValidUrl` 失败）静默忽略，不提示。

## 事件：updater_*

| 事件 | 载荷 | 前端行为 |
| --- | --- | --- |
| `updater_download_progress` | `{ received: number, total: number \| null }` | `Object.assign(downloadProgress)` |
| `updater_finished` | 无 | 重置 `isUpdating`、置 `isIgnored=true`、进度归零 |
| `updater_error` | `string`（错误原文） | 重置状态并写 `lastError` |

## 共享命令：notify

见 `../settings-preferences/api-contract.md`。本领域补充平台事实：

| 平台 | 实现 | 备注 |
| --- | --- | --- |
| Linux | `notify_rust::Notification`（icon `open-video-downloader`） | 关闭回调在独立线程中等待，避免句柄泄漏 |
| Windows/macOS | `tauri_plugin_notification` | 需要系统通知权限（capability `notification:default`） |

## Failure And Edge Cases

- `app_ready` 是"显示窗口"的唯一开关；前端若在 `initStores()` 中抛出未捕获异常，窗口会保持隐藏（表现为"启动后无界面"）。
- `updater_check` 在 portable/snap/MS Store 下不发起网络请求，因此这些形态下"检查更新"永远显示最新。
- `navigate` 的 `route` 是 vue-router 名称（不是路径）；新增菜单项必须使用已注册的路由名。
- `shortcut_action` 没有窗口聚焦判断：应用在后台时也会响应全局快捷键（符合预期）。
