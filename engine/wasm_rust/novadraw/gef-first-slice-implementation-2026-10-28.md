# GEF 对等样例首批实施

方案已记录本次授权，实际源码工作区已迁移到 `engine/novadraw`。
本文件是交付摘要；项目方案与验证记录从以下入口阅读。

- [完整方案与能力清单](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/novadraw/doc/design/editor/gef-official-examples-parity-proposal.md)
- [Scene 输出契约 ADR-053](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/novadraw/doc/adr/adr-053-embedded-vello-scene-target.md)
- [首批实施证据与成本](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/novadraw/doc/verification/reviews/gef-scene-target-implementation-2026-10-28.md)
- [公开 Scene backend 实现](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/novadraw/crates/novadraw-backend-vello/src/scene.rs)

首批完成 Vello Scene 输出，为后续 Masonry 自定义 Widget 提供公开的后端出口。
4 项新契约、39 项原后端回归及既有纹理 GPU 契约通过；新 Scene 在 DPI
1/1.5/2、两种背景与两代位置的 12 帧比较中，与纹理后端逐像素一致。

Masonry Widget、Xilem View、CreationTool、框选和模板创建尚未实施。
下一步为固定 Vello 兼容版本并建设稳定 Widget/View 消费者。
本会话没有执行 Git commit；工作期间检测到并发提交 `93182988` 已收录 Scene
输出代码，验证补记仍在工作区。
