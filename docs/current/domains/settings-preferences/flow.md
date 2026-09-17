---
status: current
layer: domain
domain: settings-preferences
canonical_for:
  - settings-preferences-flow
related:
  - docs/current/domains/settings-preferences/data-model.md
  - docs/current/domains/settings-preferences/backend-behavior.md
last_verified: 2026-09-18
---

# settings-preferences 流程

## Purpose

描述设置/偏好从落盘到生效的完整路径，特别是"写入之后会触发哪些系统副作用"。

## Current Behavior

```mermaid
flowchart TD
  A[应用启动 setup] --> B[ConfigHandle::init]
  A --> C[PreferencesHandle::init]
  B --> B1[读 config.store.json 的 config 键]
  B1 --> B2[与服务端 Default 深合并 -> materialize]
  B2 --> B3[before_initialized: 推断 enableProxy, 填默认下载目录]
  B3 --> B4[写回 store + ArcSwap]
  C --> C1[读 preferences.store.json 的 preferences 键]
  C1 --> C2[深合并 -> materialize -> 写回]
  D[前端 main.ts initStores] --> E[preferencesStore.load -> preferences_get]
  D --> F[settingsStore.load -> config_get]
  F --> G[applySettings: locale + lang 属性]
  H[用户点击保存] --> I[settingsStore.patch(draft)]
  I --> J[config_set -> apply_patch]
  J --> K[json_merge 到当前值 -> materialize -> on_updated]
  K --> L[resize 两个信号量 / 注册或注销快捷键 / 建销托盘 / 切换 i18n locale / 启停 autostart]
  J --> M[写 store + ArcSwap 更新]
  M --> N[返回新 Config -> 前端 applySettings]
  O[窗口移动/缩放] --> P[300ms 防抖 -> preferences_set 局部 patch]
```

### 关键节点

| 节点 | 输入 | 职责 | 输出/状态变化 | 边界 |
| --- | --- | --- | --- | --- |
| `JsonStoreHandle::init` | `tauri_plugin_store` 文件 | 读取持久化 JSON、与默认值深合并、执行 `before_initialized`、立即写回 | `ArcSwap<T>` + store 文件 | 文件损坏/缺字段时自动补默认值，不报错 |
| `apply_patch` | 任意 JSON patch | 深合并 → 反序列化 → `on_updated` → 写盘 → 更新内存 | 返回新值给前端 | 反序列化失败会中断，内存与磁盘都不变 |
| `on_updated`（Config） | 新 `Config` | 同步运行时副作用 | 信号量、快捷键、托盘、语言、自启动 | 副作用失败只记录 warning，不回滚配置 |
| `reset` | 无 | 用 `Default` 覆盖并触发 `on_updated` | 全量默认值 | 不重置 preferences（另有接口） |
| 窗口几何写入 | `window.rs` 事件 | 300ms 防抖后 patch `preferences.window` | `preferences.store.json` | 关闭窗口时 `flush_now` 强制写入 |

### 前端加载时序（`src/main.ts`）

1. `preferencesStore.load()`（失败只 `console.error`）。
2. `settingsStore.load()`；若 `appearance.theme !== 'system'` 则 `applyTheme(theme)`；按 `appearance.language`（经 `resolveLocale()`）设置 i18n locale 与 `<html lang>`。
3. 两者都结束后 `invoke('app_ready')`（此时窗口才显示）并挂载 Vue。

### 写入语义（深合并）

`config_set`/`preferences_set` 接收的是**完整对象或任意子树**，后端按 JSON 递归合并：

- 对象递归合并（未出现的键保持现值）；
- 数组**整体替换**（例如 `subtitles.languages`、`sponsorBlock.removeParts`、`recents.recent[label]`）；
- `null` 会覆盖为 `null`（因此"清空某字段"用 `null`，"不改"用"不传该键"）。

设置页保存时提交的是整份 `draft`（`structuredClone(settings)`），因此**任何未在 UI 暴露的字段也会被原样回写**——
新增字段若只在后端有默认值、前端类型没跟上，保存会被前端覆盖为 `undefined`。

### 运行时副作用清单（`Config::on_updated`）

| 字段 | 副作用 |
| --- | --- |
| `performance.maxConcurrency` | 异步 `resize` DownloadLimiter 与 FetchLimiter |
| `input.globalShortcuts` | `register_shortcuts`（true）或 `unregister_shortcuts`（false） |
| `system.trayEnabled` | `create_tray` / `destroy_tray`（幂等） |
| `appearance.language` | `i18n.set_locale`（否则 `unset_locale` 走系统语言）；若托盘开启会重建托盘以刷新文案 |
| `system.autoStartEnabled` | 调 `autolaunch().enable()/disable()` |
| 其它字段 | 无副作用，仅在下一次使用时被读取 |

`system.trayEnabled` 与 `system.closeBehavior` 的组合生效点见 `../app-lifecycle/backend-behavior.md` 的关闭行为。

### 重置（reset）

- `config_reset`：用 `Config::default()` 覆盖并触发 `on_updated`；前端 `SettingsView` 的"重置"按钮调用。
- `preferences_reset`：用 `Preferences::default()` 覆盖，**不触发**额外副作用（窗口几何恢复默认只在下次启动生效）。

### 最近使用（Recents）

- 结构：`recents.recent: { [label]: string[] }`，`IndexMap` 保序。
- 前端 `addRecentPath(label, path)`：去重后 `unshift`，截取前 5 条，再整体 `patch`。
- `clearRecentPaths(label)` 置空数组。
- label 由界面语义决定（例如视频/音频目录选择器各自一个 label）。

## Boundaries

- 只有两个存储文件：`config.store.json` 与 `preferences.store.json`；不含密钥（auth-secrets）与队列（不持久化）。
- 前端 store 是内存镜像，保存成功后用后端返回值整体覆盖（不做乐观更新）。
- 设置页的"草稿"只在组件内，未保存时切换页面会丢失（无提醒）。

## Contracts

- 字段与默认值：`data-model.md`。
- 命令与错误：`api-contract.md`。
- 文件路径与 portable/snap 差异：`../platform/storage.md`。

## Failure And Edge Cases

- 前端提交 `undefined` 字段时 JSON 序列化会丢键，因此不会破坏已有值；但提交 `null` 会覆盖。
- `on_updated` 中 i18n/tray 逻辑会在每次任意配置保存时都执行一遍（即使语言没变），因此保存设置可能引起托盘重建闪烁。
- `before_initialized` 只在首次加载时执行：`enableProxy` 为 `null` 时按 `proxy` 是否非空推断；`downloadDir` 为 `null` 时填系统下载目录。之后不会自动纠正用户清空的值。
- 窗口几何写入使用物理像素；跨显示器/缩放变化时 `restore_main_window` 会校验矩形是否落在某个显示器内，否则跳过位置恢复。

## Verification

见 `docs/current/domains/settings-preferences/verification.md`。
