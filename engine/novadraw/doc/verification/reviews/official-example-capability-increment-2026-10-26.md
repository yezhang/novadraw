# 官方样例能力补充实施记录

类型：`implementation-verification`

日期：2026-10-26

本批按 [能力补充方案](../../design/architecture/official-example-capability-increment-2026-10-26.md)
先实施已有 Core 契约内的样例修正。新增 Core 协议仍待评审，本记录不把提案写成
已接受契约，也不替换此前功能复核的原始快照。

## 已落实的消费者

| 消费者 | 改动 | 断言证据 |
|---|---|---|
| HelloWorld | 包装 root 使用已有 StackLayout，Label 随 Canvas bounds 更新 | 80×40、420×240、200×100 的收缩、放大和恢复 |
| UMLClassDiagram | 独立外部 DogEar Border；6 单位填充，5 单位描边折角，外框宽高减 1 | 最终 FillPath、StrokePath 与四条外框 Line；原窄宽测量与 locator 回归 |
| ShapeStyles | Rectangle 分开 fill/outline；Ellipse 使用整数 optimized bounds；圆角使用已有 Arc Path；Triangle 使用 NORTH 整数布局；disabled 浅/深两层绘制 | 0.25/2.5/5/30 线宽的公开 Rectangle 命令、三角形三点与圆角半径、两层颜色/顺序/AA 与临时变换恢复；原样式矩阵回归 |

当前实现未改变 Graphics、Render IR、后端或 Runtime 的公共协议。
零逻辑线宽仍不绘制、miter < 1 仍拒绝；样例继续显示 unsupported。
SWT 零宽的准确采用关系和 device hairline 的区别已写入方案，等待独立裁定。
disabled 的白/灰颜色是便携样例配色，不作为原生按钮主题像素对等证据。

## 验证结果

| 验证 | 结果 | 证明范围 |
|---|---|---|
| 样例库 `--lib` | 90 passed，0 failed，1 ignored | 新增 resize、折角、图形几何和 disabled 断言，以及既有样例行为 |
| `render.antialias` | 通过 | Core 4 项 AA 契约、后端 13 项定向测试、样例 3 项定向测试、单独执行的 40 个 GPU 矩阵与 Web backend 编译 |
| 样例库 `--lib` 严格 Clippy | 通过 | 本批样例与其依赖的受检编译路径 |

样例库忽略的 GPU 测试已由 `render.antialias` 显式执行，不能将忽略解释为 GPU 未验证。
另检查了圆角、三角形与 Enabled 轴的离屏矩阵图像；未建立新的上游逐像素基线。
样例库不证明真实窗口像素；GPU 矩阵不证明所有系统字体和 surface present。
本批没有执行 Java，没有将未验收的平台登记为通过，没有运行 workspace 全量测试
或重做 AA release 成本测试。

最终 workspace check、workspace 格式检查、文档门禁与 `git diff --check` 均通过。
完整日志保存于 `target/verification/official-example-capability-increment/`。
本批中文成稿已按好好说话技能再次回读，机械扫描的半角中文标点、破折号、
空泛元话语和机械编号标题均为 0 处；技术术语、数字与命令保留原意。

## 后续 Core 门禁

方向上下文、低 miter、通用下划线与可选 device hairline 各自需要规范修订、
外部消费者和后端一致性证据。执行状态只由
[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
真实文字布局和 Caret 先补反例，避免把验证空白当作 Core 缺陷。
