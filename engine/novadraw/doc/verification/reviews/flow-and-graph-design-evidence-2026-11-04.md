# Flow 与图布局设计的证据导航

类型：`verification-record`

能力归属：Core Flow／Preparation／图布局的文档维护。
日期：2026-11-04。本次整理不新增行为验收，不改变 P2 完成状态。

## Flow 的已记录交付

结构化 Flow 于 2026-10-21 获批并实施，六个官方入口及最终门禁通过。
TextFlow 保留六个 section、蓝色嵌套 inline、字号与源码分行；
Large 保留四十个 block 和可追加文字来源；Caret 的多语言、片段边框、
光标与选择共用快照。MultiLineLabel 使用 width-tracking viewport、
两像素 margin、自动滚动条、焦点、键盘与 accessibility。
Surrounding 保留 Toolbar 三 Shape 子树和 RTL page；Bidi 的独立树共享 RangeModel。

字体包含随包普通、等宽、多语言字体以及 Noto Sans JP／Hebrew 和 OFL 许可证。
Native 六个入口截图的原记录路径为 `target/visual-verification/screenshots/`；
该路径是本地产物指引，不保证截图一直存在。
SWT 原生比较 Text、Hit info 面板与 Caret 控件不属于 Canvas 验收。
稳定发布、字体实例、窄窗口、真实子树、滚动、键入、来源、命中和键盘回归
属于此前的验证范围。

完整旧叙述见[整理前 Flow 原文](../evidence/document-quality-2026-11-04/structured-flow-before.md.txt)；
原批准、逐切片与测试计数从[ADR 分类记录](../../archive/adr-classification-2026-11-03.md)追溯。
当前行为以[Flow 规范](../../design/architecture/structured-flow-layout.md)为准。

## 图布局与投影的证据

纯请求、Directed／Compound、统一 adapter、候选查询与 GP01–GP04 的实施结果
由[图布局完成记录](../evidence/adr043/implementation.md)负责。
其中包括 52 个方向组合、Native 代表性场景、消费者与最终门禁；
本文未重跑这些验证。

[整理前图投影原文](../evidence/document-quality-2026-11-04/graph-preparation-before.md.txt)
保留实施前缺口表、切片流水、原测试入口与完成事实。
该表的“当前行为”指实施前基线，不能用于判断现在的 Core。
当前合同从[图投影规范](../../design/architecture/graph-projection-preparation.md)进入。
快照身份、共享预算、依赖环与同源恢复的规则仍由规范维护。

两个原文的字节与路径见[manifest](../evidence/document-quality-2026-11-04/manifest.json)。
执行状态继续由[P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
