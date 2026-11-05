# 独立 RichTextFigure 与双内容样例方案

类型：`proposal`

能力归属：拟新增 `novadraw-richtext` 扩展包拥有富文本文档、段落排版编排、缓存、
内容绘制和文本位置查询；Core（`novadraw`）拥有 Figure 生命周期、布局调度、
文字服务、坐标、输入、Viewport、ScrollBar、资源与帧发布。Vello backend
拥有编码与 GPU 资产，Winit platform 拥有窗口和原生输入。
`rich-text-figure-app` 拥有类图示例内容、单画布对象选择、双区域编排与诊断控制。
本方案不新增 Editor／GEF 能力。

日期：2026-11-05。状态：用户已授权实施；文档模型与双内容 app 的基础切片已实现。
完整方案和性能验收仍未闭合。

用户要求：设计独立 `RichTextFigure`，并建设独立样例 app
`rich-text-figure-app`。app 包含类图和富文本两个内容 Figure，
富文本区域含自己的滚动条。两个内容共享一个画布和 Runtime，采用左右并排布局。
用户后续要求已落地：单击选择画布对象，双击富文本进入编辑模式。
当前编辑模式仅开放组件内滚动，文本输入、光标及文本选择后续逐步添加。
方案阶段未扫描本项目 Rust 实现；获授权后已核对公开 API 并实现基础切片。
当前用户要求“先构建核心数据结构，样例 app 的能力从简单开始”，
因此下文的完整首版目标不等于本切片已经全部交付。
已实现的接口、范围和验证见
[组件 README](../../../crates/novadraw-richtext/README.md)与
[基础切片记录](../../verification/reviews/rich-text-figure-foundation-2026-11-05.md)。

## 要解决的问题

现有 Flow 贡献树适合将多个 Figure 组合成富文本。文档型消费者已经拥有文本、
样式和段落结构时，再将每个片段转成 Figure，会增加树身份、贡献采集和内容
组装工作。长文档需要直接保存文档结构，明确描述编辑影响，并按可见范围输出。

新增 `RichTextFigure` 作为外部场景中的复合 Figure。正文保存在内部文档模型，
不会为每个字符、span、段落或排版行创建 Figure。内部从文档快照生成段落布局、
高度索引和不可变绘制结果，最终仍通过 `PaintContext` 进入场景的有序命令流。
不会在 Figure 内创建第二套 GPU 设备、窗口、帧时钟或提交循环。

独立性体现在文档结构和排版编排，不改变 Core 的阶段调度与最终出版边界。
现有 Flow 贡献协议继续保留。新组件不通过把正文重新转成
`BlockFlowFigure`／`InlineFlowFigure`／`StructuredTextFigure` 来实现。

## 依据与采用范围

| 依据 | 本方案采用的契约 |
|---|---|
| [Figure 能力模型](figure-capability-model.md) | 外部 Figure 使用同一注册、生命周期和阶段接口；私有数据不必进入中心 registry |
| [组件更新](component-update.md) | attached 内容由 typed update 提交；源状态和派生结果分开 |
| [图文整合](../rendering/text-graphics-integration.md)与 [ADR-025](../../adr/adr-025-unified-graphics-and-glyph-preparation.md) | 文本布局结果不可变，paint 不 shaping，资源与最终提交统一 |
| [结构化 Flow](structured-flow-layout.md) | Unicode、整段 shaping、来源映射和交互几何继续保留；新文档入口不替代贡献树 |
| [管线删减方案](rich-text-pipeline-reduction-proposal.md) | 承接原生富文本模型直接发布内容的候选，不能成为免检查路径 |
| [文本增量提案](long-text-incremental-layout-proposal.md) | delta、位置映射和巨段局部计算有独立证明要求；其未批准算法不随本方案自动获批 |
| [ScrollBar 与活动调度](scrollbar-and-figure-activity.md) | 复用共享 RangeModel、普通部件、拖拽和活动调度 |

