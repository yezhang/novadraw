# “表现平面”及相关概念术语审计

类型：`book-maintenance-audit`

状态：`completed`

审计日期：2026-10-15

完成日期：2026-10-15

适用范围：

- `book/user/`：《Novadraw 应用开发与扩展指南》
- `book/internals/`：《深入理解 Novadraw》
- `book/contributor/`：《Novadraw 贡献者指南》
- `book/WRITING_PROFILE.md` 和三本书的维护规则

## 审计结论

书稿不再把“表现平面”作为首选中文名称。首次定义时使用：

> 表现状态层（Presentation Plane）

后续正文根据所指对象使用“表现状态”“表现覆盖值”“表现覆盖快照”或具体机制名称。
代码标识符、ADR 标题和必须保留的英文原词不改。

“表现状态层”指 Runtime 在稳定场景状态与帧录制之间维护的可丢弃状态边界。它保存
动画当前采样出的 Figure 覆盖值、树外动画临时绘制项及其所有权和生命周期信息。
它不保存业务事实，不替代稳定场景状态，也不拥有后端资源或已经呈现的像素。

实现中的 `PresentationSnapshot` 是稀疏覆盖快照。它只记录当前活动 Figure 的透明度、
变换、内容替换和动画临时绘制项。无活动动画时，`AnimationService::snapshot()` 返回
`None`，当前帧不生成表现覆盖快照，帧录制直接读取稳定场景状态。

## 事实依据

本审计以当前规范、实现和验证记录为依据：

- `doc/adr/adr-026-animation-and-presentation-plane.md`
- `doc/design/animation/animation-system.md`
- `doc/design/animation/capability-integration.md`
- `doc/verification/reviews/p2-m01b-animation-presentation-evidence.md`
- `doc/verification/reviews/p2-m01c-animation-consumer-evidence.md`
- `novadraw/src/animation/mod.rs`
- `novadraw/src/animation/service.rs`
- `novadraw/src/figure/preparation.rs`
- `novadraw-editor/src/feedback/mod.rs`

这些依据共同确认以下事实：

- Runtime 独占 `AnimationService`、活动时间线、通道所有权、表现覆盖值和动画临时绘制项；
- 已提交状态先到达最终值，动画采样不回写 Figure、布局、路由、通知或历史；
- 帧录制把稳定场景状态与可选表现覆盖快照合并；
- 取消、完成、禁用或故障清理覆盖值后，画面显示已经存在的最终状态；
- `FigurePresentation` 也用于非动画的 Figure 准备结果，不能与表现状态画等号；
- Editor Overlay 是 Figure 树中的临时视觉所有权机制，与树外 `TemporaryVisual` 不同。

## 为什么不用“表现平面”

“平面”在本项目中已经容易被理解为坐标平面、绘制平面、算法平面或 GPU
presentation plane。读者仅凭“表现平面”无法判断它描述的是状态所有权、空间层级还是
后端呈现机制。

可选名称的判断如下：

| 候选表达 | 裁决 | 原因 |
|---|---|---|
| 表现状态层 | 首选 | 明确这是状态边界，不是坐标面或 Z-order 图层 |
| 表现平面 | 退出读者正文 | 直译痕迹明显，空间含义过强 |
| 呈现平面 | 不采用 | 容易与“帧已呈现”和后端 present 混淆 |
| 表现层 | 不单独使用 | 容易被理解为 MVC 或应用分层中的 UI 层 |
| 视觉层 | 不采用 | 容易被理解为 Figure 图层或绘制顺序 |
| 视觉覆盖层 | 不作为总称 | 只能覆盖 Figure override，无法完整包含树外临时绘制项和所有权机制 |
| 表现状态域 | 不作为首选 | 含义准确，但“域”增加了新的抽象词，正文没有必要引入 |

“层”在这里表示因果链中的状态边界，不表示 Figure 树中的图层。首次定义必须写明：

```text
稳定场景状态
-> 可选表现覆盖快照
-> 帧录制
-> 渲染提交包
-> 后端状态
```

## 核心边界

### 稳定场景状态仍是权威输入

布局、路由、通知、历史和默认命中继续读取稳定场景状态。动画先接受最终状态，再从旧值
向最终值生成视觉过渡。动画结束或被取消时不需要回滚源状态。

书稿应避免下列表达：

- 表现状态替代稳定场景状态；
- 动画状态成为当前 Figure 状态；
- 表现状态保存动画前后的两套场景；
- 取消动画恢复源状态；
- 后端从表现状态取得新的业务事实。

