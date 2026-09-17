---
status: current
layer: eval
domain: evals
canonical_for:
  - feature-checklist
related:
  - docs/current/shared/testing-strategy.md
last_verified: 2026-09-17
---

# 功能交付检查清单

用于新功能/功能变更在提 PR 前自检。逐项勾选，不适用的项要写明原因。

## 需求与设计

- [ ] 需求边界与 `docs/current/product/overview.md` 一致；新增能力已更新该文件。
- [ ] 若属复杂改动：`docs/changes/<feature>/` 已建（brief/design/tasks/verification）。
- [ ] `reviews/design-review.md` 结论为 `PASS`（或已获开发者 override）且记录在案。
- [ ] `verification.md` 的 AC 已逐条对照真实代码核对（引用的文件/字段/命令/事件存在），`acceptance_review_status: approved`。
- [ ] `design.md` 的 `End-to-End Data Interaction Flow` 与 `Frontend/Backend Fetching Logic` 已填（如适用）。

## 实现

- [ ] 领域边界正确（未把逻辑放到错误层，见 `rules/architecture.rules.md`）。
- [ ] 新命令/事件已三处同步：Rust handler、`src-isolation/main.ts` 白名单、E2E mock。
- [ ] 新设置项：Rust `Default`、前端 `defaultSettings`、设置页绑定、文档表格四处一致。
- [ ] 新诊断码：改 `diagnostic_rules.json` + 前端 i18n `errors.runner.<code>`。
- [ ] 未引入禁止项（`rules/forbidden.rules.md`）。

## 验证

- [ ] `npm run lint:fix` 无改动残留（或改动已提交）。
- [ ] `npm run test:unit` 通过；新增逻辑有对应用例。
- [ ] `npm run test:e2e` 通过；新增用户路径有覆盖或说明无法自动化的原因。
- [ ] `cd src-tauri && cargo fmt --all && cargo clippy --all-targets -- -D warnings && cargo test` 通过。
- [ ] `npm run build` 通过（含 `vue-tsc` 与隔离产物）。
- [ ] 手工验证：在 `npm run tauri dev` 中走一遍完整用户路径（入队 → 配置 → 下载 → 完成/失败）。
- [ ] 影响平台差异的改动已在目标平台（至少 Windows + 一个 Unix）验证或标注未验证。

## 文档与收尾

- [ ] 稳定事实已同步 `docs/current/`（含领域 `index.md` 与 `manifest.yaml`）。
- [ ] 相关领域 `verification.md` 的 Must-Not-Break 段已复核。
- [ ] `python docs/scripts/check_doc_runtime.py docs` 无 ERROR。
- [ ] change 目录已闭环（`design.md` 置 `implemented` + `landed_in`；worklog 循环行完整）。
- [ ] PR 描述包含：变更点、影响领域、验证命令与结果、风险与回滚方式。
