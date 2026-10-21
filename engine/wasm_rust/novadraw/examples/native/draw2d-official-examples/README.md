# Draw2D Official Examples

本目录独立移植全部 25 个官方入口中的 Canvas / LightweightSystem 内容。
Figure 构成、布局、连接、文字、输入和图形资源按官方 Draw2D 语义实现，
不引用 `novadraw-example-scenes` 的近似场景。

SWT 原生 Text/Combo/Scale/Group、控制面板、Shell pack 和打印对话框不属于
引擎建设或对等验收范围。现有控制工具只是驱动场景参数的临时示例界面，
不代表 Novadraw 原生控件替代方案。Canvas 内真正的 Draw2D Figure 控件仍需实现。
不编写、修改、编译或运行 Java 验证代码，上游源码仅作只读参考。

```sh
cargo run -p draw2d-official-examples
cargo run -p draw2d-official-examples -- --list
cargo run -p draw2d-official-examples -- --screenshot=HelloWorld
cargo run -p draw2d-official-examples -- --screenshot-all
```

`src/lib.rs` 保存当前实现状态，未移植条目不绑定替代场景。
历史 complete 只保留当时证据范围，其他状态在 Canvas 功能验收后更新；
移除原生控件需求不会自动关闭尚未核对的 Canvas 差额。

`source-contracts.tsv` 保留官方入口/helper/resource 的只读来源清单；
`completion-contracts.tsv` 只登记 Canvas 完成范围。普通本地验证：

```sh
cargo test -p draw2d-official-examples --test source_contracts
```

当前计划见 [Canvas Core 计划](../../../doc/verification/plans/draw2d-official-examples-core-plan.md)。
本轮新增的 Java 探针、运行脚本和生成基线已撤销。

Obstruction 的组求解和交互按 [ADR-040](../../../doc/adr/adr-040-group-route-solving.md)
实现。运行后，背景左拖创建障碍；背景右键两次创建一对端点与连接；
左拖移动障碍或端点；障碍右键删除；端点中键删除整对，未配对端点中键只删除自身。
成对删除修正了官方示例的 parent mismatch，保留差异记录。

```sh
cargo test -p novadraw --test p2_group_route_contract
cargo test -p draw2d-official-examples --lib obstruction
cargo run -p draw2d-official-examples --example obstruction_verification
```

最后一个命令通过真实输入构造四阶段 Native 场景并截图，产物在
`target/visual-verification/screenshots/`。路线允许斜线与 Canvas 外折点，
不增加官方源码未规定的视口避让。
