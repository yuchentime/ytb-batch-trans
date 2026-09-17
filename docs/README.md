---
status: current
layer: root
canonical_for:
  - docs-runtime-entry
last_verified: 2026-09-17
---

# docs/ 使用说明

这个目录同时承担两件事，二者互不干扰：

1. **Agent Runtime 文档库**（本文件及其下 `index.md`、`manifest.yaml`、`current/`、`changes/`、`decisions/`、`postmortems/`、`archive/`、`evals/`、`templates/`、`scripts/`）：
   给 AI 编码代理和人类开发者使用的「当前事实 + 变更过程 + 决策/复盘」文档库，目标是用最小但足够的上下文完成二次开发。
2. **应用官网静态站点**（`index.html`、`js/`、`css/`、`img/`、`favicon.ico`、`license.html`、`privacy.html`）：
   由 `.github/workflows/pages.yml` 打包 `docs/` 目录整体发布到 GitHub Pages。
   线上地址：https://jely2002.github.io/youtube-dl-gui
   ![status badge](https://img.shields.io/github/deployments/jely2002/youtube-dl-gui/github-pages?label=deploy)
   新增 `*.md` 文件不会影响站点构建；`docs/manifest/`（由 `npm run manifest:gen` 生成）已被 `.gitignore` 忽略。

## 阅读与加载顺序

```text
docs/index.md                                  # 人类路由图
docs/manifest.yaml                             # 机器路由（按任务类型给出 required 文档）
docs/current/domains/<domain>/index.md         # 领域入口
该领域 responsibility unit 对应的 current 文档
docs/current/rules/*.rules.md + shared/*       # 约束与共享契约
仅在需要「为什么」时：decisions/ / postmortems/ / changes/ / archive/
```

## 当前事实与过程记录的分工

- `current/` 是**唯一事实来源**。任何已经稳定下来的业务事实、接口契约、数据模型、平台事实都应落到这里。
- `changes/` 是**过程记录**（brief/design/tasks/verification/worklog/reviews），落地后不再作为默认上下文。
- `decisions/` 记录长期取舍（ADR）；`postmortems/` 记录失败与可复用洞察；`archive/` 存放被取代的旧内容，默认不加载。
- `source/` 存放尚未成为正式基线的原始草案；`evals/` 存放交付前检查清单。

## 维护规则

- 新增或拆分领域：先更新 `docs/current/domains/<domain>/index.md`，再更新 `docs/manifest.yaml` 的路由与 `code_globs`（用于漂移检测）。
- 健康检查：`python docs/scripts/check_doc_runtime.py docs`（结构、行数预算、路由 token 预算、`last_verified` 过期、孤儿文档等）。
- 漂移巡检（只读报告）：`python docs/scripts/reconcile_docs.py docs`，按领域逐个核对后再改文档。
- 结构契约见 `docs/templates/`；写新文档前先读对应模板。
- 文档治理细则见 `docs/current/rules/documentation.rules.md`。
