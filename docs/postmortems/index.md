---
status: current
layer: postmortems-index
canonical_for:
  - postmortems-entry
last_verified: 2026-09-17
---

# postmortems/ 使用说明

记录事故、疑难缺陷与**可复用洞察**，目标不是追责，而是让同类失败更难重复。
模板：`docs/templates/postmortem-template.md`；命名：`YYYY-MM-DD-<slug>.md`。

## 什么算值得复盘的事故

- 影响用户数据或使功能整体不可用（配置损坏、凭据丢失、下载全失败）；
- 发布事故（清单验签失败、更新不可用、平台构建失败）；
- 排查耗时超过预期的疑难缺陷（即使最终修复只有一行）；
- 触及安全边界（凭据泄漏、XSS、路径穿越、供应链校验绕过）。

## 必须包含的内容

1. 时间线与用户可见影响；
2. 直接原因与根本原因（区分症状与原因）；
3. **失败类（假设错误类型）**，而不是表面症状；
4. 证据（日志、复现步骤、真实运行输出）；
5. 护栏：明确指向一个具体位置——`docs/current/rules/*.md`、某个 `verification.md` 的 AC、`manifest.yaml` 路由或脚本；
   只是"写进本复盘"不算护栏。
6. 复发计数与升级策略：同类失败第 2 次必须落一个显式检查；第 3 次以上必须变成自动化/结构性护栏。

## 索引表（见下）

| 日期 | 主题 | 失败类 | 复发计数 | 护栏位置 |
| --- | --- | --- | --- | --- |
| （暂无记录） | — | — | — | — |

新复盘必须在此表登记，并在同一次改动中落地护栏（改 rule / 加 verification AC / 加脚本检查）。

## 已知风险清单（潜在复盘候选）

以下问题当前是**已知薄弱点**，尚未构成事故，但如果被用户报告应优先复盘：

| 风险 | 出处 | 潜在护栏 |
| --- | --- | --- |
| 配置 JSON 损坏导致应用无法启动（需手工删文件） | `platform/storage.md` | 启动时备份损坏文件并回退默认值 |
| 保险库主密钥失效后静默重建（凭据丢失且无警告） | `domains/auth-secrets/flow.md` | 重建前提示 + 备份旧快照 |
| `media_download` 的 `unwrap()` panic | `domains/media-queue/backend-behavior.md` | 改为返回错误并上报 |
| 嵌套播放列表导致计数器不归零（`Cleanup` 不发出） | `domains/media-queue/backend-behavior.md` | 显式超时/兜底清理 |
| 二进制下载不遵循应用代理设置 | `domains/toolchain/backend-behavior.md` | 复用网络设置或明确提示 |

## Verification

- `python docs/scripts/check_doc_runtime.py docs` 会检查每条复盘是否指向具体护栏位置（或显式标注 one-off）。