### 表现覆盖是稀疏增量

当前 `PresentationSnapshot` 包含两类数据：

1. 按 `FigureId` 索引的可选透明度、变换和内容覆盖；
2. 不进入 Figure 树的动画临时绘制项。

没有覆盖的属性继续读取稳定场景状态。书稿不应把表现覆盖快照画成完整 Figure 树副本，
也不应写成另一份完整场景。

### 帧录制负责合并

表现状态层不独立产生完整帧。Figure renderer 仍遍历稳定 Figure 树，并在相应 Figure
处应用可选覆盖；树外动画临时绘制项在 Figure 树录制后追加绘制。表现状态层只改变当前
视觉输入，不取得帧提交、后端提交或完成确认的所有权。

### 清理是状态边界的一部分

每个表现覆盖值必须有活动计划或明确所有者。完成、取消、禁用、目标销毁和故障处理都要
清除对应覆盖、临时绘制项和持续唤醒。没有所有者但长期残留的覆盖会形成第二个状态权威。

## 相关概念裁决

| 英文或代码概念 | 书稿首选表达 | 含义与使用边界 |
|---|---|---|
| Presentation Plane | 表现状态层 | Runtime 持有的可丢弃状态边界 |
| presentation state | 表现状态 | 当前活动覆盖值和动画临时绘制状态的统称 |
| presentation override | 表现覆盖值 | 某个通道当前采样值，帧录制时覆盖已提交值 |
| `PresentationSnapshot` | 表现覆盖快照 | 当前帧使用的稀疏、只读覆盖集合 |
| presentation channel | 动画表现通道 | 类型化的动画目标槽位；定义后可简称“表现通道” |
| presentation value | 当前表现值 | 通道有覆盖时取覆盖值，否则读取已提交值 |
| presentation geometry | 采样表现几何 | 动画当前采样出的视觉几何，不等于已提交几何 |
| presentation transition | 视觉过渡 | 从过渡前快照到已提交最终值的表现变化 |
| `FigurePresentation` | 图形准备结果 | 受检测量、视觉边界和不可变绘制数据，不专属于动画 |
| `PresentationBinding<V>` | 表现准备绑定 | 把类型化当前值准备成 `FigurePresentation` 的能力描述 |
| `TemporaryVisual` | 动画临时绘制项 | Runtime 管理、树外、默认不可交互的绘制项 |
| visual envelope | 保守视觉包围范围 | 计算重绘区域所需的保守视觉范围 |
| channel owner | 通道所有者 | 当前有权写入某个动画表现通道的计划 |
| before capture | 过渡前快照 | 源状态变化前显式捕获的稳定几何或绘制结果 |
| committed fallback | 显示已提交最终状态 | 无覆盖、取消、禁用或失败后的显示结果 |

代码引用应保留源码拼写。例如，解释 API 时可以写：

> `PresentationSnapshot` 是当前帧的表现覆盖快照。

不要把 `FigurePresentation` 统一翻译成“表现快照”。该类型既用于普通 Figure 准备，
也可被动画内容覆盖和动画临时绘制项复用。正文应根据机制称为“图形准备结果”或直接保留
代码类型名。

## 三类临时视觉必须分开

书稿当前使用“临时视觉”概括了不同机制。writer agent 必须按所有者、是否进入 Figure
树和交互能力拆开。

| 概念 | 所有者 | 是否进入 Figure 树 | 默认交互 | 首选中文 |
|---|---|---|---|---|
| `TemporaryVisual` | Runtime 的动画服务 | 否 | 不参与命中和 accessibility | 动画临时绘制项 |
| Overlay | Viewer | 是 | 由具体 Handle 或 Feedback 决定 | 覆盖视觉 |
| Handle | Viewer | 是 | 可命中，产生类型化动作 | 控制柄 |
| Feedback | Viewer | 是 | 通常只表达候选结果 | 反馈图形 |

Overlay 不是表现状态层的同义词。它表示 Viewer 对稳定内容上方临时 Figure 子树的所有权
和清理关系。Handle 与 Feedback 继续使用 Figure 树的坐标、裁剪、重绘和销毁协议。

未来某类 Feedback 可以选择复用动画临时绘制项，但这需要明确的能力接入和交互合同。
书稿不能因为设计矩阵列出该方向，就声称当前 Editor Overlay 已经由 `TemporaryVisual`
实现。

## “呈现”与“表现”必须分工

“表现”用于 Runtime 中可丢弃的当前视觉状态：

- 表现状态；
- 表现覆盖值；
- 表现覆盖快照；
- 采样表现几何。

