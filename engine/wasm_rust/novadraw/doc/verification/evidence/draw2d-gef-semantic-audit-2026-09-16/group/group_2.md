# Group 2: Layout / Validation / UpdateManager / Damage / Viewport

日期：2026-09-16。范围：`scope: full_file`，当前工作区，不限 diff。

## 结论

不建议将本组整体标为与 Draw2D 完全等价。两阶段更新、typed constraint、
range clamp、freeform 内容域及 zoom 协调有实际实现；六布局的完整
min/preferred/constraint/hints 行为仍有差异。

按分组契约，JSONL 仅提交以下最明确的三个问题，均为 P1、置信度 10/10：

| ID | 问题 | 性质 |
|---|---|---|
| G2-F1 | Freeform 溢出子节点移动时，旧位置 damage 被父节点 visual bounds 截掉 | damage 算法错误 |
| G2-F2 | LayoutSnapshot 返回 node-local client area，布局再次计入 inset | 坐标协议实现错误 |
| G2-F3 | Border 南/东区预留与实际布局使用不同的尺寸上限 | 布局算法内部不一致 |

下文映射中的其他未等价项保留为审计 delta，不因 JSONL 的三条限额被标成
“已验证”或“有意变体”；未找到明确设计依据的算法差异，不擅自认定为有意。

## 范围与方法

- 已依次读取 `AGENTS.md`、`CLAUDE.md`、`evidence/reviewer-brief.md`，
  并读取 `review_files.md`、`review_groups.md` Group 2、项目记忆。
- 使用 `analyzing-gef-code` 和 `bits-code-guard` 的分组工作流，已读取通用工作流、
  七维度与定级规则；Rust 无额外语言专项。这里只产出分组报告，不生成主审聚合报告。
- `SKILL_ROOT=/Users/bytedance/.trae-cn/skills/bits-code-guard`；
  `REPO_ROOT=/Users/bytedance/Documents/code/GitHub/drawjs`；
  `WORK_DIR=engine/wasm_rust/novadraw/doc/verification/reviews/draw2d-gef-semantic-audit-2026-09-16/evidence`；
  本组只写 `WORK_DIR/group/group_2.md` 与 `group_2.jsonl`。
- 完整读取本组 `layout/*.rs`、`container/*.rs`、`runtime/update/listener.rs`、
  `runtime/update/mod.rs`、`lib.rs`；读取 deferred/repair 完整产品函数及相关内嵌测试。
- 共享 `graph/mod.rs` 实际阅读：LayoutState/cache、FigureNode box/transform、
  repaint/freeform extent、validation cycle/递归校验、LayoutOutput 验证与提交、
  preferred/minimum/maximum、constraint/manager、bounds/erase、失效父链和 LayoutContext。
- 共享 `runtime/runtime.rs` 实际阅读：layout/constraint mutation、set_bounds、
  guarded mutation、stabilize、submission/complete/prepare_frame；未把共享文件的其他职责算作本组完整覆盖。
- 完整读取 `m5_layout_contract.rs`、`m8_viewport_contract.rs`、
  `d2_freeform_contract.rs`、`d2_layer_contract.rs`。补读
  `d4_constrained_measurement.rs`、`graph/update_integration_test.rs:432-574`、
  deferred panic recovery 测试及 `graph/render_recursive.rs` 的绘制裁剪调用链。
- 查阅当前 parity 账本、`layer-and-freeform.md` 与 `scalable-zoom.md`；
  未采用 2026-09-15 报告作为结论证据，未读 archive 或 Zest 源码。

### 官方证据

以下 `J/` 均指
`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/`。
以下 `G/` 指同仓库 `org.eclipse.draw2d.doc.isv/guide-src/`。

- 完整阅读官方 `G/layout.adoc`、`G/coordinates.adoc`、`G/painting.adoc`。
- 阅读 `J/LayoutManager.java`、`AbstractLayout.java`、`XYLayout.java`、
  `StackLayout.java`、`BorderLayout.java` 完整契约与实现。
