---
status: current
layer: shared
domain: data-ownership
canonical_for:
  - data-ownership
related:
  - docs/current/platform/storage.md
  - docs/current/domains/settings-preferences/data-model.md
last_verified: 2026-09-17
---

# 数据归属

## Purpose

明确每一类数据"由谁拥有、存在哪、生命周期多长、谁能改"，避免状态被多处写导致不一致。

## Current Behavior

### 状态归属表

| 数据 | 拥有者（唯一写入方） | 存储位置 | 生命周期 | 消费方 |
| --- | --- | --- | --- | --- |
| 设置 `Config` | Rust `ConfigHandle` | `app_dir/config.store.json` | 持久 | 前端 settings store、调度器、yt-dlp 参数构造 |
| 偏好 `Preferences` | Rust `PreferencesHandle` | `app_dir/preferences.store.json` | 持久 | 下载路径模板、最近路径 UI、窗口恢复 |
| 机密 `AuthSecrets` | Rust `StrongholdState` | `app_dir/vault.hold` + 系统钥匙串 | 持久 | `with_auth_args` |
| 二进制版本表 | Rust `BinariesManager` | `bin_dir/metadata.json` | 持久 | `check`/`ensure` |
| 队列（Group/Item） | 前端 `media-group` store | 内存 | 会话 | 卡片、路由、下载组装 |
| item/group 状态 | 前端 `media-state` store | 内存 | 会话 | 卡片步骤、按钮可用性 |
| 组级选项与 override | 前端 `media-options` store | 内存 | 会话 | 下载组装 |
| 进度/阶段 | Rust 解析器 → 前端 `media-progress` store | 内存 | 会话（按 item 清理） | 卡片、底栏、窗口进度 |
| 目标路径 | Rust 解析器 → 前端 `media-destination` store | 内存 | 会话 | "打开所在位置" |
| 诊断/fatal | Rust 解析器 → 前端 `media-diagnostics` store | 内存 | 会话 | 卡片、日志页、上报 |
| 体积缓存 | 前端 `media-size` store（后端产出） | 内存 | 会话 | 配置步骤体积显示 |
| 分组日志 | Rust `LogStore` | 内存（环形缓冲） | 会话；`group_cancel`/删除时清理 | 日志页 |
| 分组运行状态与取消信号 | Rust `RUNNING_GROUPS` | 内存 | 会话（Cleanup 时移除） | 调度器 |
| 调度编号 | Rust `NumberingManager`（调度器内） | 内存 | 会话 | 文件名模板 |
| 应用更新对象/字节 | Rust `UpdateStore` | 内存 | 会话（安装后清空） | 更新流程 |
| 剪贴板监听开关与已见 URL | 前端 `watch-clipboard` store | 内存 | 会话 | 顶栏开关 |
| 窗口几何 | Rust `window.rs`（写 `Preferences`） | 持久 | 持久 | 启动恢复 |

### 单一写入方规则

1. **持久化数据只有 Rust 写**（`Config`/`Preferences`/`vault`/`metadata.json`）；前端通过命令间接修改。
2. **队列数据只有前端写**；后端不感知 Group/Item，只接收 `DownloadItem` 快照。
3. **进度/诊断类数据只有后端产生**；前端只做聚合与展示，不构造（唯一例外：`downloadGroup` IPC 失败时本地构造一条 fatal）。
4. **状态机只有 `media-state` store 写**；组件与其它 store 只能调用 `setState`/`setGroupState`。
5. **取消信号只有 `group_state` 模块写**（`ensure_group_running`/`cancel_group`）。

### 跨层数据流

```text
前端（内存）               后端（内存/持久）
Group/MediaItem  ──快照──▶  DownloadItem（一次性，不保存）
Options/Overrides ──patch─▶  resolve_with_patch(Config, override)
                            └─▶ yt-dlp argv
Config/Preferences ◀─命令─▶  JsonStoreHandle（持久）
AuthSecrets      ◀─命令─▶  StrongholdState（持久）
进度/诊断/日志/路径 ◀─事件─  解析器（内存）
```

### 生命周期与清理

| 触发 | 清理内容 |
| --- | --- |
| `deleteGroup` | state/progress/size/diagnostics/destination/options + `group_cancel`（含日志与调度队列） |
| `group_cancel` | 运行状态置 false、丢弃该 group 排队条目、删除分组日志 |
| 下载批次全部结束 | 后端计数器删除 + 调度器发 `Cleanup`（移除运行状态与队列） |
| 应用退出 | 全部内存数据（队列、进度、诊断、日志、编号、更新字节） |
| HMR（dev） | `deleteAllGroups()`（避免旧 store 与新代码混用） |

## Boundaries

- 没有跨进程/跨设备同步：单实例、单机、单用户。
- 没有数据库事务：持久化以文件为单位（`tauri_plugin_store` 的原子写）。
- 队列与进度不参与持久化，因此"崩溃恢复"不存在；yt-dlp 的文件级续传由 yt-dlp 自身决定。

## Contracts

- 目录与文件：`../platform/storage.md`。
- 字段定义：`../domains/settings-preferences/data-model.md`、`../domains/media-queue/data-model.md`。
- 命令/事件路径：`ipc-conventions.md`。

## Failure And Edge Cases

- 前端镜像与后端不一致的常见来源：`patch` 局部更新但前端未用返回值覆盖（约定要求整体覆盖）。
- 组级 override 是"差异集"，若被整体替换成完整快照，会失去"继承全局"的语义（见 media-queue/data-model）。
- 后端计数器以"下发条目数"为准：同一条目重复下发会导致计数不归零（`Cleanup` 不发出）。
- `LogStore` 的上限是全局字节数，单个 group 的长输出会挤掉其它 group 的日志。

## Verification

- 手工：下载 → 暂停 → 删除 group，确认前端各 store 与服务端 `RUNNING_GROUPS` 都没有残留（删除后再恢复该 group 不应出现事件）。
- `cargo test`：调度器清理路径（`Cleanup` 分支）由 dispatcher 测试间接覆盖。