“呈现”用于后端和表面的 present 结果：

- surface 当前不可呈现；
- Host 提交后端；
- `RenderDisposition::Presented` 表示帧已呈现；
- 后端管理已经呈现的像素。

普通中文里的“呈现场景”“呈现文字”可以保留，但同一段讨论动画状态与后端结果时，应改用
明确对象，避免“呈现状态”“呈现快照”同时指向两套机制。

## 当前书稿中的事实漂移

### 无覆盖时的关系写错

`book/internals/src/01-graphical-framework.md` 当前写道：

> 没有活动表现覆盖时，表现状态等价于直接读取稳定场景状态。

应改为：

> 没有活动表现覆盖时，当前帧不生成表现覆盖快照，帧录制直接使用稳定场景状态。

这项修改反映 `AnimationService::snapshot()` 的实际返回值，也避免把表现状态描述成稳定
场景状态的副本。

### `FigurePresentation` 与动画表现状态混用

`book/internals/src/08-animation-and-presentation.md` 把 `FigurePresentation` 与表现通道、
动画临时绘制项并列，却没有说明它也是普通 Figure 的准备结果。writer agent 应先定义
`FigurePresentation` 的基础职责，再说明动画如何复用该不可变值。

### 中断策略超出当前实现

设计文档列出 `Replace`、`Queue`、`Ignore` 和 `Additive`。当前公开
`InterruptionPolicy` 只实现 `Replace` 与 `Ignore`。书稿若描述现状，只能写已经实现的
两项；`Queue` 和 `Additive` 必须标为目标设计或后续扩展方向。

### 表现几何命中尚未实现

`InteractionGeometryPolicy` 声明 `Committed`、`Presentation` 和 `NonInteractive`，
但当前 Figure 与内容表现绑定只接受 `Committed`。书稿不能声称普通 Figure 已经支持
基于采样表现几何的命中。

可以准确写成：

> 当前 Figure 动画的命中仍使用已提交几何。`Presentation` 策略保留在公共协议中，
> 但现有绑定会拒绝该策略。

动画临时绘制项继续使用默认不可交互语义，不需要把它描述成已经实现的通用
`NonInteractive` Figure 绑定。

### “稳定 presentation”缺少明确对象

“capture selected stable presentation”容易被理解成捕获表现状态。布局和路由过渡实际
捕获的是源状态变化前的稳定边界矩形、路径或不可变绘制结果。正文应写“捕获过渡前稳定
快照”，并说明具体捕获对象。

## writer agent 修改范围

### 公共写作规则

修改 `book/WRITING_PROFILE.md`：

- 把 `Presentation Plane` 的首选中文从“表现平面”改为“表现状态层”；
- 增加 presentation state、override、snapshot、channel、`FigurePresentation` 和
  `TemporaryVisual` 的推荐译法；
- 明确“表现”与“呈现”的分工；
- 把未定义的“临时视觉”“呈现状态”“呈现快照”列为需要核对对象的表达；
- 保留“帧已呈现”与 `RenderDisposition::Presented` 的既有对应关系。

### 《深入理解 Novadraw》

优先校准以下文件：

- `book/internals/src/README.md`
  - 术语表改为“表现状态层”；
  - 定义它是稀疏覆盖机制。
- `book/internals/src/01-graphical-framework.md`
  - 修正无活动覆盖时的关系；
  - 图中把稳定场景状态直接连接到帧录制输入，表现覆盖作为可选合并输入；
  - 避免把表现状态画成完整状态副本。
- `book/internals/src/08-animation-and-presentation.md`
  - 建议章名改为“动画与表现状态”；
  - 重新定义表现状态层、覆盖快照、通道和动画临时绘制项；
  - 分开当前实现与目标设计；
  - 修正中断策略和表现几何命中的实现状态；
  - 分开 `FigurePresentation` 与 `PresentationSnapshot`。
- `book/internals/src/appendix-concept-map.md`
  - 更新六类状态表；
  - 分开动画临时绘制项与 Overlay；
  - 更新章节输入输出表。
- `book/internals/src/OUTLINE.md`、`SUMMARY.md`
  - 同步章名、关键图和术语。
- `book/internals/src/07-rendering-and-frame.md`
  - 校准第 8 章交叉引用；
  - 保持“表现覆盖快照”与后端“帧已呈现”分离。
- `book/internals/src/10-layers-viewport-connections.md`
  - 检查“呈现状态污染”等表达，按对象改为“表现状态”或“已提交几何”。
