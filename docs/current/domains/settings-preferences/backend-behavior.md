---
status: current
layer: domain
domain: settings-preferences
canonical_for:
  - settings-preferences-backend-behavior
related:
  - docs/current/platform/storage.md
  - docs/current/domains/settings-preferences/flow.md
last_verified: 2026-09-17
---

# settings-preferences 后端行为

## Purpose

说明持久化句柄的实现（合并、快照、写入）、副作用执行时机与窗口几何的写入策略。

## Current Behavior

### 状态句柄

| 类型 | 文件 | 存储键 |
| --- | --- | --- |
| `ConfigHandle = JsonStoreHandle<Config>` | `config.store.json` | `config` |
| `PreferencesHandle = JsonStoreHandle<Preferences>` | `preferences.store.json` | `preferences` |

`JsonStoreHandle<T>`（`src-tauri/src/state/json_handle.rs`）由三部分组成：

- `swap: Arc<ArcSwap<T>>`：无锁读的内存快照，`load()` 返回 `Arc<T>`（读路径零拷贝、零锁）。
- `store: Arc<Store<Wry>>`：`tauri_plugin_store` 的 JSON 文件句柄。
- `init`：读键 → `materialize`（默认值 + 深合并）→ `before_initialized` 钩子 → 立即写回（保证磁盘上出现完整结构）。

三个写路径：

| 方法 | 行为 |
| --- | --- |
| `apply_patch(patch)` | 当前值 → JSON → `json_merge` → `materialize` → `on_updated` → 写盘 → `swap.store` |
| `reset()` | `default_value()` → `on_updated` → 写盘 → `swap.store` |
| `load()` | 只读快照 |

注意顺序：**副作用先于写盘**。若 `on_updated` 抛错（当前实现不会），配置已生效但未持久化。

### 深合并（`json_state.rs`）

```text
json_merge(base, patch):
  两者都是 object -> 逐键递归
  其它            -> 直接用 patch 覆盖（数组/标量/null 都是整体替换）
```

`materialize` 先序列化 `default_value()`，再把持久化内容合并进去，最后反序列化为强类型结构。
因此：

- 新增字段不需要写迁移代码（旧文件自动补默认值）；
- 删除字段不会报错（未知键被忽略）；
- 类型错误（例如把字符串写到 `boolean` 字段）会让整个 `apply_patch` 失败并保持原状。

### 钩子实现

| 钩子 | `Config` | `Preferences` |
| --- | --- | --- |
| `before_initialized` | 推断 `network.enableProxy`；`output.downloadDir` 为空时填系统下载目录 | 无 |
| `on_updated` | 并发 resize、快捷键、托盘、i18n、自启动（见 `flow.md`） | 无 |

`Config` 实现位于 `src-tauri/src/state/config.rs`，`Preferences` 在 `state/preferences.rs`（仅声明常量）。

### 窗口几何（`src-tauri/src/window.rs`）

- `restore_main_window`：读 `preferences.window`；`maximized` 时用 `prevX/prevY`；尺寸 > 0 才应用；
  位置需通过"矩形与任一显示器相交"检查（避免显示器拔出后窗口消失）。
- `track_main_window`：监听 `Moved`/`Resized`，各生成局部 patch（`prevX/prevY` 随移动一起写），
  经 `PatchDebouncer`（300ms）合并后调用 `preferences.apply_patch`；`CloseRequested` 时 `flush_now`。
- 最大化状态下不记录位置与尺寸（避免把最大化尺寸写成普通尺寸）。
- Linux 特例：窗口获得焦点时把 `resizable` 切换 false→true，规避 KDE Wayland 下标题栏按钮失效的问题（tao#1046）。

### 关闭行为（`setup_close_behaviour`）

- 非 macOS：仅当 `system.trayEnabled` 为真时才拦截关闭；`closeBehavior == hide` → `prevent_close` + `hide`，`exit` → 放行。
- macOS：始终 `prevent_close` + `hide`（符合平台习惯），由托盘/菜单退出。
- 托盘关闭（`trayEnabled` 变为 false）由 `on_updated` 处理。

### i18n 与配置的耦合

`I18nManager` 在 `lib.rs` 中于 `ConfigHandle` 之后创建，初始 locale 取 `config.appearance.language`；
之后每次 `config_set` 都会调用 `set_locale`/`unset_locale`。缺 key 时回退英文（`FALLBACK_LOCALE = "en"`）。

### 事件驱动

设置变更**不发事件**：前端以命令返回值为准。唯一例外是窗口几何（后端自写自读，前端不感知）。

## Boundaries

- 两个 store 文件都是明文 JSON（不含密钥）。
- 不做配置版本迁移，只依赖"默认值 + 深合并"。
- 写入是"最后写入者获胜"，无并发保护与文件锁（单实例插件降低风险，但不消除窗口防抖写入与用户保存的竞态）。

## Contracts

- 字段与默认值：`data-model.md`。
- 文件路径：`../platform/storage.md`。
- 前台阅读顺序：`flow.md`。

## Failure And Edge Cases

- 磁盘上 JSON 损坏（非法 JSON）时 `Store::get` 返回原始值，`materialize` 会失败 → `init` 返回 `Err`，应用启动失败（`setup` 中 `?` 上抛）。这是配置损坏的最严重后果。
- `ArcSwap` 快照意味着正在进行的下载任务**不会**看到新的设置（例如中途改并发）：`YtdlpRunner::new` 每次运行都会重新 `load()`，因此新任务会看到新值。
- 窗口几何用物理像素，跨 DPI 显示器时可能出现尺寸不一致（依赖显示器相交检查兜底）。
- macOS 下 `closeBehavior` 配置无效（永远隐藏窗口）。

## Verification

见 `docs/current/domains/settings-preferences/verification.md`。