- 阅读 `J/FlowLayout.java:128-416`、`ToolbarLayout.java:92-387`、
  `GridLayout.java:145-796`，覆盖测量、span、grab、压缩、换行和摆放。
- 阅读 `J/Figure.java` 的 `getClientArea:672-686`、`erase:375-383`、
  `validate:2174-2181`，以及 `DeferredUpdateManager.java:90-307`。
- 阅读 `J/ViewportLayout.java`、`FreeformViewport.java`、`DefaultRangeModel.java`、
  `FreeformLayout.java`、`FreeformHelper.java`、`FreeformLayer.java`、
  `FreeformLayeredPane.java`、`ScrollPaneLayout.java`、`IScalablePane.java`。
- 阅读 `J/Viewport.java` 的 range/view-location/translation/validate 方法；
  `J/zoom/AbstractZoomManager.java:347-440`、`DefaultScrollPolicy.java`、
  `MouseLocationZoomScrollPolicy.java`。未完整展开 ScrollPaneSolver，相关等价性不作全量结论。

### 先建立的基线

1. `revalidate` 向上传播失效，正常 layout 自上而下，布局后验证 children；
   validation 期间可重新入队，不能绘制中间不稳定状态。
2. layout 是可替换策略；约束属于 parent-child 布局关系；min/preferred 的 hints
   用于约束测量，不能统一解释为最终硬尺寸。具体算法决定接受哪个轴。
3. local client area、insets 和 scale 必须在布局与绘制坐标域之间一致转换。
4. repaint 先收集，再在 validation 后映射到根；移动必须覆盖旧、新可见区域。
5. viewport 负责 scroll/range，scalable 负责 scale；zoom 按
   location calculation -> scale -> viewport validation -> location commit 执行。
6. freeform 的负坐标与内容 extent 属于基础 Draw2D 能力；
   Novadraw 可以保留独立 presentation bounds，但不能因此丢失可见 damage。

## 确认问题

### G2-F1: Freeform 溢出子节点的旧 damage 丢失