- `book/internals/src/14-correctness-and-evolution.md`
  - 区分已实现能力、目标合同和验证证据。

### 用户指南与贡献者指南

用户指南通常不需要引入“表现状态层”总称。描述动画时直接说明：

- 最终状态先提交；
- 当前帧可以使用动画覆盖值；
- 取消后显示已提交最终状态。

贡献者指南可以保留代码类型和模块名，但必须说明：

- `FigurePresentation` 是图形准备结果；
- `PresentationSnapshot` 是动画表现覆盖快照；
- `TemporaryVisual` 不进入 Figure 树；
- Overlay、Handle 和 Feedback 属于 Viewer 所有权。

## 执行约束

writer agent 执行时必须遵守以下约束：

- 修改前重新读取工作区中的最新文件，不按本文假定的旧行号覆盖并行修改；
- 只校准书稿和书稿维护规则，不修改 Rust 标识符、公共 API 或 ADR 决策；
- 不对 `presentation`、`presented`、`visual` 或“呈现”执行全局机械替换；
- 保留 `RenderDisposition::Presented` 对应“帧已呈现”；
- 保留 `FigurePresentation`、`PresentationSnapshot`、`TemporaryVisual` 等代码拼写；
- 每处“临时视觉”都按所有者、树归属和交互能力重新分类；
- 每处能力描述都标明它属于当前实现、已接受设计还是后续方向；
- 更新章名时同步 `SUMMARY.md`、`OUTLINE.md`、交叉引用和维护提示；
- 不把本审计反向当作规范性 ADR。若规范文档也要迁移术语，应另开任务。

## 推荐执行顺序

```text
重新统计三本书中的相关术语
-> 更新 book/WRITING_PROFILE.md
-> 校准核心原理书第 1 章和第 8 章
-> 更新术语表、概念图和目录
-> 校准其他章节的交叉引用
-> 检查用户指南与贡献者指南
-> 构建三本书并抽查术语
```

## 验收清单

- 首次出现时使用“表现状态层（Presentation Plane）”；
- 普通正文不再使用未定义的“表现平面”；
- 无活动覆盖时，明确不生成表现覆盖快照；
- `PresentationSnapshot` 明确为稀疏表现覆盖快照；
- `FigurePresentation` 明确为图形准备结果；
- `TemporaryVisual`、Overlay、Handle 和 Feedback 已分开；
- “表现”与“帧已呈现”没有混用；
- `Queue`、`Additive` 和表现几何命中没有被写成当前已实现能力；
- 取消、完成和禁用均写成清除覆盖并显示已提交最终状态；
- `book/WRITING_PROFILE.md`、术语表、正文、概念图和目录使用同一套首选名称；
- 三本书构建通过；
- `cargo xtask docs` 和 `git diff --check` 通过。

## 执行结果

本次修订完成以下工作：

- 公共写作规则统一使用“表现状态层”“表现状态”“表现覆盖值”“表现覆盖快照”“图形
  准备结果”和“动画临时绘制项”；
- 《深入理解 Novadraw》第 8 章改名为“动画与表现状态”，并同步目录、提纲、术语表、
  概念图和章节交叉引用；
- 第 1 章和第 8 章明确：没有活动表现覆盖时不生成 `PresentationSnapshot`，帧录制
  直接读取稳定场景状态；
- `FigurePresentation` 已与动画表现状态分开定义，普通 Figure 准备、动画内容覆盖和
  动画临时绘制项可以复用同一图形准备结果；
- 当前实现只支持 `InterruptionPolicy::Replace` 和 `InterruptionPolicy::Ignore`；
  Queue、Additive 和采样表现几何命中均标为尚未实现；
- `TemporaryVisual` 已与 Viewer 的 Overlay、Handle 和 Feedback 按所有者、树归属和
  交互能力分开；
- 用户指南补充图形准备结果定义，贡献者指南补充三类视觉对象的代码边界；
- 读者正文中的“表现平面”“表现快照”“呈现快照”“呈现状态”和未分类“临时视觉”
  命中清零；`Presentation Plane` 只保留在首次定义和术语表；
- 未修改 Rust 标识符、公共 API、ADR 和规范性设计。

验证结果：

- 中文正文半角标点命中为 0；扫描命中只来自 Mermaid 箭头语法；
- `cargo xtask docs` 通过；
- `git diff --check` 通过；
- `./scripts/build_book.sh` 通过，三本书的 HTML 与 PDF 均已生成；
- PDF 已抽查核心原理书第 1、8 章、用户指南测量章节和贡献者指南 Runtime 章节。
