---
status: current
layer: rules
domain: coding
canonical_for:
  - coding-rules
related:
  - docs/current/rules/frontend.rules.md
  - docs/current/rules/backend.rules.md
last_verified: 2026-09-17
---

# 编码规则

## 格式化与静态检查（强制）

| 语言 | 命令 | 说明 |
| --- | --- | --- |
| TypeScript/Vue | `npm run lint:fix` | ESLint：`@stylistic`（2 空格、单引号、分号、1tbs）、`typescript-eslint` recommendedTypeChecked、`eslint-plugin-vue` flat/essential、`@intlify/vue-i18n` |
| Rust | `cargo fmt --all`（配置 `src-tauri/rustfmt.toml`：max_width 100、tab_spaces 2、Unix 换行、重排 import/mod） | 必须与 `cargo fmt --check` 一致 |
| Rust lint | `cargo clippy --all-targets -- -D warnings` | 警告即错误 |
| 类型 | `vue-tsc --noEmit`（`npm run build` 内含） | 类型错误会阻断构建 |

提交前顺序（根 `AGENTS.md`）：`npm run lint:fix` → `npm run test:unit` → `npm run test:e2e` → `npm run build`；
Rust 侧 `cargo fmt --all` → `cargo clippy`（-D warnings）→ `cargo test`。

## 通用

1. 不引入新依赖除非必要；新增依赖必须说明理由与许可证（AGPL-3.0 兼容）。
2. 不写"顺手重构"：改动范围与 change 的 design 一致；发现无关问题记录到文档/issue。
3. 不提交调试残留（`console.log`、`dbg!`、`println!`）；需要长期日志用 `tracing`/`console.warn`。
4. 不使用 `any`/`unknown` 断言绕过类型（已有 `as unknown as` 仅限 locale 类型镜像）；Rust 不用无条件 `unwrap()`（见架构规则例外）。
5. 新增代码必须有注释解释"为什么"，而不是"做了什么"；公共函数用 rustdoc 风格说明契约与失败语义。

## TypeScript

- 优先 `type`/`interface` 描述数据结构；枚举用 `enum` 或字面量联合，取值必须与 Rust serde 输出一致。
- 异步：`async/await`；允许显式 `void promise`（项目风格对 fire-and-forget 会加 `void` 前缀）。
- 错误处理：`try/catch` + `console.error` 或 toast；不得静默吞掉用户可见失败（除明确记录的例外）。
- 类型导入使用 `import type`。

## Rust

- 错误用 `Result<T, E>`；错误类型实现 `Display`（如 `YtdlpDownloadError`）。
- 命令返回 `Result<T, String>`；不可失败命令直接返回 `T`。
- 并发原语：跨 await 用 `tokio::sync`，纯短临界区用 `std::sync`；全局表用 `LazyLock<Mutex<...>>`。
- 避免 clone 大对象：配置用 `Arc<Config>` 快照。
- 平台差异用 `#[cfg(...)]` 显式分支，不引入运行时探测（除 `paths.rs` 的目录探测）。
- 测试与被测代码同文件（`#[cfg(test)] mod tests`）。

## 提交

- Conventional Commits：`type(scope): summary`（也接受 `type: summary`）；主体用祈使句描述影响。
- 一个提交一个意图；跨领域大改动拆分为可独立回滚的提交。

## Verification

```bash
npm run lint:fix && npm run test:unit && npm run build
cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test
```