- 位置：[repair.rs:116-119](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-scene/src/runtime/update/repair.rs#L116-L119)。
- 严重度 P1，逻辑错误，置信度 10/10；正常有限坐标即可触发，不是异常输入防御问题。
- 触发：普通不透明 root 为 `(0,0,200,150)`，其 freeform 子容器 F 为
  `(20,20,20,20)`，F 内子图形 C 为 `(60,0,10,10)`。初帧提交后，
  用 `Runtime::set_bounds` 将 C 移至 `(80,0,10,10)`，下一帧采用 retained partial。
  场景不含 viewport 或其他会顺带触发全屏重绘的节点。
- 调用链：`runtime.rs:1946-1968` -> `graph/mod.rs:3887-3955`。
  `erase` 把旧 local visual box 转入 F 的 local 坐标并以 F 为 dirty owner；
  `collect_parent_chain_steps` 随后首先与 F 自身 `(0,0,20,20)` 相交，
  旧 `(60,0,10,10)` 被清空。后面的 OverflowVisible 分支只控制祖先 clip，
  无法恢复已被第一步丢弃的旧区域。
- 结果：新位置根域 `(100,20,10,10)` 进入 damage，旧根域 `(80,20,10,10)`
  不进入 damage，保留帧留下残影。绘制路径
  `render_recursive.rs:159-190,230-255` 明确允许该 freeform 溢出可见。
- Java `Figure.erase` 与 `DeferredUpdateManager.repairDamage` 要求旧可见区域修复；
  Java freeform 另通过 `FreeformHelper.setFreeformBounds` 维护包络。
  Novadraw 不改写 presentation bounds 是合理迁移，但旧擦除仍必须覆盖完整视觉区域。
- 建议：在删除/移动前保留旧根域 damage，或区分 owner 自身 repaint 与后代擦除，
  使后代贡献不被 owner 的非包络 visual box 截断；不要靠强制全屏重绘掩盖问题。
- 测试局限：`d2_freeform_contract.rs:325-344` 仅测静止 child 自身 repaint；
  该输入从 child 开始传播，可绕过本缺陷。应另覆盖溢出 child 移动的 old/new damage。

### G2-F2: 布局 client area 重复应用 inset

- 位置：[graph/mod.rs:4162-4164](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-scene/src/graph/mod.rs#L4162-L4164)。
- 严重度 P1，逻辑错误，置信度 10/10。
- 触发：普通容器 bounds 为 `(0,0,100,100)`，四边 inset 均为 10，
  安装 `StackLayout` 并添加一个 child；经 Runtime validation/layout 提交。
- `LayoutContext::get_container_bounds` 在 `layout/mod.rs:80-81` 承诺返回
  child 坐标域，但实际调用 `FigureNode::client_area`，返回 node-local
  `(10,10,80,80)`。`stack_layout.rs:59-68` 原样写为 child bounds。
  绘制再经 `FigureNode::child_transform` (`graph/mod.rs:496-506`) 加 `(10,10)`。
- 结果：child 实际从 `(20,20)` 开始，而不是 `(10,10)`；
  右/下内容被父 client clip 截掉 10。XY/Flow/Toolbar 等消费相同 origin 的布局也受影响。
- Java `Figure.getClientArea` 在 local 模式将 origin 置零，StackLayout 原样摆放；
  官方 coordinates 明确 children 相对 parent client area。
  Novadraw 统一 local 坐标是有意迁移，重复计入 inset 不是该迁移的要求。
- 建议：区分 node-local paint clip 与 child-content layout area，在引擎查询边界
  定义并返回后者；同时明确 scalable/viewport 的逆变换策略，避免逐个 layout 减 inset。
  不需要改递归渲染主循环。
- 测试局限：`m5_layout_contract.rs:15-33` 的 Stack 用例 inset 为零；
  viewport border 测试仅验证 transform/client extent，没有覆盖带 inset 的 Stack arrange。

### G2-F3: Border 南/东区分配与预留不一致

- 位置：[border_layout.rs:279-287](file:///Users/bytedance/Documents/code/GitHub/drawjs/engine/wasm_rust/novadraw/novadraw-scene/src/layout/border_layout.rs#L279-L287)。
- 严重度 P1，逻辑错误，置信度 10/10。
- 触发：容器 `200x200`，`BorderLayout::with_sizes(0,0,0,0)`，
  South child 使用 `BorderConstraint::with_size(South,150)`，另有 Center child。
- `border_layout.rs:254-263` 得 `south_h=150`、`center_h=50`；
  `281` 行摆放 South 时又裁至 `ch*0.5=100`，得到 `y=100,height=100`。
  Center 是 `y=0,height=50`，两者之间出现没有业务含义的 50 像素空带。
  East 同理：预留上限为 `cw-west_w`，实际摆放又取 `cw/2`。
- Java `BorderLayout.layout:178-226` 用同一实际子尺寸递减剩余区域；
  即使接受 Novadraw 的 50% 限制，预留与实际也必须采用同一值。
- 建议：先确定各区唯一的最终尺寸，再用该尺寸计算 Center 和所有 child bounds；
  另行决定 50% 上限是否属于产品契约，不混用两套上限。
- 测试局限：本组 `m5_layout_contract` 没有该非对称 Border 输入；
  内嵌 Border 测试只检查解析、构造与空容器测量。

## 详细语义映射

状态说明：“对应”仅表示所述静态路径相符；“合理迁移”有可解释的契约；
“明确收窄”表示 API 实际没有该能力，不等于已获准延期；
“算法差异”没有足够依据认定为有意。以下全部不是本组运行测试的通过声明。

### M01 LayoutManager / Constraint / Cache

- Java：`LayoutManager` 全接口、`AbstractLayout.invalidate/getPreferredSize`；
  parent 独占 manager/constraint，失效清缓存。
- Rust：`layout/mod.rs:33-153,301-357`，`graph/mod.rs:335-401,3694-3761`，
  `runtime/runtime.rs:1651-1756`。typed downcast、parent-owned map、提交前类型兼容检查，
  generation+hints 测量缓存与 `invalidate(reason)` 均存在。
- 状态：合理迁移。不是“完全无缓存”：内置算法无私有缓存，但图级已有缓存。
  `m5_layout_contract:227-369` 覆盖错误 output、constraint 与 cache；
  构建期低层 setter 不具备 Runtime 同等的入口校验。

### M02 Client Area / Size Box Model

- Java：`Figure.getClientArea:672-686`、`StackLayout.calculateMinimumSize/calculatePreferredSize`：
  local origin 清零；测量扣 hint inset 后加回尺寸 inset，并考虑 border preferred。
- Rust：`FigureNode::{client_area,child_transform}:484-507`、
  `FigureTree::{preferred_size,minimum_size}:2155-2252`。
- 状态：算法差异。origin 问题为 G2-F2。普通带 layout 的容器测量路径直接返回
  layout 结果，未统一加 inset/border preferred；Stack 等算法自身也未补偿。
  无 layout 的 intrinsic 路径则会 `owner_scoped_border_size`，两种路径不一致。
  例：Stack 唯一 child 为 `20x30`、四边 inset 10，其自然尺寸仍为 `20x30`，
  而应需要 `40x50` 的外框。既有零 inset 用例不覆盖该测量 delta。

### M03 XYLayout

- Java：`XYLayout.calculatePreferredSize/layout`：无 constraint 的 child 不参与；
  任一轴为 -1 时，以 constraint 的宽高向 child 查询 preferred。
- Rust：`xy_layout.rs:81-113,141-207`：支持 `XYConstraint`/Rectangle；
  缺约束跳过、固定位置和显式零值保留。
- 状态：部分对应，hints 是算法差异。measure 传容器 hints，arrange 固定传
  `(-1,-1)`，不是 constraint 的宽高。`width=80,height=-1` 的可换行 child
  会按无约束高度摆放；`d4_constrained_measurement` 证明外部 constrained Figure
  是现有扩展能力，但该测试使用自定义 ColumnLayout，不验证 XY。
  Rust minimum 使用 child minimum、任意负宽高视为自动，也是 Java 之外的行为；
  Java XY 的 inherited minimum 默认等于 preferred，不能仅按同名 API 判等。

### M04 StackLayout / FillLayout

- Java：`StackLayout` 分别取 children min/preferred 的逐轴最大值，layout 全覆盖 client area。
- Rust：`stack_layout.rs:14-69` 对应聚合与全覆盖；`fill_layout.rs:31-82`
  只测量/摆放第一个 child，是独立本地扩展，不应称作 Stack 等价实现。
- 状态：Stack 主算法对应，盒模型受 M02/G2-F2 影响；没有
  `setObserveVisibility` 开关，聚合包括隐藏 children。
  `m5_layout_contract:15-33` 只验证零 inset 的全覆盖。

### M05 BorderLayout

- Java：`BorderLayout.calculateMinimumSize/calculatePreferredSize/layout/setConstraint`：
  visible 且存在的区域消耗空间，TOP/BOTTOM 后 LEFT/RIGHT，再 CENTER；
  测量逐步减剩余 hints，同一区域后赋 constraint 的 child 替换原映射。
- Rust：`border_layout.rs:125-177,220-369`，typed region、可选固定厚度，
  Rectangle 兼容约束；measure 分 min/preferred，但所有 child 收到同一 hints。
- 状态：混合。固定厚度、默认预留四边、无约束自动分配是代码明确表达的本地策略，
  不把“不是 Java”本身报 bug；但不应宣称完整 Border 语义等价。
  G2-F3 是即使接受这些策略也存在的内部算法错误。
  另无 visibility 查询，重复区域会重叠摆放而非 Java 的单 owner 映射，
  measure 与 arrange 对无约束 child 的区域解释也不同。

### M06 GridLayout

- Java：`GridLayout.java:192-234,238-764`，约束含 hints/span/indent/grab/alignment；
  constrained width 下重测 wrapping；equal columns 在有 grab 时统一分配空间。
- Rust：`grid_layout.rs:128-297,319-478`：占位扫描防 span 重叠、track maxima、
  min/preferred 分算，约束 hints、indent、fill、正向 extra 分配均存在。
- 状态：部分实现，以下是算法差异而非可认定的有意迁移：
  容器 hints 被忽略；缩小时 `distribute_extra` 直接返回，无 track 压缩与 wrapping 重测；
  equal-width 仅在分 extra 前生效。两列初始均 40、可用宽 200、
  仅第一列 grab 时最终为 160/40，而不是 100/100。
  `m5_layout_contract:71-152` 的等宽用例让 span 覆盖所有 grab 列，未揭示此输入。
  span deficit 平摊所有 track、span grab 标记全部 track 与 Java 分配也不同。

### M07 FlowLayout

- Java：`FlowLayout.java:128-206,308-416`：按方向仅传主轴 hint，按行聚合、
  支持整行对齐、行内对齐与 stretch。
- Rust：`flow_layout.rs:70-100,143-205,246-280`：arrange 按方向传 hint，
  有超宽首项、换行、行间距、横纵转置。
- 状态：换行主干对应；API 明确收窄为起点对齐，不含对齐/stretch 开关。
  measure 却将双轴 hints 同时传给 child，与 arrange 及 Java 不一致；
  minimum 单独聚合 child minimum，也不同于 Java 默认 minimum=preferred。
  内嵌测试主要为空容器/构造，不能证明 constrained 内容的 measure/arrange 等价。

### M08 ToolbarLayout

- Java：`ToolbarLayout.java:122-192,280-387`：只给副轴 hint，必要时二次测量；
  主轴按 preferred-minus-minimum 比例收缩，副轴最终不小于 minimum。
- Rust：`toolbar_layout.rs:82-104,135-207`：比例收缩、方向转置、min/max、
  stretch、minor alignment 均可用。
- 状态：部分对应。arrange 查询固定 `(-1,-1)`，缺副轴受限测量；
  measure 未屏蔽主轴 hint。非 stretch 分支的
  `natural_minor.min(available.1)` 可小于 minimum：
  竖向宽 20、child min width 40 时得到 20，Java 得 40。
  这是算法边界，不是文档明确的收窄。现有 M5 测试只测 stretch=true。

### M09 FreeformLayout

- Java：`FreeformLayout.getOrigin` 默认 `(0,0)`，可开启 positive-coordinates；
  继承 XY 的 constraint preferred fallback。
- Rust：`freeform_layout.rs:8-60,93-193`，有限 origin、Option 轴、显式零值，
  不改写负坐标，无约束保留当前 bounds。
- 状态：强类型校验与内容域是合理迁移；positive-coordinates 是明确未交付能力，
  `doc/design/architecture/layer-and-freeform.md` 明确讨论其收窄性质。
  已固定一轴时仍以双轴无约束测量另一轴，具有 M03 同类 hints delta；
  现有 `d2_freeform_contract:347-434` 只用固定 intrinsic 值。

### M10 Validation Iteration / Error

- Java：`Figure.validate:2174-2181`、`DeferredUpdateManager.performValidation:198-214`：
  父先 layout，子后 validate，循环消费新增失效，finally 复位状态。
- Rust：`graph/mod.rs:1664-1889`：drain 批次、提升 highest invalid ancestor、
  递归处理、viewport children-prevalidation、预算诊断；
  `deferred.rs:553-620` 保留错误并阻止正常 update 的 repair。
- 状态：基本调度对应，预算/Result 是合理增强；预算实际统计 queued ID 数而非
  “轮数”，大于 10,000 个有限 queued ID 也会被判 non-converging，应与递归深度上限区分。
  `m5_layout_contract:417-441` 只测试真正重复失效的小预算，
  没有证明大批量有限输入的可用性。
  隐藏子树跳过、恢复再入队是调度收窄，不照搬 Java validate。

### M11 LayoutOutput Atomicity

- Java：`LayoutManager.layout` 可直接 setBounds；`Figure.validate` 无 output batch。
- Rust：`layout/mod.rs:227-295`、`graph/mod.rs:1991-2124`：
  snapshot + buffered output，先校验 direct child，再应用 bounds/visibility/effect。
- 状态：结构验证是合理增强，不能扩大为“所有失败原子”。
  当前 `validate_layout_output` 不校验 Bounds 的 finite/non-negative，
  普通 set_bounds 也不拦截；外部 LayoutManager 的 NaN 输出可进入 NodeState。
  作为外部错误输出的健壮性边界记录，不因理论坏输入升级为 P1。
  `m5_layout_contract:227-255` 和 viewport 内嵌测试只覆盖非法 child，
  不能证明数值和外部 manager 私有状态回滚。

### M12 Two-Phase Update / Panic

- Java：`DeferredUpdateManager.performUpdate:172-191`：非重入，先 validation，
  再 repair，finally 清 updating；新增 dirty 留下一轮。
- Rust：`deferred.rs:391-417,456-461,508-620`：dirty snapshot、updating guard、
  catch_unwind 后还原 dirty/invalid 工作、再 resume panic。
- 状态：对应并增强；不把它说成静默吞异常。
  `deferred.rs:1046-1071` 有 panic-once/retry 断言，但 Runtime 外层 fault 策略
  与独立 UpdateManager 的可重试行为不能混为一谈。
  `perform_validation` 独立低层入口也不是完整通知/repair 事务。

### M13 Update / Layout Notifications

- Java：`DeferredUpdateManager` 的 `fireValidating`、`firePainting` 是阶段调用。
- Rust：`listener.rs:50-74,204-230`，`deferred.rs:340-364,517-578`：
  typed records 在稳定边界 flush，含 source epoch/sequence，query 指向最新稳定场景。
- 状态：有意的时序收窄与合理迁移，不能当作可同步干预 layout 的 Java 事前 hook。
  `Validating` 名称并不改变 deferred observation 的实际时序。

### M14 Damage Projection / Coalescing

- Java：`DeferredUpdateManager.repairDamage:272-307`，逐父变换、求交、union 后 paint。
- Rust：`repair.rs:34-158,161-286`，node-local 起点、父 child transform、
  client clip、根域保守 AABB；小片合并、超过 8 区域退化为 union。
- 状态：通常路径对应；多矩形与 conservative union 是合理变体。
  OverflowVisible 祖先免裁剪是必要适配，但擦除转 owner 后的起点 clip 漏洞为 G2-F1。
  `UpdateManager::compute_damage` 仅 union 各 owner local rect，
  不能用于跨 owner 的根域 damage 判断；正常 repair 使用 prepare_damage_set。

### M15 RangeModel

- Java：`DefaultRangeModel.setAll/setValue`：max/extent/min/value 通知顺序，
  value 限于 minimum 与 maximum-extent。
- Rust：`range_model.rs:149-178,187-215,241-275`：
  finite/bounds 检查，extent 限于合法 span，状态整体提交，锁外通知。
- 状态：有意增强，Java 不做相同程度的 extent 规范化。
  `m8_viewport_contract` 覆盖 clamp、非法输入、listener remove。
  Mutex 防止数据竞争，不等于跨线程 listener 通知全局顺序保证；
  当前 viewport 模型为私有标准实例，未据此假设可注入恶意 RangeModel 的死锁路径。

### M16 Viewport Tracks / Layout

- Java：`ViewportLayout.calculatePreferredSize/layout`、`Viewport.readjustScrollBars`：
  只给 tracks 轴 hint，跟踪轴至少 minimum，其他轴至少 preferred。
- Rust：`viewport.rs:386-397,412-545`：set_contents 单 child、clamp、
  tracks width/height、range effect 和重绘存在。
- 状态：尺寸 max 主干对应；arrange 无论 tracks 值都传 `(area.width,area.height)`，
  与 Java 的非 tracks 轴 -1 不同；preferred 查询未先以 child minimum 抬高 hint。
  对宽度敏感的 contents 可能改变是否需要滚动，不能以固定 Rectangle 的 M8 测试证等价。
  range effect 在整个 output 的 child 校验后提交，内嵌
  `invalid_layout_output_does_not_commit_viewport_range_effect` 覆盖此保证。

### M17 Scale / Zoom

- Java：`IScalablePane.IScalablePaneHelper`、`zoom/AbstractZoomManager.primSetZoom/setZoomAsText`、
  两种 ScrollPolicy；scale 不累计改写原 bounds，fit 可以绕过普通 minimum zoom。
- Rust：`scalable.rs:67-138,184-232`、`zoom.rs:76-82,177-267,334-397`：
  unscaled intrinsic、scale invalidation、policy -> scale -> validate -> scroll；
  fit 不强制 minimum level，因此低于 0.5 本身不是 bug。
- 状态：协调顺序对应。freeform range 保留未缩放内容单位，由
  `(anchor + location*old_zoom)*(new_zoom/old_zoom)` 转换回新内容单位，是明确迁移。
  `m8_viewport_contract` 与 `d2_freeform_contract:438-497` 有 anchor/edge/fit 断言。
  普通 scalable 装 layout 后，LayoutContext 仍返回未逆缩放的 node client area
  （M02）；不能由无内部 layout 的缩放用例证明 arrange 等价。
  外部 ScalableFigure 的可写协调接入有限，见扩展性评价。

### M18 Freeform Extent / Layer / ScrollPane / Frame

- Java：`FreeformHelper.getFreeformExtent/invalidate/setFreeformBounds`、
  `FreeformViewport.readjustScrollBars`、`ScrollPaneLayout.layout`。
- Rust：`graph/mod.rs:1565-1641`、`layer.rs`、`viewport.rs:422-486`：
  nested extent 派生、hidden 仍贡献 extent、普通 child bounds 与 freeform extent 区分；
  viewport 用 extent union baseline，保留负 minimum。
- 状态：内容域/presentation 分离与稳定缓存是合理迁移；
  `d2_freeform_contract` 明确断言 dirty 时仍读取旧稳定 extent。
  `scroll_pane.rs:675-729` 两轮根据固定 preferred 决定 bars，只能确认固定尺寸场景；
  Java 经 ScrollPaneSolver，受 hints 变化的内容没有完整等价证据。
  `runtime.rs:3523-3785` 在稳定后提交、等待 in-flight completion、失败强制 full retry，
  是 SWT repaint 到保留式后端的合理迁移，不把质量门通过等同像素级 damage 完备。

## 变体与后置能力

### 合理迁移

- Rust parent-owned typed constraint、只读 snapshot + output、generation/hints cache；
  finite range 校验与锁外通知。
- 统一 node-local/child-content 坐标；freeform extent 与 presentation bounds 分离；
  未缩放内容域 range、保守根域 AABB、多 damage region。
- stable notification、明确 validation 失败、in-flight submission/retry。
  上述设计选择均不豁免 G2-F1/F2/F3。

### 明确收窄或本地策略

- Flow 缺少行/项对齐和 stretch；Stack 缺少 visibility observation 开关。
- Border 固定边厚、默认四边预留、无约束自动分区不同于 Java；
  这是代码表达的策略，不自动代表获准的完整 parity。
- Viewport 不公开替换共享 range model；ScrollPane 固定标准 viewport 组合；
  UpdateManager 是具体事务组件，不支持 Java 子类式调度替换。
- ZoomScrollPolicy 可替换，但 ScaleHandle 创建只 downcast 两种内置 scalable 类型，
  外部实现 `ScalableFigure` 不能直接获得同等的 ZoomManager 可写端口。

### 明确后置与未决 Delta

- Freeform positive-coordinates、scrollbar 按住连发已有文档后置/收窄说明；
  本组不伪装为新增算法 bug。
- Grid equal/grab、constrained wrapping、XY/Toolbar/Viewport hints、
  layout 盒模型和输出数值校验未找到相应“有意不支持”依据，
  必须保留为未决 delta，不能归入上述后置清单。
- 本组不改 parity 账本；主审应据这些方法级范围重新评估
  `layout.manager`、`damage.repaint`、`viewport.scroll_zoom` 的宽泛 verified 描述。

## 扩展性、复杂度与 Rust API

- 扩展性：外部 manager/constraint、Figure constrained measurement 与 ZoomScrollPolicy
  有真实接口；maximum size 属于 Figure 而非 LayoutManager，迁移位置合理。
  LayoutSnapshot 缺 child visibility/insets 查询，外部 manager 难以完整实现 Java
  Border 的可见性和 box-model 语义。sealed viewport effect 有助维持内部不变量，
  但不是通用外部 layout effect API。
- 算法复杂度：XY/Stack/Flow/Toolbar/Border 的单次排列为 O(n)，不含子树测量成本。
  失效上溯为 O(depth)，invalid Vec 的 contains 去重在批量 n 项下可达 O(n²)。
  Grid 空间为 O(rows*columns)，span fit 扫描依输入跨度增长，没有固定线性保证。
  damage 父链为 O(dirty*depth)，两两合并及重复扫描可能超线性；
  8 区域上限发生在合并之后，不限制合并前工作量。本组无基准，不作实测性能结论。
- Rust API：Box manager 与 parent-owned constraint 减少别名；LayoutOutput 的结构验证
  比直接 mutable tree 更可控。构建期 `revalidate` 会 expect，运行期应消费 Result/
  FramePreparation；`FigureTree::validate()` 实际只改有效位，不宜按名字理解为完整
  Java validate。底层 size 查询的 tuple 返回无法直接表达类型/数值测量错误。
- 七维度：逻辑/业务语义见三个问题与映射 delta；健壮性见 output 数值、
  validation budget；并发已检查 RangeModel 锁与通知边界，未发现可确认的当前
  Runtime 并发故障；安全无文件/网络/鉴权入口可构成确认风险；性能只作静态复杂度
  评价；纯命名、注释、日志风格问题未作为缺陷提交。

## 验证与交接

- 本组没有运行 cargo、测试、GUI、replay 或 benchmark；没有修改产品代码、测试、
  既有文档，没有提交 Git。所有触发结果是当前完整函数的静态推导。
- 主审回传：`workspace.quality`、`verify.update/event/scroll-pane`、
  G3-G5.5 replay 全部通过。此为主审执行结果，本组未读取执行日志，不能写成本组复跑。
- 主审另记录 `web.build` E0308，本组没有建立其与上述三个问题的因果关系。
- 后续建议由主审加入针对性反例：F1 old/new partial damage，F2 带 inset 的
  arrange+paint 坐标，F3 非对称 Border 大于半轴尺寸。
  六布局 hints/minimum/Grid equal-grab 的测试矩阵也需要扩展，
  现有 passing gates 不覆盖这些输入。
- 机器产物：同目录 `group_2.jsonl`，3 条；路径均相对 Git 根，行号指向当前源码。
