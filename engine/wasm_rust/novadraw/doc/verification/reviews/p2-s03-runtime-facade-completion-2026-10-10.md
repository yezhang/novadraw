# P2-S03 Runtime 子系统 Facade 与统一 Figure Mutation 完成记录

类型：`verification`

日期：2026-10-10

状态：`complete`

## 1. 范围

本批按
[ADR-033](../../adr/adr-033-runtime-subsystem-facade.md)
关闭 API 与架构 Review R01：

- Runtime 本体只保留组合根、Figure mutation 和故障查询职责；
- frame、resource、connection、input 等领域操作进入短借用 facade；
- 删除内置 concrete Figure 专用 mutation facade；
- 内置与外部 Figure 共用 typed component/capability update 提交链。

## 2. 实施结果

- 新增 `RuntimeFrames`、`RuntimeResources`、`RuntimeConnections`、`RuntimeInput`、
  `RuntimeFocus`、`RuntimeTooltip`、`RuntimeAccessibility`、`RuntimeAnimation`、
  `RuntimeObservations`、`RuntimeText`、`RuntimeViewports` 与 `RuntimeLayers`；
- 所有 facade 只保存 `&mut Runtime`，不持有独立状态，Drop 无行为；
- `FigureMut` 统一承载节点状态、容器和 typed Figure update；
- `FigureComponentUpdate<U>` 与 `FigureCapabilityUpdate<U>` 共用 prepare、effect
  validation、commit、revision、damage 和 notification 提交链；
- prepared update 支持显式 no-op、类型化属性通知、资源依赖变化和外部结构 effect；
- Label、Image、TextFlow、Border、Clickable、ScalablePolygon、Polyline、
  RoundedRectangle 与 Triangle mutation 已迁入 typed update；
- 删除原 Runtime 平铺方法、concrete Figure facade 和对应重复 FigureTree primitive；
- FigureTree 保留只读查询职责，Runtime 本体 rustdoc 公开方法数为 23，低于 25 上限；
- API 门禁独立拒绝旧 `prepare_submission`、`label`、`viewport` 和 `add_viewport`
  Runtime 入口。

## 3. 验证

- `cargo check -p novadraw --lib`：PASS；
- `cargo test -p novadraw --no-run`：PASS；
- `cargo test -p novadraw-editor --no-run`：PASS；
- `cargo xtask verify core.p2-f02-figure-capability`：PASS；
- `cargo xtask verify core.runtime`：PASS；
- `cargo xtask verify core.p2-s03-runtime-facades`：PASS；
- `cargo xtask check --quick`：PASS；
- `cargo xtask docs`：PASS；
- `git diff --check`：PASS；
- P2-S02/S03/S04 组合交付 `cargo xtask check --full`：PASS。

## 4. 关闭结论

R01 已关闭。Runtime 仍是唯一状态 owner 与 commit authority，但领域调用面不再平铺
于 Runtime 本体；内置 Figure 也不再拥有外部 Figure 无法复用的 mutation 路径。

下一批为 P2-S04 Editor Tool / Request / Policy / Handle 统一开放协议。
