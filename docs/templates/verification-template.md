---
status: current
layer: domain
domain: <domain>
canonical_for:
  - <domain>-verification
last_verified: <YYYY-MM-DD>
version: 1.0
acceptance_review_status: <change 层必填：pending | approved | revised；domain 层省略>
---

# <Domain> Verification

## Test Layer Taxonomy

验证文档应区分三层测试。每个检查点必须明确属于其中一层，不能为了省事把 L1 当 L2 用，也不能把本可自动化的 L2 推到 L3。

- **L1 — Local Logic Verification**：局部逻辑验证，用单元测试 + mock 覆盖单个类/方法的输入输出、分支、异常和状态回写。只证明代码在当前假设下行为正确，不证明真实世界行为。
- **L2 — Automated Real-World Verification**：自动化真实联动验证，用集成/端到端测试覆盖多服务、数据库、缓存、外部 API 的真实交互。大多数真实联动场景应放在这一层。
- **L3 — Human-Aided Verification**：人工协同验证，仅用于自动化成本过高或确实无法自动化的部分，如视觉验收、复杂审批、真实第三方回调、生产灰度。

> 判断原则：能自动化的真实联动问题尽量放进 L2；只有 L2 成本过高或无法做到时，才退到 L3。

## Acceptance Criteria（代码级验收规则）

> **change 层 verification.md 必填；domain 层可省略。**
> 生成纪律：AC 必须基于对现有代码逻辑的仔细分析，引用真实存在的接口路径、类/方法名、字段名、权限码与错误码；只覆盖关键判断点（权限边界、状态转换、唯一性/并发、配额/资金语义、级联/清理边界、错误码映射），不写成伪代码，不得凭空捏造代码实体。
> 生成过程是多轮迭代：起草 → 对照实际代码逐条核验 → 修订 → 再核验，直到每条 AC 都能在代码中找到真实落点；禁止一次成稿，禁止核验发现不匹配后草草通过。
> 可判定判据：评审方拿到 AC 后无需额外调查即可直接判定 pass/fail（对照代码即可验证，不依赖另行阅读其他材料）。
> 与 design.md 的关系：design.md 记录业务层级目标（如“普通用户不能创建订单”），AC 把目标降维为可判定的代码级规则——哪个接口的哪个字段、在什么条件下必须发生什么、禁止发生什么。Must-Not-Break 保留用户可感知的业务承诺，AC 是其可判定落点，两者并存、互相不替代。

### AC-<NN> <判断点名称>

| 要素 | 内容 |
| --- | --- |
| 代码位置 | <真实接口路径 / 类.方法名> |
| 前置条件 | <触发该规则的输入或状态> |
| 预期行为 | <必须发生什么，含错误码 / 状态变化> |
| 禁止出现 | <什么情况下必须不发生什么> |
| 判定方式 | <代码走查 / 单测断言 / SQL 检查 / 前端行为检查> |

## Required Automated Checks

```bash
# L1：局部逻辑验证
<command>

# L2：自动化真实联动验证
<command with env/profile switch>
```

- [L1] <auto check>
- [L2] <auto check>
  - 适用场景：<why this needs real services>
  - 前提条件：<env prerequisites>
  - 预计耗时：<estimated duration>
  - 副作用：<external writes, cost, etc.>
  - 清理策略：<cleanup steps or known gap>

> L1 只能证明代码路径正确，不能证明真实世界行为（如签名过期、时间流逝、网络抖动、外部服务真实响应）。涉及过期、签名、刷新、缓存失效的测试，至少应有一层 L2；L2 成本过高或无法做到时，才退到 L3。

## Required Manual Checks

仅当自动化成本过高或确实无法自动化时才使用 L3。

- [L3] <manual acceptance step>
  - 原因：<why automation is not viable>
  - 执行人：<who runs it>
  - 判断标准：<what counts as pass/fail>

## Must-Not-Break Checks

每条 check 都应是用户可感知的行为承诺，而不是代码层面的断言。

- [ ] <business constraint, not code assertion>

## Regression Matrix

| Scenario | Expected Result | Layer | Check | Why It Matters |
| --- | --- | --- | --- | --- |
| <scenario> | <expected> | L1 / L2 / L3 | <test/manual> | <why this check proves real reliability, and what gap it fills> |

## Known Guardrails

- 禁止把 L1 测试当作 L2/L3 的充分证据。
- 禁止伪造时间、伪造外部服务状态、伪造签名过期来让测试“跑通”。
- 高成本 L2 测试必须可开关（环境变量 / profile / tag），默认不进入 CI。
- L2 测试写入外部服务后必须说明 cleanup 策略，或明确列为已知风险。
- L2 能自动化解决的真实联动问题，不要默认推到 L3 人工处理。
- 涉及过期、签名、刷新、缓存失效的测试，至少有一层 L2；L2 成本过高或无法做到时，才退到 L3。

## Known Anti-Patterns

| Anti-Pattern | Why It Is Wrong | Correct Approach |
| --- | --- | --- |
| 用 mock 模拟外部服务过期，宣称验证了过期兜底 | 把 L1 当 L2 用 | 真实让服务状态过期，或走 L3 |
| 为了跑通测试，把过期时间改成 1ms 或伪造时间 | 破坏真实过程可靠性 | 要么真实等待，要么标注为 L1 逻辑测试 |
| 高成本集成测试默认开启 | 拖累 CI / 团队 | 加环境变量开关，文档说明 |
| 集成测试写入外部服务不做 cleanup | 污染真实环境 | 补充 cleanup 或列为风险 |
| 把本可 L2 自动化的真实联动测试偷懒改成 L3 人工 | 回归效率低下 | 优先用 L2 自动化覆盖 |
| 验证文档只列命令，不说明验证什么 | 无法判断覆盖度 | 每个命令都要说明“它证明了什么” |

## When To Read History

- Read `<postmortem-or-ADR>` when changing `<risky-area>`.

## Acceptance Criteria Review

> **change 层必填，在 Design Review 通过后、编码前执行**：执行方在此提出验收承诺（每条 AC 如何证明完成），评审方对照实际代码逐条核验可判定性与完备性；`revised` 不是终点——修订后必须重新核验，双方迭代直到一致（`acceptance_review_status: approved`）才可开工。每轮修订原因记录在备注列，记录只追加、不覆盖。

| AC | 核验结果 | 备注（修正内容 / 协商一致结论） |
| --- | --- | --- |
| AC-01 | pass / revised / fail |  |
| AC-02 | pass / revised / fail |  |

- 评审结论：AC 已协商一致 / 需修正（列出修正项）
- 评审方 / 确认人 / 时间：

## Version History

> 版本语义：代码实施前的一切迭代（起草、核验、修订、协商）都只算 v1.0，不记录版本历史；仅当代码实施 + 评审交付给开发者后，开发者提出改动时升版本（v2.0、v3.0…）。实施过程中发现的偏差走 design.md 的 Design Deviations，不升版本。升版后：涉及 AC 变更必须重新执行 Acceptance Criteria Review 并在下方追加核验轮次；记录只追加、不覆盖 v1.0 内容。

### v2.0 — <YYYY-MM-DD>

- 触发：<开发者验收反馈，问题描述>
- 改动内容：<逐条列出，注明涉及的 AC 编号：新增 / 修订 / 删除>
- 影响范围：<受影响的验证手段、Regression Matrix、Must-Not-Break>
- 处理状态：<已重新实施 / 待实施 / 已拒绝并说明理由>
