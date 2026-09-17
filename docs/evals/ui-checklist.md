---
status: current
layer: eval
domain: evals
canonical_for:
  - ui-checklist
related:
  - docs/current/rules/frontend.rules.md
  - docs/current/domains/media-queue/frontend-behavior.md
last_verified: 2026-09-17
---

# 界面交付检查清单

## 结构

- [ ] 使用了既有基础组件（`BaseButton`/`BaseFieldset`/`BaseSelect`/`BaseSecretInput`/`BaseProgress` 等），未重复造轮子。
- [ ] 组件放在正确区域目录；命名符合 `PascalCase` 与前缀约定（`Base`/`The`/领域前缀）。
- [ ] 未引入内联 `style`；自定义样式使用 `<style scoped>` 或 Tailwind 类。
- [ ] 颜色/间距使用 daisyUI 语义变量，未硬编码色值。

## 状态与交互

- [ ] 每个异步动作有加载态（`loading` 或 spinner），避免重复提交。
- [ ] 按钮可用性与所在 store 的状态严格对应（例如下载按钮仅在 `configure`）。
- [ ] 失败路径有用户可见反馈（toast / 错误卡片），不是只 `console.error`。
- [ ] 空态有明确文案（首页空态、无日志、无诊断、无最近路径）。
- [ ] 长文本/标题有截断或 `title` 兜底；缩略图有占位图兜底。

## 无障碍

- [ ] 图标按钮有 `aria-label`/`title`/`tooltip` 或 `sr-only` 文本。
- [ ] 表单控件有 `<label for>` 或 `aria-label`；错误提示与控件关联。
- [ ] 键盘可达：Tab 顺序合理，`details`/`collapse` 可键盘展开。
- [ ] `role`/`data-testid` 与既有约定一致（便于 E2E）。

## 文案与国际化

- [ ] 所有用户可见文本走 `t('...')`；无硬编码字符串（除品牌名/技术名）。
- [ ] `src/locales/en.json` 已补键；其它语言缺失时回退英文且不报错。
- [ ] 插值使用命名参数；数字/百分比格式化一致（复用 `helpers/units`、`useDuration`）。
- [ ] 后端产生的文案（托盘/通知/安装）未在前端重复定义。

## 主题

- [ ] light/dark/system 三种模式下都可读（对比度足够，语义变量正确）。
- [ ] 未直接操作 `document.body`（除 `useTheme`/`applyTheme`）。
- [ ] 图标颜色使用语义类（`text-base-content`、`text-error` 等）。

## 验证

- [ ] `npm run lint:fix` 与 `npm run build` 通过。
- [ ] 组件测试或 E2E 覆盖新增交互（至少覆盖主路径与一个失败路径）。
- [ ] 在 `npm run tauri dev` 的 800×900 窗口与最小 750×650 尺寸下都不溢出/不遮挡。
- [ ] 涉及平台差异的界面（Windows/macOS 快捷键文案、路径分隔符）已核对。
