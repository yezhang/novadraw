# G3 Selection / Targeting / 输入仲裁行为记录

类型：`verification`

日期：2026-09-13

状态：`complete`

人工门禁：`PASS`

## 1. 范围

G3 固定 Viewer 交互所有权，不引入 G4 的 Request、EditPolicy、Command contribution
或 drag tracker：

- 有序 selection、primary selection 与 EditPart focus；
- Part / Handle / Feedback visual owner；
- point targeting、ancestor fallback 与 contents fallback；
- 标准 root layer 组合；
- Figure consumed/capture 与 Editor fallback 仲裁；
- Native 人工验收入口。

受影响 API family：

- Draw2D `event.dispatcher` P2 delta；
- GEF `viewer.targeting`、`viewer.selection`、`viewer.focus`、`root.layers`；
- GEF `input.arbitration` 的 Figure/Editor 边界。

## 2. DispatchOutcome P2 Delta

`EventDispatcher` 内部原有 `bool handled` 曾被 `Runtime::dispatch_*` 丢弃。G3 新增
`DispatchOutcome`，只公开：

- 实际 dispatch target；
- Figure handler 或 Runtime fallback 是否 handled；
- dispatch 后的 primary pointer capture。

该 delta 不改变 target 查找、事件点降域、capture、focus、scroll/zoom fallback 或
callback 语义。旧调用方可继续忽略返回值。

## 3. Viewer 行为

- `SelectionModel` 保持稳定顺序，最后一项为 primary；
- replace/append/toggle/remove/clear 返回 typed delta；
- focus 与 Figure keyboard focus 分开存储；
- part retire 时 selection、focus、handle 和 feedback 同步清理；
- `VisualOwner` 明确区分 Part、Handle 和 Feedback；
- feedback 不参与 targeting，handle 优先于普通 part；
- 无 selectable visual 时返回 contents fallback；
- standard root layers 固定 scaled/unscaled 域和 z-order；
- Figure widget handled press 后不进入 Editor selection fallback；
- Figure capture 持续到 release，不被 Editor 中途接管。

正式 Tool/tracker 从 G4 开始，复用同一 `DispatchOutcome`，不再修改 Core 输入协议。
Viewport/zoom 下 feedback 与 auto-expose 归入 G5。

## 4. 自动验证

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
git diff --check: PASS
```

新增契约：

- `novadraw-scene/tests/p2_dispatch_outcome_contract.rs`：2 项；
- `novadraw-editor/tests/g3_selection_contract.rs`：3 项；
- `novadraw-editor/tests/g3_viewer_interaction_contract.rs`：7 项。

覆盖 replace/append/toggle、primary/focus、删除 reconcile、layer z-order、handle 优先、
feedback 穿透、contents fallback、widget consumed 和完整 pointer capture。

## 5. Native Probe

`cargo run -p node-editor-demo` 已成功启动。截图检查确认：

- Vello 帧非空；
- 蓝色/绿色节点与 Widget 正常显示；
- root layer 组合无明显错位或重叠；
- 窗口标题可显示 selection 数量。

人工步骤见
[`../manual/g3-selection-targeting.md`](../manual/g3-selection-targeting.md)。检查点 A
的单选、多选、primary/toggle、widget arbitration、handle targeting 与 resize
全部 PASS。

## 6. 人工验收发现与修复

验收额外发现两项同源视觉回归：

- 删除 selection handle 后矩形描边出现残留增粗；
- 多选 toggle 后 damage union 内出现黑色区域，resize 后消失。

运行时证据确认 damage bounds 与 overlay 坐标正确，黑区只出现在 Partial frame。
Vello scratch texture 原先对 Partial damage 使用透明 base color，但随后通过
`copy_texture_to_texture` 直接覆盖 opaque retained texture，透明像素最终显示为黑色。

修复后 Full/Partial scratch 均先恢复项目规定的 RGB(238,238,238) 不透明背景，再绘制
scene 并复制 damage 区。相同 Partial damage 路径下黑区和描边残留均消失，不需要
Full redraw；workspace 全量门禁与 Vello feature 定向测试通过。

## 7. 单测生成工作流

`bits-unit-test-gen` Step1-Step6 已执行。初始缺陷探测测试稳定证明
`DispatchOutcome` 与 `SelectionModel` 缺失，随后由正式实现闭合。Step6 的
`utree flush` 因 sandbox 拒绝写入 skill 安装目录 `.skill_update_*` 未能落盘；
项目测试源码与 Cargo 门禁不受影响。
