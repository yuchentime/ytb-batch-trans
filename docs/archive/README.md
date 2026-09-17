---
status: current
layer: archive-readme
canonical_for:
  - archive-entry
last_verified: 2026-09-17
---

# archive/ 使用说明

`archive/` 存放**被取代但保留可追溯性**的内容，默认不加载、不作为实现依据。

## 子目录

| 目录 | 内容 |
| --- | --- |
| `superseded/` | 被新版本取代的 current 文档（保留原文 + `superseded_by`） |
| `references/` | 仍有参考价值的历史材料（旧版官网文案、旧截图、外部资料抄录） |
| `old-changes/` | 已闭环且不再需要默认阅读的 change 目录（整目录移入） |

## 移入规则

- `current/` 文档被取代：先在新文档中说明变更，再把旧文档移入 `archive/superseded/`，frontmatter 置
  `status: superseded` 并补 `superseded_by: docs/current/...`。
- change 目录移入：该 change 已 `implemented` 且其事实已完全同步到 `current/`，并且至少经过一次发布。
- 移入时在对应 `index.md`（或本文件）登记一行：日期、来源路径、归档原因、替代位置。

## 归档登记表

| 日期 | 来源 | 原因 | 替代位置 |
| --- | --- | --- | --- |
| （暂无） | — | — | — |

## 禁止

- 把归档内容当作当前事实引用（引用会导致读者被旧行为误导）。
- 不删除内容以"整理"为名；归档是保留证据的手段。
- 在归档目录中继续编辑（需要修改说明是"当前事实"，应移回 `current/`）。

## Verification

- 归档文档必须带 `status: superseded` 或 `historical`，`superseded` 必须有 `superseded_by`（脚本会校验）。
- 归档内容不参与 token 预算与孤儿检查。
