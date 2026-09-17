---
status: current
layer: domain
domain: settings-preferences
canonical_for:
  - settings-preferences-api-contract
related:
  - docs/current/shared/ipc-conventions.md
  - docs/current/domains/settings-preferences/data-model.md
last_verified: 2026-09-18
---

# settings-preferences 接口契约

## Scope

- 覆盖 6 个持久化命令与 1 个受设置驱动的系统命令（`notify`）。
- 所有命令都是同步的（`config_set` 内部触发异步副作用，但不等待完成）。
- patch 语义、字段表见 `data-model.md`。

## Conventions

- `get`/`set`/`reset` 都返回**完整的**对象（不是 patch 结果），前端用返回值整体覆盖内存镜像。
- `patch` 参数是任意 JSON 子树；`null` 表示显式置空，缺键表示不改。
- 失败统一返回 `Err(string)`（错误来自 `Box<dyn Error>` 的 `to_string()`）。

## config_get

- **功能职责**：读取当前完整设置（内存快照，不读磁盘）。
- **参数**：无。
- **返回**：`Config`（字段见 `data-model.md`）。

## config_set

- **功能职责**：把 patch 深合并进当前设置，触发运行时副作用并持久化，返回合并后的完整设置。
- **参数**：

| 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `patch` | `object` | 是 | 任意深度的设置子树 |

- **返回**：`Config`。
- **副作用**：见 `flow.md` 的"运行时副作用清单"。
- **失败**：`materialize` 失败（类型不符）→ 返回 `Err`，内存与磁盘都不变。

示例：

```json
// 只改并发与语言，不动其它字段
{ "performance": { "maxConcurrency": 4 }, "appearance": { "language": "zh-CN" } }

// 清空自定义下载目录
{ "output": { "downloadDir": null } }
```

## config_reset

- **功能职责**：把设置恢复为后端默认值（`Config::default()`）。
- **参数**：无。
- **返回**：`Config`（默认值）。
- **副作用**：触发 `on_updated`（并发回到默认、快捷键/托盘/语言/自启动按默认值调整）。

## preferences_get

- **功能职责**：读取当前完整偏好。
- **参数**：无。
- **返回**：`Preferences`。

## preferences_set

- **功能职责**：深合并 patch 并持久化。
- **参数**：

| 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `patch` | `object` | 是 | 偏好子树 |

- **返回**：`Preferences`。
- **副作用**：无（偏好不触发运行时行为，除窗口几何在下一次启动时生效）。

前端 `preferencesStore` 的实际调用形态：

```ts
// 追加一条最近路径（数组整体替换）
patch({ recents: { recent: { ...recent, [label]: list.slice(0, 5) } } });
// 记录窗口几何由后端 window.rs 防抖后调用 preferences_set
```

## preferences_reset

- **功能职责**：偏好恢复默认（目录为 `null`、最近使用清空、窗口几何归零）。
- **参数**：无。
- **返回**：`Preferences`。

## notify

- **功能职责**：按当前通知设置与窗口可见性决定是否弹出系统通知；标题/正文由后端 i18n 渲染。
- **鉴权/归属**：由前端在业务事件点调用（入队、完成、失败等），属于"受设置驱动的系统行为"。

| 名称 | 类型 | 必填 | 说明 |
| --- | --- | --- | --- |
| `kind` | `NotificationKind` | 是 | `queueAdded` / `queueDownloading` / `queueFinished` / `videoFinished` / `playlistFinished` / `videoReady` / `playlistReady` / `downloadFailed` |
| `params` | `Record<string, string> \| null` | 否 | i18n 插值参数（常用 `title`、`n`、`message`） |
| `force` | `boolean` | 是 | `true` 时跳过 `onBackground` 的窗口可见性检查（快捷方式触发的动作使用） |

- **返回**：`Result<(), string>`；被策略拦截时返回 `Ok(())`（静默）。
- **判定顺序**：`disabledNotifications` 含 `kind` → 返回；`notificationBehavior == never` → 返回；
  `onBackground` 且（窗口可见或聚焦）且 `!force` → 返回；否则弹通知。

- **平台实现**：Linux 用 `notify_rust`（图标 `open-video-downloader`），其它平台用 `tauri_plugin_notification`。
- **文案来源**：`notifications.<kind>.title` / `.body`（Rust 侧 locale 文件，缺 key 回退英文）。

## Failure And Edge Cases

- `notify` 在任何"不应弹"的情况下都返回成功，调用方无法区分"已弹出"与"被策略拦截"。
- 命令是同步的：`config_set` 返回时副作用可能还没跑完（信号量 resize 是异步的）。
- `preferences_set` 的并发调用可能相互覆盖（例如窗口防抖写入与用户点击保存同时发生）；后端不做合并锁，只按调用顺序覆盖。
- 前端若提交了后端不认识的额外键，会被 `materialize` 忽略（未在结构体中定义的字段不会保留）。
