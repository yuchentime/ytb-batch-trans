---
status: current
layer: rules
domain: backend
canonical_for:
  - backend-rules
related:
  - docs/current/platform/backend-runtime.md
  - docs/current/domains/download-engine/backend-behavior.md
last_verified: 2026-09-17
---

# 后端规则

## 模块与命令

1. 一个命令一个文件，位于 `src-tauri/src/commands/<group>/<name>.rs`；`mod.rs` 负责重导出。
2. 新增命令必须三处同步：命令文件、`commands/mod.rs`、`lib.rs` 的 `generate_handler!`；前端可调用时再加 `src-isolation/main.ts` 白名单。
3. 命令体只做参数适配与状态获取，业务逻辑放 `scheduling`/`runners`/`binaries`/`state` 等模块。
4. 命令签名参数用 `snake_case`；返回 `Result<T, String>` 或 `T`；不在命令里做前端职责的校验。

## 状态管理

1. 持久化状态必须实现 `JsonBackedState`（`STORE_FILE`、`ROOT_KEY`、`default_value`、可选钩子）。
2. 只读快照用 `ArcSwap`（`load()` 返回 `Arc<T>`），不得在热路径加锁读配置。
3. 需要副作用的字段变更必须写在 `on_updated` 中，且必须幂等（可能被重复调用）。
4. 新增进程内全局状态必须用 `LazyLock<Mutex<...>>` 或 managed state，并在文档中登记清理时机。

## 并发与进程

1. 抓取/下载任务只能通过调度器入队；不得在命令或 runner 中直接 spawn。
2. 子进程必须走 `YtdlpRunner`（统一 PATH 前置、编码、隐藏窗口、进程组/Job Object）。
3. 需要取消的长任务必须订阅 `group_state` 的 watch 通道，并在取消时杀进程树。
4. 阻塞 IO/CPU 密集操作放 `spawn_blocking` 或独立线程；避免在 async 上下文里做文件遍历/解压。

## 解析与外部契约

1. yt-dlp 的参数构造集中在 `runners/ytdlp_args/*`，新增 flag 必须同时补 `tests.rs` 断言。
2. yt-dlp 输出解析集中在 `parsers/*`；新增识别逻辑优先改 `diagnostic_rules.json`，不在代码里散落字符串匹配。
3. 所有对外部数据（JSON、行文本、归档路径）的解析必须处理缺字段/异常形态，不得 panic。
4. 模板渲染必须保持"未知占位符原样保留 + 已替换值消毒路径分隔符"的语义。

## 错误与日志

1. 错误用自定义枚举 + `Display`；对外边界处转成 `String`。
2. 日志分级：`trace`（调度细节）、`debug`（命令与目录，仅 dev）、`info`（运行摘要、生命周期）、`warn`（可恢复失败）。
3. 不得在日志中输出凭据值；需要表达"是否使用认证"时只记录布尔摘要。
4. 只对"应用内部错误"上报 Sentry（白名单见 `download-engine/backend-behavior.md`），业务失败不上报。

## 测试

1. 纯函数与解析器必须带单测；参数构造必须有 argv 断言。
2. 涉及全局状态（调度器/计数器/分组状态）的测试必须使用唯一 id 并自行清理。
3. 平台相关代码用 `#[cfg]` 分支，测试覆盖可在任意平台运行的部分。

## Verification

```bash
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test
```
