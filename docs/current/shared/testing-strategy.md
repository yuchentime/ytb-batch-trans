---
status: current
layer: shared
domain: testing-strategy
canonical_for:
  - testing-strategy
related:
  - docs/current/platform/build-test-lint.md
  - docs/evals/feature-checklist.md
last_verified: 2026-09-17
---

# 测试策略

## Purpose

规定不同改动类型应该补哪一层测试、mock 的写法与边界，避免"只加 E2E"或"只加 Rust 单测"造成盲区。

## Current Behavior

### 分层与职责

| 层 | 覆盖目标 | 不覆盖 |
| --- | --- | --- |
| Rust 单测（模块内） | 纯函数与状态机：参数矩阵、解析器、覆盖合并、模板渲染、调度器并发、路径解析 | IPC 序列化、真实进程、UI |
| Vitest（jsdom） | store 逻辑、helper 换算、组件渲染与交互（含表单校验） | 真实 IPC、真实进程、样式布局 |
| Playwright E2E | 前端编排与 UI 流程（入队→选择→下载→暂停→完成/失败），事件驱动状态 | 后端逻辑、真实下载、隔离模式 |
| 手工验收 | 一切依赖真实环境的行为：真实下载、托盘/快捷键/自启动、更新、跨会话持久化、跨平台差异 | — |

### 归位规则（改动 → 必测层）

| 改动 | 必测 |
| --- | --- |
| 新增/修改 yt-dlp 参数 | Rust 单测（`ytdlp_args/tests.rs` 断言 argv）+ 手工一次真实下载 |
| 新增解析规则或进度格式 | Rust 单测（行 → 事件）+ 前端单测（事件 → 状态） |
| 新增/修改 store 逻辑 | Vitest 单测（含边界：缺失 group、重复事件、失败分支） |
| 新增命令/事件 | Rust 侧无单测则至少 E2E mock + 手工 `tauri dev` 验证；必须更新 `tauriMock` |
| 新增设置项 | 前端类型 + 默认值 + 页面绑定 + Vitest（换算/序列化）+ 手工保存/重启 |
| 新增系统集成（托盘/快捷键/更新） | 手工验收（无自动化） |
| 文档/清单/CI | 无需测试，但必须跑 `check_doc_runtime.py` / 触发一次工作流 dry-run |

### mock 写法（单元与 E2E）

- 单元测试：`vitest.setup.ts` 每个用例前 `clearMocks()` + `installTauriMock({ ...mediaHandlers })` + `setActivePinia(createPinia())`。
  因此单测默认**只有 media 处理器**；需要其它命令时在该用例内重新 `installTauriMock` 合并对应 handlers。
- E2E：`main.ts` 在 `__E2E__` 时调用 `installTauriMock({...mediaHandlers, ...configHandlers, ...binaryHandlers, ...updateHandlers, ...strongholdHandlers})`。
- `tests/utils/tauriMock.ts` 提供 `IPCHandler` 类型与 `ensureInvokeArgsObject` 参数校验；校验失败会抛错（刻意设计：契约漂移要暴露）。
- 事件注入：通过 `window.E2E` 暴露的发射器逐条推送（见 `tests/e2e/utils/fixtures.ts`）。
- 未注册的命令会挂起，因此**新增 IPC 必须同步 mock**。

### 测试数据约定

| 数据 | 约定 |
| --- | --- |
| group/item id | 测试内可读字符串（`group-a`、`item-1`），不要求 uuid |
| URL | 使用示例域名，不指向真实服务 |
| 视频元数据 | 见 `tests/e2e/utils/mockData.ts` |
| Rust 全局状态 | dispatcher 测试使用唯一 group id，避免跨用例污染 |

### 覆盖率

- 前端：`coverage/units`（v8 provider）与 `coverage/e2e`（monocart）。
- 后端：`cargo llvm-cov`（CI）。
- Codecov 配置 `.codecov.yaml` 忽略部分目录（`src-tauri/**` 在 e2e 侧等）。覆盖率是趋势指标，不作为门禁。

### 测试的"必须不破坏"清单

- 每个领域的 `verification.md` 都有 Must-Not-Break 段；改动涉及这些语义时，测试必须显式覆盖或手工验证并在 change 的 `verification.md` 中记录 AC。

## Boundaries

- 不做快照测试（UI 结构变化频繁）。
- 不做视觉回归与像素对比。
- 不做性能基准；唯一的隐式性能约束是"读线程按字节读取"这类已知取舍。

## Contracts

- 交付前检查清单：`../evals/feature-checklist.md`、`../evals/bugfix-checklist.md`、`../evals/regression-matrix.md`。
- 命令与配置：`../platform/build-test-lint.md`。

## Failure And Edge Cases

- E2E 依赖 Vite dev server（`reuseExistingServer: !CI`）；本地端口 1420 被占用时会失败。
- Rust 单测依赖系统库；缺库环境无法运行（见 build-test-lint）。
- 测试中的时间相关逻辑（`partialDownload`、最近路径截断、速度缓存 `speedHoldMs`）需注入固定时间或容忍抖动，避免偶发失败。
- 组件测试若依赖 `useI18n()`，需 `import { i18n } from '../../src/i18n'` 并在 `mount(..., { global: { plugins: [i18n] } })` 中传入（`vitest.setup.ts` 只负责 Tauri mock 与 pinia）。

## Verification

```bash
npm run test:unit            # 前端单元
npm run test:e2e             # 前端 E2E（需要 Playwright 浏览器）
cd src-tauri && cargo test   # 后端单元
```
