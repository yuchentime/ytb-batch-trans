---
status: current
layer: rules
domain: documentation
canonical_for:
  - documentation-rules
related:
  - docs/README.md
  - docs/manifest.yaml
last_verified: 2026-09-17
---

# 文档治理规则

## 目录职责

| 目录 | 职责 | 默认加载 |
| --- | --- | --- |
| `docs/index.md` / `docs/manifest.yaml` | 人类路由图 / 机器路由 | 是 |
| `docs/current/` | 唯一事实来源（product / domains / platform / shared / rules） | 是（按路由） |
| `docs/changes/` | 重要改动的过程记录（brief/design/tasks/verification/worklog/reviews） | 否 |
| `docs/decisions/` | ADR（长期取舍） | 按需 |
| `docs/postmortems/` | 事故与可复用洞察 | 按需 |
| `docs/source/` | 尚未成为基线的原始草案 | 否 |
| `docs/archive/` | 被取代内容 | 否 |
| `docs/evals/` | 交付前检查清单 | 按需 |
| `docs/templates/` | 结构契约模板 | 按需 |
| `docs/scripts/` | 文档健康检查与漂移巡检脚本 | 按需 |

注意：`docs/` 同时承载应用官网静态站点（`index.html`、`js/`、`css/`、`img/`、`license.html`、`privacy.html`、`favicon.ico`）。
新增 Markdown 不影响站点；`docs/manifest/`（二进制清单产物）被 `.gitignore` 忽略，不要手工提交。

## 写文档的硬性要求

1. 每个 `docs/current/**` 文件必须有 frontmatter：`status: current`、`layer`、`domain`（或 `canonical_for`）、`last_verified`（`YYYY-MM-DD`）。
2. `status` 只能用 `current` / `draft` / `historical` / `superseded`；`superseded` 必须给 `superseded_by`。
3. 行数预算：`index.md` ≤120、`overview.md` ≤200、`flow.md` ≤250、`api-contract.md` ≤350、
   `frontend-behavior.md`/`backend-behavior.md`/`verification.md` ≤300、`*.rules.md` ≤200；
   其它 current 文档 >350 行要拆分或说明，>600 行不得作为必需上下文。
4. 契约类文档必须字段级：请求/响应/事件载荷逐字段说明类型、必填、允许值、为空语义。
5. 领域文档必须写清边界与失败形态（`Failure And Edge Cases`），不得只写"正常路径"。
6. 文档中引用代码实体（文件、函数、结构体、命令、事件）必须真实存在；改名时同步文档。
7. 禁止在 `current/` 里留待办标记（`TODO` 加冒号、"尚未实现"等，脚本会告警）；未定内容放 `source/` 或 change 目录。
8. 领域新增/拆分文档后，更新该领域 `index.md` 与 `docs/manifest.yaml`（`required`/`optional`/`code_globs`）。

## 变更流程（与 agent-runtime-docs 契约一致）

1. 复杂改动（多循环、跨领域、影响共享契约）创建 `docs/changes/YYYY-MM-DD-slug/`，写 `brief.md`、`design.md`、`tasks.md`、`verification.md`。
2. 编码前必须先写 `reviews/design-review.md`（Design Review 门禁），结论不是 `PASS` 时停下等确认；
   随后校准 `verification.md` 的 AC 并记录 `Acceptance Criteria Review`。
3. 每个循环在 `worklog.md` 追加一行（含 Re-anchor 列，每 3 个循环后的下一行必须填 `on-track`/`scope-drift`/`intent-drift`/`doc-drift`）。
4. 显式要求评审时写 `reviews/Lxxx.md`（AC 覆盖矩阵 + 独立验证），issue id 使用特性级稳定编号并在 `reviews/index.md` 记账。
5. 落地后：`design.md` 置 `status: implemented` + `landed_in`；把稳定事实同步进 `current/`；更新领域 `index.md` 与 manifest。
6. 交付后用户反馈触发版本升级（`version: 2.0` + `Version History`）；实现期偏差只写 `Design Deviations`，不升级版本。
7. 事故/复盘写 `postmortems/YYYY-MM-DD-<slug>.md`，并在其中指向具体护栏（rule / verification / manifest / script），否则标注 one-off。

## 健康检查与巡检

```bash
python docs/scripts/check_doc_runtime.py docs          # 结构、行数、路由 token 预算、last_verified、孤儿文档、worklog 契约
python docs/scripts/reconcile_docs.py docs             # 只读漂移报告（悬空引用、代码比文档新、未闭环 change）
```

- 修改文档后至少跑 `check_doc_runtime.py`，`ERROR` 必须清零（`WARN` 需判断是否需要处理）。
  本仓库的脚本副本已按 skill 契约适配：`changes/*/reviews/` 下允许 `design-review.md`（编码前门禁）与 `index.md`（issue 台账），其余文件必须是 `Lxxx.md`。
- 每几周做一次对账（reconcile）：一次只核对一个领域，先读该领域 current 文档与对应代码，再更新 `last_verified`。
- `manifest.yaml` 的 `code_globs` 是漂移检测的桥梁，结构调整后必须维护。

## 与代码的关系

- 代码是事实，文档是事实的**说明**；两者冲突时先确认真实行为，再改文档并在 change 中记录偏差。
- 不得为了让文档"看起来一致"而修改测试期望或注释掉断言。
- 文档中的示例命令必须可执行（脚本/CLI），示例 JSON 必须符合 schema。

## Verification

- `python docs/scripts/check_doc_runtime.py docs`（0 error）。
- 涉及 manifest 的改动：确认 `required` 引用的文件存在且 token 预算不超。
