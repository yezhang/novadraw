# P2-S05 深层布局与文字 Presentation 收敛完成记录

类型：`verification`

日期：2026-10-12

状态：`complete`

## 1. 范围与根因

本批关闭
[UML 扩展能力验证 F-01](uml-extension-capability-2026-10-05.md#f-01深层复合布局与文字-presentation-未稳定收敛已关闭)。

最小复现为 root Toolbar -> inset compartment Toolbar -> 两个 Label。修复前：

- LayoutManager 已返回正确的 child content measurement；
- 两个 Label bounds 和 glyph command 均存在；
- compartment outer preferred/minimum size 遗漏上下左右 insets；
- 父 Toolbar 因此提交过小 bounds，client clip 再扣 insets 后裁掉后续文字。

根因位于 `FigureNode` 的 layout content/outer measurement 投影，不在 Label
presentation、clip/transform 记录、稳定化重启或递归渲染主循环。

## 2. 契约与实现

[Deep Layout and Presentation Convergence](../../design/architecture/deep-layout-presentation-convergence.md)
冻结四个尺寸域与顺序：

```text
layout constraints =
    container inverse(shrink(parent outer constraints, NodeState.insets))

parent measurement =
    container projection(expand(layout content measurement, NodeState.insets))
```

实现结果：

- `FigureNode::layout_content_constraints` 先扣 insets，再调用外部容器 constraint
  反投影；
- preferred measurement 将 content width/height 和 baseline 扩为 unprojected outer，
  再调用外部容器 projection；
- minimum size 使用同一 content -> outer -> projected 顺序；
- 显式 preferred/minimum override 保持 unprojected outer 语义，不重复加 insets；
- 无 LayoutManager 的 intrinsic/prepared measurement 路径保持不变；
- UML compartment 恢复自身 ToolbarLayout，没有 refresh 或第二帧 workaround；
- 未修改 ToolbarLayout、Label、稳定化循环和 `render_recursive.rs`。

外部 `ProjectingInsetContainer` 契约证明第三方 FigureContainer/LayoutManager 与内置
容器共用同一投影和错误通道，Core 不按具体容器类型分支。

## 3. 自动验证

- `cargo test -p novadraw --test p2_deep_layout_presentation_contract`：PASS；
  - parent outer constraints `100x60` 经 inset 与 scale 反投影为 content `40x20`；
  - content `30x10`、baseline `4` 投影为 outer `80x40`、baseline `18`；
  - 两层 Toolbar 首个 submission 使用最终 preferred/minimum、bounds、client clip
    与两条 glyph run；
  - 首帧完成后放大第一个 Label 字体，下一帧为 partial damage，并覆盖两个 Label
    的 old/new surface bounds。
- 共享回归：`m5_layout_contract` 17、`d4_constrained_measurement` 2、
  `m8_viewport_contract` 26、`m10_reusable_shape_border_contract` 16 项全部 PASS。
- `cargo xtask verify core.p2-s05-deep-layout-presentation`：PASS；
  - UML Headless probe：6 class、6 connection、41 glyph run、14 route points；
  - 报告：`target/verification/reports/uml-demo.json`。
- `cargo xtask check --quick`：PASS。
- `cargo xtask docs`、`git diff --check`：PASS。
- `cargo xtask check --full`：PASS。

## 4. Native/Vello 像素证据

macOS Native/Vello 截图：

`target/visual-verification/screenshots/uml-demo_Order_Domain_UML_1791784828.png`

相对修复前截图 `uml-demo_Order_Domain_UML_1791782568.png`，固定文字 ROI 的深色像素：

| 成员 | 修复前 | 修复后 |
|---|---:|---:|
| `provider` | 0 | 365 |
| `capture` | 0 | 1227 |
| `cancel` | 0 | 958 |
| `email` | 0 | 307 |
| `total` | 0 | 313 |
| `quantity` | 0 | 315 |

截图同时确认 class border、compartment separator 与 connection route 未发生错位。

## 5. 工作量与关闭结论

生产实现只增加 FigureNode 的三个内部盒模型组合函数，并调整 preferred/minimum 查询的
LayoutManager 分支；没有扩大派生状态扫描，没有增加 feedback round，没有新增公开 API，
也没有引入内置特权路径。

P2-S05 完成判据全部满足，UML F-01 关闭。自动图布局仍归 P2-L01，fragment style
仍归 P2-T03；下一审计整改批次为 P2-S06。
