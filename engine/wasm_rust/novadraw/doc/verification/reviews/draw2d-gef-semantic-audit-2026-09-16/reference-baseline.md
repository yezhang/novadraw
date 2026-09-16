# 对标基线与判定方法

类型：`reference-analysis`

日期：2026-09-16。

## 1. 固定的源码版本

| 对象 | 基线 | 范围 |
|---|---|---|
| Novadraw | HEAD `e8d54ac063d05c63abb9f2850365a837ec4e8c9d` + 本次开始时工作区 | 含未提交及未跟踪的 Rust 源码，尤其 G5.5 与验证工具；不能用 HEAD 单独重现 |
| Eclipse GEF Classic | `4463d9d0ce13c19d10fbe769d29f28b7345a8cba` | `org.eclipse.draw2d`、`org.eclipse.gef` 核心包与两者官方指南 |
| 项目规范 | ADR-014、ADR-015、相应专题设计 | 实现目标；不能取代外部事实或实际运行证据 |

快照见 [baseline.json](evidence/baseline.json)。其中收录 158 个 `novadraw*` crate 的
Rust 文件及 SHA-256，共 75,788 行，包含测试。清单表示审计边界，不表示逐行穷尽证明；
各组报告单独说明实际阅读的方法、调用链及测试。Native/Web 和 xtask 入口另作验证。
参考仓库存在 IDE/构建元数据变更，但本次限定的核心源码和官方 `guide-src` 与上述
commit 无差异。未使用 Zest 源码、文档或产品行为定义任何需求。

## 2. 官方文档

下面是固定 commit 的官方仓库链接；本次读取本地同版本原文，不依赖可能变化的在线
latest 页面。方法细节优先核对同版本 Javadoc 与实现。官方指南的历史 API 名称或
默认行为不能覆盖源码中已有的新扩展。

| 文档 | 核心事实 |
|---|---|
| [Draw2D Overview](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.doc.isv/guide-src/overview.adoc) | 轻量 Figure、有序组合、LightweightSystem、事件分发和更新分离 |
| [Painting](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.doc.isv/guide-src/painting.adoc) | inherited style；paintFigure→paintClientArea/children→paintBorder；状态恢复与累计 clip |
| [Coordinates](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.doc.isv/guide-src/coordinates.adoc) | inherited/relative 域不同；receiver 坐标与其定义的 child 坐标不能混用 |
| [Hit Testing](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.doc.isv/guide-src/hittest.adoc) | 逆 Z-order、TreeSearch prune/accept，命中复制绘制坐标与裁剪 |
| [Layout](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.doc.isv/guide-src/layout.adoc) | bottom-up invalidation、top-down validation；测量 hints；更新延迟合并 |
| [Connections](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.draw2d.doc.isv/guide-src/connections.adoc) | Connection 是 Figure；anchors→route→locators→bounds/damage；策略可替换 |
| [GEF Developer's Guide](https://github.com/eclipse-gef/gef-classic/blob/4463d9d0ce13c19d10fbe769d29f28b7345a8cba/org.eclipse.gef.doc.isv/guide-src/guide.adoc) | 三套结构、模型通知、EditPart 生命周期、Tool/Tracker、Request/Policy/Command 和各类交互 |

## 3. 必须保留的核心语义

1. **空间一致性**：同一模型几何事实驱动 paint、hit、layout、dirty、anchor、locator，
   显示次序与命中优先级一致。
2. **更新一致性**：失效可合并，布局/路由先稳定，再修复损伤；失败不能发布半成品。
3. **交互一致性**：目标、捕获、焦点、物理 hover 各有清晰职责；gesture 的 source、
   endpoint 与约束 index 在结束前保持身份；取消不修改模型。
4. **编辑一致性**：Tool 解释输入，Request 表达意图，Policy 解析目标、贡献命令与反馈，
   Command 只改模型，通知刷新可重建视图，undo/redo 重用同一链路。
5. **扩展一致性**：应用可替换 Figure/Layout/Anchor/Router/Locator、Tool/Tracker、
   Request/Policy，而无需修改框架中心分支。
6. **生命周期一致性**：对象/订阅/资源所属明确；删除、重建、重挂载的承诺可区分。

Java 类继承、SWT 控件类型、可变引用、整数像素、静态临时对象和异常机制不是兼容目标。
Rust 迁移可以改变它们，但必须给出可观察等价性和外部扩展证据。

## 4. 本报告状态词

| 状态 | 含义 |
|---|---|
| 保留 | 当前公开行为与被检查核心语义一致；不代表 Java 全部重载已移植 |
| 迁移 | 表达不同，有明确替代契约，仍保留该核心行为 |
| 部分 | 基本链路存在，但方法族、失败路径或外部扩展尚不完整 |
| 收窄 | 当前设计有意限制原行为；不称为“完整语义等价” |
| 缺陷 | 存在具体输入/时序导致当前行为违背外部核心语义或项目自身契约 |
| 后置 | 已明确延后，不能计为本次实现完成 |
| 未验证 | 证据不足，尤其平台视觉、第三方替换或动态复杂度 |

这些是本次审计用语，不直接改写 parity ledger 的状态枚举。缺陷严重度 P0/P1/P2
描述影响；Draw2D roadmap 的“P2 delta”是增量管理类别，两者不是同一概念。

每条结论按“官方契约→Rust 公共入口→实现/调用链→测试或反例→差异与措施”组织。
没有统一方法计数分母，不给出虚假的全项目兼容百分比；测试通过仅证明已有断言成立。

## 5. 证据分层

- **本次静态证据**：六个分组的 Java/Rust 方法、行号及上下文。
- **本次执行证据**：workspace.quality、额外 manifest commands、公共 API 事件复现。
- **历史证据**：M1-M10、G1-G5 既有审查和人工记录；只作背景，不冒充当前快照验收。
- **待补证据**：G5 检查点 C、G6 Native/Web/Headless 同一编辑事务、保存重建、
  缺陷反例回归和新增外部 Tool/Request 等扩展用例。

本次工作交付评估与方案，不修改运行时代码，不调整历史里程碑完成状态。
