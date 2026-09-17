---
status: current
layer: rules
domain: architecture
canonical_for:
  - architecture-rules
related:
  - docs/current/platform/backend-runtime.md
  - docs/current/shared/data-ownership.md
last_verified: 2026-09-17
---

# 架构规则

## 分层

1. **前端只做编排与展示**：状态机、表单、列表、路由、i18n、系统调用编排。不实现抓取、不解析 yt-dlp 输出、不生成下载参数。
2. **后端只做能力**：进程执行、并发调度、解析、持久化、加密、系统集成。不感知 UI 状态与队列结构（只接收快照）。
3. **唯一数据源**：持久化数据只由 Rust 写（`Config`/`Preferences`/`vault`/`metadata.json`）；会话内队列只由前端写。
4. **领域边界**：媒体队列（media-queue）、下载引擎（download-engine）、工具链（toolchain）、设置（settings-preferences）、
   凭据（auth-secrets）、外壳与更新（app-lifecycle）。跨领域调用必须走命令/事件，不得直接读写对方的 store 或模块内部状态。
5. **共用能力上移**：两个及以上领域需要的纯逻辑放到 `src/helpers/`（前端）或 `src-tauri/src/parsers|runners|models`（后端），不得复制实现。

## 依赖方向

```
Vue 组件  ->  stores  ->  src/tauri（IPC 封装）  ->  Tauri 命令
Vue 组件  ->  composables / helpers（纯函数）
Rust 命令 ->  scheduling / runners / binaries / state / stronghold
scheduling -> runners -> parsers / models
```

禁止反向依赖：组件不得直接 `invoke`（除既有的日志订阅与外链打开）、命令不得依赖前端概念（`MediaState`、`Group`）。

## 调度与并发

1. 所有抓取/下载任务必须经 `GenericDispatcher` 排队；禁止在命令里直接 spawn yt-dlp。
2. 并发上限只能来自 `DynamicSemaphore`（受 `performance.maxConcurrency` 控制）；不得使用额外的信号量或 sleep 节流。
3. 取消必须通过 `group_state` 的取消通道；不得用"丢弃事件"的方式假装取消。
4. 长耗时阻塞操作必须 `spawn_blocking` 或独立线程；不得阻塞 tokio 工作线程。

## 状态与生命周期

1. 新增进程内状态必须说明清理时机（谁在什么时候移除），并优先复用现有容器（`RUNNING_GROUPS`、计数器、`LogStore`）。
2. 新增持久化字段必须走 `JsonBackedState`（深合并 + 默认值），不得自己读写 JSON 文件。
3. 事件载荷必须携带 `id`/`groupId`（或等价的归属标识），否则前端无法归属。

## 安全

1. 前端可调用的命令必须在隔离白名单中；新增命令必须同时更新白名单与 capability（如需新插件）。
2. 外部输入（URL、模板、ffmpeg 参数）一律作为参数传递，不得拼接 shell。
3. 机密只走 stronghold + 系统钥匙串；日志/事件/Sentry 中不得出现明文（见 `security.rules.md`）。
4. 远端内容（清单、归档）必须签名/哈希校验后才使用。

## 变更与文档

1. 复杂改动（多循环、跨领域、影响共享契约）必须先写 `docs/changes/<feature>/design.md` 并通过 Design Review 才能改代码。
2. 落地的业务事实必须同步回 `docs/current/`，不得只留在 change 目录或 commit message 中。

## 例外与豁免

- 现有 `media_download` 的 `unwrap()`、`lib.rs` 中的 `expect/panic` 属于已知债务：**不得扩散**（新增代码不允许同类写法）。
- `DiagnosticRules` 是唯一允许"配置驱动字符串匹配"的地方。

## Verification

- 结构性验证靠 review（架构、数据归属、依赖方向）。
- 可执行验证：`cargo clippy --all-targets -D warnings`（依赖与死代码）、`npm run lint`、`npm run build`。
