---
status: current
layer: source-index
canonical_for:
  - source-entry
last_verified: 2026-09-17
---

# source/ 使用说明

`source/` 存放**尚未成为正式基线的原始材料**：设计草案、架构草图、迁移设想、方案对比。
这里的内容不参与默认加载，也不作为实现依据。

## 何时放这里

- 只有想法/草图，尚未确定要做；
- 方案对比（多个候选，未做决定）；
- 从外部（issue、讨论、论文、上游仓库）抄录的原始材料；
- 需要保留但明确"不是当前事实"的说明。

## 何时不放这里

| 内容 | 正确位置 |
| --- | --- |
| 已决定要实现的功能设计 | `docs/changes/<feature>/design.md` |
| 已经稳定的业务事实 | `docs/current/**` |
| 长期技术取舍 | `docs/decisions/` |
| 事故与洞察 | `docs/postmortems/` |

## 目录约定

```text
docs/source/<topic>/
  index.md        # 主题说明：来源、日期、当前状态（是否已被取代）
  raw-design.md   # 原始材料
```

每个主题目录的 `index.md` 必须写明：材料来源、撰写/抄录日期、当前状态（继续沿用 / 已放弃 / 已进入 change）。
一旦内容升级为正式基线，必须移到 `changes/` 或 `current/`，并在原 `index.md` 标注去向（必要时把目录移到 `archive/`）。

## 现有主题

| 主题 | 状态 | 内容 |
| --- | --- | --- |
| `batch-transcribe-translate/` | 草案，待澄清（2026-09-17） | 把工具改造成“批量转录 + AI 翻译成中文”：原始需求、本机环境实测、硬约束与开放问题 Q1–Q16 |

> 更早期的设计讨论散落在 GitHub issue/PR 中，未抄录到仓库。

## Verification

- `python docs/scripts/check_doc_runtime.py docs`（`source/` 不参与孤儿与预算检查，但仍会检查 frontmatter 与引用完整性）。
