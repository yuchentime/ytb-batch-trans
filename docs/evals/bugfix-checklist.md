---
status: current
layer: eval
domain: evals
canonical_for:
  - bugfix-checklist
related:
  - docs/current/shared/testing-strategy.md
last_verified: 2026-09-17
---

# 缺陷修复检查清单

## 诊断纪律（先做，不可跳过）

- [ ] 症状与原因已分离记录（症状：用户看到什么；原因：代码为什么这样）。
- [ ] 至少形成两个竞争假设，并写明各自的证据（日志/复现/真实运行输出）。
- [ ] 在改代码前已有可复现步骤或失败信号（不靠猜）。
- [ ] 未盲目照搬用户/他人提出的修复方案；若采纳，说明为何假设成立。
- [ ] 复现的是用户真实路径（不是测试里的简化路径）。
- [ ] 缺失的信号已主动补齐（日志级别、诊断码、临时 debug 输出后移除）。

## 定位

- [ ] 已确定缺陷所属领域（media-queue / download-engine / toolchain / settings-preferences / auth-secrets / app-lifecycle）。
- [ ] 已读该领域的 `flow.md` 与 `backend/frontend-behavior.md`，确认是"设计如此"还是"实现错误"。
- [ ] 若属"设计如此"，是否需要在 `current/` 文档中补充说明（避免再被当成 bug）。

## 修复

- [ ] 修复最小化：只改导致缺陷的路径，不做无关重构。
- [ ] 不通过放宽校验/吞掉错误来"修复"（例如把 fatal 降级为 warning）。
- [ ] 未破坏对应领域的 Must-Not-Break 语义。
- [ ] 若涉及并发/取消：验证没有引入进程泄漏或重复计数。

## 验证

- [ ] 新增回归测试（单测优先；无法自动化的写入手工步骤）。
- [ ] 复现步骤在修复后不再复现；相邻路径（同领域其它入口）也回归验证。
- [ ] `npm run test:unit`、相关 `npm run test:e2e`、`cargo test` 通过。
- [ ] `npm run lint:fix`、`cargo fmt/clippy` 无新增告警。

## 复盘

- [ ] 判断是否属可复用失败类（假设错误类型）：若是，写 `postmortems/YYYY-MM-DD-<slug>.md` 并指向具体护栏。
- [ ] 复发计数：同类问题第 2 次 → 加显式检查；第 3 次以上 → 加自动化/结构性护栏。
- [ ] 若缺陷暴露了文档缺失或错误，已同步修正 `docs/current/`。
- [ ] 提交信息使用 `fix:` 前缀并说明影响面。

## Verification

```bash
npm run test:unit && npm run test:e2e && npm run build
cd src-tauri && cargo test
```