Draw2D 参考范围仅为 `org.eclipse.draw2d`，本地参考仓库提交为
`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`：
`text.FlowPage` 将内部 Flow 布局接入普通 Figure；
`Viewport.paintClientArea`、`translateToParent`、`translateFromParent`
统一滚动与坐标；`ScrollPane.setContents` 将内容与滚动组件组合。
采用这些基础扩展契约，不要求复制 Flow 的内部数据结构。
Java 仅只读参考，不编译或运行，不使用 Zest。

[Xi 增量断行](https://xi-editor.io/docs/rope_science_05.html)提供
Rope、编辑 Delta 和断行结果增量维护的参考，同时明确复杂 shaping 的限制。
本地 Zed 的 `crates/rope/src/rope.rs` 使用 `SumTree<Chunk>`，
`crates/text/src/text.rs` 区分 Buffer 与 BufferSnapshot。
本方案借鉴这些职责和索引方法，不直接依赖 Zed／GPUI 或搬入整个编辑器。
quiet-md 的实现尚未核验，不作为本次接口或算法依据。

## 组件边界与包归属

建议新增平台无关扩展包 `crates/novadraw-richtext/`，依赖 Core 公开协议，
不依赖 Editor、Winit 或 Vello。它是框架组件包，不是 app 内的私有实现。
正文默认使用 Runtime 已配置的文字引擎，当前默认依赖为未打补丁的
Parley 0.12.0；扩展包不持有第二个字体库或 shaping 服务真源。

内部模块按职责分为 document、layout、index、figure、interaction。
document 管理受检内容；layout 调用文字服务并管理段落资产；
index 支持位置和高度查询；figure 适配 Core；interaction 映射文本位置。
这些模块是实现分工，不分别创建空 package。

首版能力如下：

- UTF-8 文本、稳定段落身份、样式范围、方向和段落对齐。
- 字体、字号、颜色、粗体、斜体和下划线等已有文字协议支持的样式。
- 自动换行、精确内容高度、组件内垂直滚动与可见段落查询。
- 受检整文档替换、局部文本编辑、样式修改及位置查询。
- 保留段落布局和 glyph 资产，复用未变段落；有效宽度变化时重新断行。

首版不交付完整编辑器的键盘导航、IME、撤销、协作、Markdown 解析、
任意交互 Figure 行内嵌入或异步渐进排版。文本位置查询为后续编辑消费者提供
基础，但不宣称已具备完整输入法或辅助功能支持。
真实 inline Figure 继续由既有 Flow 支持；新组件不能作为该能力的等价替代。

首版采用原生结构化输入，不把 Markdown 当内部真源。以后可由外部解析器
生成同一文档输入。大段局部 shaping、独立字体后端与纹理块缓存均后置，
不作为这个 app 可运行的前提。

## 外部 Figure 树与内部文档

“两个内容 Figure”指 app 布局根的两个直接内容孩子，
不是整个场景只有两个节点。类图允许普通 Figure 子树，
`RichTextFigure` 允许视口与滚动部件子树；正文节点数不影响后者的树规模。

```text
AppRoot                         双列布局
├─ ClassDiagramFigure           类图复合容器
│  ├─ 类框与成员文字
│  └─ 关系连接
└─ RichTextObjectGroup           app 透明对象组
   ├─ RichTextFigure             组件外壳、样式与尺寸配置
   │  └─ ScrollPaneFigure
   │     ├─ ViewportFigure
   │     │  └─ RichTextContentFigure  唯一正文内容面
   │     ├─ 垂直 ScrollBarFigure  首版始终显示
   │     └─ 水平 ScrollBarFigure  首版隐藏
   └─ SelectionSurface           app 选择边框与输入面
```

`RichTextContentFigure` 是组件内部适配类型，拥有正文输入与文本查询入口。
`RichTextFigure` 是对用户公开的复合组件根，拥有外壳配置，容纳整个子树。
采用 detached 子树描述与具名挂载，挂载返回组件 handle。
不能仅将根 `Box<dyn Figure>` 放进树后，在 paint 或事件回调中偷偷补建孩子。
子树构造、准入和回执使用 [统一 mutation](runtime-mutation-and-subtree-admission.md)。

正文内部结构为：

```text
RichTextDocumentSnapshot
├─ Rope                         UTF-8 文本 backing
├─ 段落目录                     稳定 ID、文本范围、段落样式
└─ 样式范围索引                 范围、样式覆盖、编辑偏向

RichTextLayoutSnapshot
├─ 段落布局资产                 analysis／line layout／glyph／交互几何
├─ 高度摘要树                   段落高度、累计位置、可见范围查询
└─ 已解析依赖                   文档、字体、约束和装饰版本
```

首版建议使用维护中的 Rope 库作为私有存储，例如 Ropey；选用前确认许可证、
MSRV、快照共享和 UTF-8 范围转换契约。Rope／库类型不进入公开签名。
段落高度采用带子树总高度和数量摘要的平衡树；局部高度变化更新对应路径。
样式范围需要支持编辑映射和重叠覆盖，不能用每次编辑全量复制的扁平 spans
作为长期增量模型。具体树实现由实现前 API／依赖核验确认，不在此冻结库版本。

段落 ID 在同一文档实例内稳定，split／merge 明确发布身份与范围映射。
段落序号变化不代表身份变化。换文档时使用新的文档实例身份，
不能只比较 revision 数值便跨文档复用旧布局或位置。

文档构造器接收显式段落及其文本。段落之间形成硬边界，不按 Rope 库自己的
行分隔规则猜测段落语义。段内换行交给文字布局协议处理。
样式覆盖以段落基础样式为起点，嵌套范围由外向内应用；同层按声明顺序，
后声明的字段覆盖先声明的同字段，未声明字段继续继承。
冻结时规范为有序、不重叠的有效样式 runs，同时保留编辑所需的原范围关系。
跨段样式输入由构造器拆到对应段落；非法范围在冻结前拒绝。

## 状态所有权与更新接口

| 状态 | 唯一 owner | 写入边界 |
|---|---|---|
| 根与内容面 bounds、继承样式、可见性 | Core NodeState | 普通 Figure mutation／布局输出 |
| 文本、段落、样式范围 | 内容面的不可变文档快照 | 内容面 typed source update |
| 段落资产、高度索引、文本交互几何 | 组件 preparation 产生的不可变结果 | Core 接受完整候选后发布 |
| 滚动位置、范围和 extent | 内部 Viewport 的 RangeModel | Core Viewport／ScrollPane 协议 |
| 焦点、capture 与原生输入适配 | Core input／platform | 现有输入边界 |
| 字体实例、资源版本与文字引擎 | Runtime 文字／资源服务 | 现有资源注册及完成边界 |
| GPU glyph、纹理和设备缓存 | backend | 同一 submission/session |

不通过 `Arc<Mutex<Document>>` 让宿主绕过 Runtime 修改 attached 正文。
宿主可独立构造候选文档快照；内容面接受带旧身份／版本条件的受检更新。
首版一个文档绑定一个内容面，不新增跨视图共享文档写事务。
若以后支持多视图共享，必须单独定义通知、权限和原子性。

以下名称是拟议接口角色，尚不是现有公开 Rust API：

| 角色 | 语义 |
|---|---|
| `RichTextDocumentBuilder` | detached 文档构造；冻结前验证文本、范围和样式 |
| `RichTextFigure`／`RichTextHandle` | 描述并挂载复合组件；handle 保存受检子树身份 |
| `ReplaceRichTextDocument` | 在内容面提交新文档；可携带预期旧文档身份和版本 |
| `EditRichText` | 对同一旧版本应用半开 UTF-8 替换范围；返回实际变更回执 |
| `SetRichTextStyles` | 校验范围与覆盖规则后提交样式改变 |
| `RichTextPosition` | 文档身份、布局版本、段落身份、UTF-8 offset 和 affinity |
| 文本点位／caret／range 查询 | 通过同一稳定布局快照返回受检位置或几何 |

handle 不拥有另一套 source 或滚动值，也不授予任意子树写权限。
宿主更新正文时用 handle 取得内容面身份，再调用普通 `FigureMut` typed update。
内容面的输入回调只排队当前 target 的更新，外壳不借回调权限修改任意子节点。
复合结构替换沿具名子树操作；不把多个独立 facade 调用称为一个原子事务。

编辑端点必须处于 UTF-8 code point 边界；这不等于合法字素或 shaping 边界。
同批编辑以同一旧文档坐标解释，先规范顺序并拒绝重叠，再映射文本、样式、
段落和已登记位置。编辑偏向与布局 affinity 分别定义。
过期版本、越界或累计长度溢出在 commit 前拒绝。相同整文档和相同样式更新
返回 NoChange，不提升版本、不发通知、不排队布局工作。

首版保持具体 typed 文档模型，不为没有第二个消费者的“任意文档提供者”
预建 capability。内置和外部消费者使用相同构造、update 与 presentation 协议。

## 排版与绘制流程

1. 内容或样式更新提交新快照及 delta，按实际依赖声明测量、布局或仅绘制失效。
2. preparation 读取已解析样式、字体身份、有效宽度与文档快照。
3. 按段落输入身份和排版依赖查找资产；未变段落直接复用。
4. 受影响段落通过 Runtime 文字服务完成整段分析、shaping 和行布局。
5. 更新段落高度索引，生成精确总高度及同版文本交互几何。
6. Core 接受完整内容尺寸与 presentation，稳定化内部 Viewport 和范围模型。
7. paint 消费不可变布局；将当前裁剪区域映射到内容坐标，查询相交段落，
   按文档绘制顺序向 `PaintContext` 输出文字及装饰。

正文不进入 Flow contributor 采集或 FlowAssembly 构造。
新的 layout 编排复用现有受检 `StyledTextRequest`／`StyledTextLayout`
和 `TextLayoutEngine`，不复制字体回退、Unicode 或 backend lowering。
如公开协议不能承接独立文档输入、共享资产或可见行输出，先记录具体缺口，
提出最小 Core delta；不读取私有类型或在 app 层绕过协议。

首版增量粒度为段落。编辑只使受影响段落重新布局，后续段落复用局部坐标，
累计位置由高度树求解。超长单段仍允许整段重排，必须报告这个成本上限。
仅宽度变化时，已支持的阶段协议可复用 analysis；不合格引擎完整排版。
不能为了保证零 shaping 数量引入上游补丁或改变文字正确性。

布局资产可以覆盖全文，绘制输出按可见范围查询。首版精确计算全部段落高度，
不引入估计屏幕外高度。首次打开和整文档 resize 可能需要处理全文；
这一成本单独采样，不用滚动性能掩盖。
墨迹可能越出逻辑行框，可见索引必须使用保守墨迹包围或可靠覆盖信息。
范围无法证明时保守输出，不丢弃字体 overhang 或装饰。

纯滚动不得重建文档、样式索引或段落布局资产。Core 可能仍执行稳定化及
presentation 校验，验收分别记录这些成本，不能将“零重新排版”
写成“整帧零准备”。paint 禁止编辑模型、shaping 或打印日志。

## 自有滚动条与坐标

自有滚动条表示组件内置、生命周期随组件、滚动范围只控制正文。
实现复用 ScrollPane／Viewport／ScrollBar，不另写拖拽、capture 和通用坐标换算。
垂直条首版始终显示，避免宽度在滚动条出现／消失时反复改变。
正文跟随视口有效宽度自动换行，水平条隐藏；首版不提供 NoWrap 模式。
style 中无法拆开的宽内容按现有精确排版策略处理，不静默改变字体大小。

设可视高度为 V，精确正文高度为 H，RangeModel 的 extent 为 V，
maximum 为 max(H,V)，value 受限于 `[0,max(0,H-V)]`。
实际条宽、insets 和 client area 来自受检布局，不在业务逻辑中假定像素常数。
渲染和命中都由 Viewport 的同一子树变换消费 value。
正文不再叠加自己的 `scroll_y`。

滚轮、翻页、箭头和滑块拖动由组件内 ScrollPane 接收；正文区域内的滚动
只改变其 Viewport，类图位置不变。指针位于类图区域时不滚动富文本。
到达正文边界后按 Core 的剩余 delta／传播规则处理，不能由 app 全局截获。
整组件外部移动时连同滚动条一起移动；祖先裁剪同时约束正文和部件。

首版 resize 保持并 clamp 旧像素滚动位置。文档编辑或换文档后也执行范围 clamp，
不隐式重置到顶部。按段落锚点保持视觉位置是后续能力，不在首版宣称支持。
窗口失焦、隐藏、禁用、移除时由 Core 结束拖拽／repeat；输入返回与新布局
必须满足同版几何契约。

## 失败、资源与生命周期

文档构造及 source update 先验证全部文本和样式范围，失败保留旧源及 revision。
新源已提交后，若布局预算耗尽、字体查询或候选几何失败，沿 Core 的
preparation 错误与恢复协议处理；不会发布一部分新段落和一部分旧高度。
保留旧稳定结果不意味着它代表新源，过期文本位置查询必须拒绝或等待稳定化。

段落缓存 key 包含文档／段落身份与内容版本、解析后的字体和排版样式、
有效宽度、文字服务／字体资源版本。颜色等仅绘制依赖分开记录，
不得用字符串长度、全局 revision 或指针地址代替完整依赖。
继承字体与方向变化参与同一失效判断。

组件预算覆盖文档快照、段落输入物化、布局候选和留存资产。
失败计算仍计费；缓存设置明确容量和逐出策略，不无限保留历史宽度。
存在外部 presentation lease 时，被逐出的资产仍保持有效，其占用单独统计。
组件释放后移除绑定、查询身份和滚动会话，不依赖进程全局缓存。

注册及替代文字引擎消费同一合同；非法资源、过期版本、不可执行 glyph 输出
仍由既有边界拒绝。扩展包不分配 frame/session，不自建发布权威。

## rich-text-figure-app

位置：`crates/novadraw-richtext/examples/native/rich-text-figure-app/`，
保留独立 Cargo package，组合 Core 类图、`novadraw-richtext`、平台与 backend。
用户明确要求样例随 `novadraw-richtext` crate 维护：它主要展示独立富文本组件，
类图是共同绘制的辅助内容。使用其他包不改变样例的能力归属，
平台依赖保留在 app package，不加入组件库的 dev-dependencies。

类图区域使用普通复合 Figure，展示少量类框、成员标签和关系连接。
首版采用静态展示，用来验证与富文本共同绘制；不新增 GEF 编辑、拖拽工具、
序列化或业务模型。具体类名和关系是样例数据，在实现时记录为展示选择。

画布对象单击显示蓝色选中框，类框内的文字归属类框；关系线与独立标签可单独选择。
双击富文本后显示绿色编辑框，输入进入组件的现有滚动子树。
点击其他对象或画布空白结束编辑。Core 的唯一 focus owner 决定互斥选择，
app 私有选择面通过 typed update 改变输入命中和边框；不建立第二份焦点权威。
此模式入口属于样例编排，尚未成为组件库的编辑 API，也不新增 GEF 编辑工具。

富文本区域首先展示标题、嵌套样式、不同字号、中文和双向文本。
长文本负载按已知 TextFlowLarge 的 40 段内容与样式顺序构造原生文档，
不经 Flow 子树转换；tooltip、焦点边框等旧样例行为不算此次显示等价分母。
追加操作定位第一段对应文本范围，与旧样例的输入目标对齐。
首版文档高度必须超过内容视口，组件自身垂直滚动条可见。

app 只保留两个直接内容孩子，窗口 resize 同时重新安排两者。
双列比例、间距、初始窗口大小和主题值由具名配置定义，
不把它们写成通用 Core 语义。小窗口保持非负尺寸和可用滚动轨道。

app 提供以下验证操作，控制只负责场景编排：

| 操作 | 可观察行为 |
|---|---|
| 双击富文本后，在正文内滚动／拖动滑块 | 仅正文可见范围变化，类图不移动 |
| F1 重置文档 | 内容回到初始版本，滚动值按范围 clamp |
| F2 追加 Z | 第一段文本改变，复用其他段落布局 |
| F3 改变首段文字颜色 | 内容和 shaping 输入不变，更新绘制样式 |
| F4 切换窄／宽布局 | 改变富文本有效宽度，显示新的断行与总高度 |
| 切走窗口再切回 | 拖拽与焦点恢复无悬空状态，正文可继续滚动 |

诊断通过独立控制入口／帧结束采集，不在产品正文中展示实现细节。
拟提供 Perfetto trace 与关闭 tracing 的计时模式；具体 CLI 和运行命令在 app
实现后写入 README，本方案不把尚不存在的命令登记为可执行入口。

## 性能假设与验收

### 窗口缩放的首轮优化合同

能力归属：`novadraw-richtext` 拥有测量／准备结果缓存；Core 仅公开已有文字引擎
可缓存身份的查询，继续由引擎选择是否允许复用。用户已要求优化样例窗口缩放。
首轮先消除同一约束下重复的全文排版，不改变断行算法或跳过 Runtime 校验。

缓存 key 包含文档快照版本、引擎身份与测量 generation、有效文字宽度和布局方向。
有效宽度与现有文字请求一致，按受检 `f32` 约束解释；高度不是段落断行输入。
只有提供 `TextLayoutCacheIdentity` 的引擎可复用，缺少身份时继续完整计算。
不同引擎即使 generation 相同也不能复用；字体资源变化、方向或文档版本变化
均使旧条目不再匹配。通过 `MeasureContext` 查询身份沿用
`TextLayoutEngine::cache_identity` 的现有 opt-in 合同，不给默认引擎特权。

组件最多保存两个完整快照，用于相邻测量与准备约束。只保存完整成功候选；
失败不覆盖已有结果，返回的外部快照继续有效。缓存不持有可写文档真源，
不从不匹配的旧高度生成新文档布局。缓存命中仍检查本次共享预算，
按访问元数据计费；没有重新分配的布局不重复计为本次分配。
底层 glyph backing 的精确留存容量仍按基础切片未完成项管理，不能宣称已完成
组件内存预算验收。这里只建立有限容量，段落缓存与跨宽度 analysis 复用另行验证。

成本回放同时包含连续宽度变化、仅高度变化和滑块拖拽；关闭 tracing 的 CPU
对照覆盖输入 mutation、稳定化和绘制命令生成。该回放没有窗口和 GPU backend，
结果不能写成 Native 整帧或 GPU 完成耗时；Perfetto 阶段归因单独采集。
正确性需验证相同约束、宽度变化、文档替换、方向、字体 generation、
不同引擎身份、拒绝缓存的外部引擎和预算不足。

设正文有 P 个段落、D 个受影响段落、可见查询返回 K 个段落。
目标是正文 Figure 子树规模与 P 无关；纯滚动的段落范围查询约为
`O(log P + K)`，编辑的高度更新约为 `O(D log P)`。
这不含具体段落 shaping 成本、Core 稳定化、glyph 编码和资源验证。
有重叠墨迹或未知覆盖时查询可以保守退化，不能据此承诺整帧复杂度。

至少记录文档输入转换、measure、analysis/shaping、line layout、
高度索引更新、Core stabilization、paint/IR、backend 编码及整体帧时间。
同时记录重新排版段落数、布局资产命中、可见段落／glyph 输出、
分配与留存占用。统一时间窗口中的指标才能比较。

性能比较分开回答两个问题：

1. 与现有 Flow 路径比较：用相同文档、字体、正文 client area、滚动序列和
   绘制条件，单独比较正文链路成本，确认删去贡献树中转的收益。
2. 双内容 app 的整体成本：在同一类图负载下比较组件各阶段前后版本，
   覆盖真实滑块拖拽和连续窗口 resize，确认没有将成本转移到 Core 或 backend。

不能直接把双内容 app 与只有文本的旧 app 的总帧时间算成提速比例。
每个优化点保留匹配 release 前后样本，报告 p50、p95、样本数、构建配置、
字体与有效宽度。Perfetto 用于归因；关闭 tracing 的独立计时判断净收益。
不预设未经测量的毫秒目标或提速倍数，不执行未被要求的产物体积比较。
原始性能产物放在 `target/verification/text-rich-text-figure/`，
正式报告放在 verification，不提交原始生成快照与 trace。

验收必须覆盖：

- 外部 package 不修改 Core 类型分支即可构造、挂载、更新和销毁组件。
- 对照完整重排结果，确认段落增量后的 glyph、尺寸、来源和交互几何一致。
- 中英文、组合字符、emoji、连字、RTL／LTR、跨字号 baseline 和字体回退。
- 初始布局、窄宽切换、长单段、样式变化、字体资源变化、空文档和超短窗口。
- 纯滚动不重建文档、不重新排版；坐标、裁剪、滑块与总高度保持一致。
- 类图不受正文滚动影响；两个内容进入同一帧，外部裁剪和变换正常。
- 非法 UTF-8 范围、错误版本、预算失败、资源失败、移除后 handle 和查询拒绝。
- 图像对照与实际 glyph／墨迹输出检查；无窗口测试不能替代原生滚动验收。

可见行输出的具体机制必须在 API 核验后确定。如果公共绘制接口只能提交
整段且 backend 尚不能缩减到可见 glyph，允许首先交付可见段落查询，
但记录长单段输出未优化，不宣称已实现按可见行输出。

## 实施顺序与评审边界

1. 评审本方案的组件归属、文档真源、内部滚动、首版范围和 app 布局。
2. 核对 Core 公开 API 与所需依赖，列出确定可复用的入口和真实缺口。
   若出现最小 Core delta，独立说明 owner、权限、失败及外部消费者后复核。
3. 实现平台无关文档模型、typed update 和完整段落排版，建立正确性对照。
4. 实现复合 Figure、内部 ScrollPane 与 app，完成原生基础展示和输入验收。
5. 建立匹配性能基线，逐项推进段落资产、高度索引和可见输出，每项比较前后成本。
6. 完成外部消费者、定向检查与最终 workspace 门禁，更新示例导航和验证报告。

本次审批不会关闭现有 FR／LT 切片，也不会自动批准巨段局部 shaping、
worker、渐进尺寸或字体依赖补丁。执行状态仍只进入
[P2 队列](../../roadmap/p2-delta-backlog.md)，遵守同一时刻一个执行切片。
本方案沿用既有 Figure 和发布边界，不因新增组件而分配 ADR。
若 API 核验发现必须改变文字服务所有权或帧出版权威，再单独记录长期决策。

文档已按技术结构与中文表达自审。基础代码、Wasm 编译、Native 图像及定向验证
已有记录；独立评审、完整人工交互与匹配性能结果尚未取得。
