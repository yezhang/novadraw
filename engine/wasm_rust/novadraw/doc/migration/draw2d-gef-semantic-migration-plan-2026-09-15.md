# Draw2D / GEF 语义迁移实施方案

类型：`proposal`

日期：2026-09-15

本方案基于
[`2026-09-15 审计证据`](../../verification/evidence/draw2d-gef-semantic-audit-2026-09-15/review_groups.md)
形成，属于待评审实施建议，不替代现行 ADR、设计 SSOT 或路线图。
实施时继续沿用 Draw2D 的 M1-M10、后续 P2 delta 与 Editor 的 G0-G6 编号体系。
以下“批次”仅表达依赖顺序，不另建里程碑编号。

## 1. 迁移准则

必须保留的对象是可观察行为及其扩展契约：有序树、统一坐标和命中、状态隔离、
validation 先于 repair、连接依赖失效、模型驱动视图、命令历史，以及可替换的
Figure/Layout/Anchor/Router/Policy/Tool。Java 类层级和方法拼写不是兼容性目标。

允许主动改变：SWT 依赖、继承式实现共享、可变对象别名、整数像素为唯一表示、
共享静态临时变量、无类型约束、依赖异常表示日常拒绝。改变之后必须仍能完成同一
行为，并在替换点提供外部实现证据。

以下理由不足以证明语义等价：

- 内置 demo 可以完成某操作；
- 已有同名 trait、struct 或 `verified` 标签；
- Rust 借用较难，所以删除原有扩展能力；
- 当前只需要两三种策略，所以永久封闭策略集合；
- 新设计已经获得批准，因此可代替实现或验证证据。

“已批准的收窄”应如实标记为收窄。尤其 remove/dispose、延迟 listener、固定 clipping、
全局连接快照与封闭 Tool/Request，不宜合并成一个笼统的“Rust 合理变体”。

## 2. 推荐执行顺序

| 批次 | 目标与改动边界 | 前置依赖 | 交付与退出条件 |
|---|---|---|---|
| A：固定基线及账本校准 | 为审计中的差异绑定 family、源码快照、测试、失败场景；区分缺陷/收窄/后置 | 本审计交付 | 工作区稳定；fmt/check/clippy/test 明确通过或记录失败；不得沿用旧测试结果证明新代码 |
| B：修复确定的行为差异 | 先修复事件 target/hover 分轨与 Policy target 身份路由，再处理审计确认的其他算法和输入错误 | A；设计冲突先修订专题契约 | 每项有独立反例、修复和回归；保持当前成功场景；按主题原子提交 |
| C：恢复真正的扩展端口 | Tool/Tracker、typed custom Request、目标路由与 source/target feedback；按证据开放 clipping/连接几何扩展 | B；ADR-014/015 变更评审 | 外部 crate 不修改引擎源码，即可新增一种工具、业务请求、策略与反馈 |
| D：闭合 G5 当前连接切片 | 连接 Anchor/Router/几何策略、bendpoint、非单位缩放和 viewport/auto-expose | B、C 中涉及连接和输入的契约 | G5 自动契约 + 检查点 C；包含 self-loop 双端点、取消、失败、undo/redo |
| E：完成 G6 产品验证 | 保存加载、重建身份、同一编辑事务 Native/Web/Headless；明确多 Viewer/多文档 history 所属 | D | 三平台同序列产生等价模型与可解释视觉；保存/重建不依赖旧 FigureId |
| F：按产品需要扩展后置能力 | TextFlow、复杂图布局、高级 Graphics、完整 widget、direct edit/IME、clipboard、snap | 核心扩展端口稳定 | 独立 scope 和验证矩阵，不回写历史 M1-M10 完成定义 |

优先恢复可扩展性，再增加更多内置策略。否则新增能力会继续扩张中心类型分支。
批次 B 的独立修复可以并行，但同一 Runtime/Viewer 公共接口的变更必须串行合并。

## 3. 确定行为差异的实施契约

### 3.1 Figure 查找与事件分发

保留一个几何遍历内核；将“可见性与裁剪”作为公共搜索约束，将“是否 enabled、
是否愿意接收某种输入”作为事件搜索策略。普通 `findFigureAt` 等价查询必须能够找到
仍然显示的 disabled Figure；不能把输入资格当作全部空间查询的资格。

目前更窄的 `tree-search-and-focus.md` 把 enabled 放入共享内核。
因此该项先修订设计与映射账本，再改实现；不是直接删除条件后就宣称完成。

将 mouse target、物理指针下的 Figure、tooltip owner 明确区分：

```text
查找输入目标与指针下 Figure
-> 根据 capture 确定本次 mouse target
-> 旧 mouse target exit
-> 新 mouse target enter
-> 向 mouse target 分发主事件
-> 独立更新 tooltip/rollover 等派生状态
```

对捕获期间 rollover 是否跟随物理指针，必须单独定义；不能靠错误移动 mouse target
来实现 Button 的 drag-out/back。鼠标按键集合、modifier、pointer ID、拖动判定和 cancel
也应进入规范输入对象；`Dragged` 不应只由“已消费并 capture”推断。

退出验证：可见 disabled 节点的通用命中与事件命中不同；交互父节点包含无 handler
子节点；进入/离开子节点但 mouse target 不变；跨节点捕获；release/cancel；坐标缩放；
回调请求删除自身；Figure widget 与 Editor Tool 输入互斥。

### 3.2 Policy target 路由

拆分“找出接收者”和“收集命令贡献”。`target()` 返回的 ID 必须实际参与路由，
并验证所属 Viewer、生命周期和重定向循环。无贡献、拒绝和非法目标使用不同结果。

同时保留 GEF 的 source/target 区别：移动原对象时 source 提供 MOVE/ORPHAN；
跨容器接收时 target 提供 ADD。不能把所有 source command 一律转交 target。
feedback 的 owner 和坐标域使用同一解析结果。

退出验证：默认 self、child→parent、foreign、disposed、循环重定向、多 source、
target 拒绝、source/target 独立反馈、失败后 overlay 清理。

### 3.3 算法和绘制修复

各域详细差异见分组证据。修复前先把具体输入和 Draw2D 的语义结果写成可执行断言；
几何计算比较允许明确的浮点误差，裁剪/命中/损伤范围则必须保持一致。
像素对比区分抗锯齿差异与漏绘/越界等功能错误。

保持 `render_recursive.rs` 保护约束。若证据指向 clip command、Figure 协议或 backend，
在那里修复。只有证明递归主协议与 Draw2D 不符，才单独评审主循环变更。

## 4. Rust API 设计建议

| 边界 | 建议 | 保留的核心语义 | 避免的新局限 |
|---|---|---|---|
| Figure 身份与树 | 保留 namespaced generational ID、受控 Runtime 写入和只读 query | 单父树、稳定顺序、退休句柄失效 | 不使用地址/全局计数器推断身份；不复制拓扑 |
| 错误结果 | 新 API 优先 `Result<ChangeOutcome, Error>`，区别未变化与非法操作；旧 bool/null convenience 明确标注 | 拒绝不造成部分提交 | 不把失败隐藏为 `false`/null；内部 panic 不伪装业务拒绝 |
| 组件内容更新 | 保留 owned typed update → prepare → commit；默认保守失效 | 外部 Figure 可以更新且触发布局/路由/重绘 | 不增加逐 Figure 的 Runtime match；不承诺类型系统证明任意扩展纯函数 |
| 生命周期 | `dispose`、同域 reparent、跨 Runtime 模型重建保持分离 | 激活/停用、引用清理、资源归属 | live detach 如有需求，另设计保活/重挂载；不得把 dispose 声称为 Java remove 完全等价 |
| 监听与行为 hook | 稳定观察 journal 与内部失效/lifecycle 分离 | old/new 因果事实及更新一致性 | 延迟事件不能替代事前 layout/veto hook；不把最新 query 当历史快照 |
| Tool | 一个 active Tool 端口，规范输入与受控 ToolContext；独立 tracker 生命周期 | 输入解释可替换、源固定、清理后执行命令 | 不继续增加 Domain 内置工具字段；不把键盘解释散入 apps |
| Request | 内置 typed enum + 应用关联类型扩展，或经实际外部用例选择对象安全 trait | 应用定义新操作并复用 targeting/policy/history | 不复制无约束 Any map，也不永久封闭请求种类 |
| Policy | 角色 key、可控动态替换与 target resolution；source/target feedback 清晰分开 | 行为组合与生命周期 | 不能只开放 Custom role 名字，却无法表达新请求或目标 |
| Anchor / Router | 只读上下文、显式依赖、typed constraint；节点端点策略可注入 | 可替换定位与路由、owner 变更触发重算 | 不把 editor 的连接端点永久绑定某个内置 Anchor |
| 连接几何 | 路由与路径表达分层；绘制、命中、bounds、tangent/locator 共享几何事实 | Connection 仍是可编辑 Figure | 不能只改 paint 做曲线，hit/handle/dirty 仍沿旧折线 |
| Layout | parent-owned 约束；明确拒绝类型；measure/arrange 分离 | 约束、preferred/minimum 与缓存失效 | 不能以“无缓存”豁免错误最小尺寸或拒绝第三方派生状态 |
| Text / Backend | 保留 `TextLayoutEngine -> Glyph IR -> RenderBackend` | 测量与绘制一致、后端可替换 | 不把 shaping 移到 Vello；字体 fallback 来自显式资源 |
| 集合与算法 | 顺序用 Vec；成员查找/去重用集合；错误事务先校验再发布 | z-order、命令组合、source/target 次序 | 不因 HashMap 遍历改变语义；性能优化需基准 |

这些是契约方向，不是已经稳定的 Rust 签名。新增公共端口必须通过外部 crate 编译与
行为验证后再冻结；优先保持现有 API 的迁移适配期。

## 5. Connection 迁移的专门门禁

默认 self-loop 保持矩形外部回环与两个独立 endpoint handle。这是本项目的编辑契约，
不是 Draw2D 所有 router 必须采用的形状，更不能用 Zest 产品行为定义。

替换路由/几何策略必须同时证明：

1. source/target anchor 独立，可动态响应 owner、ancestor 与资源/几何变化；
2. 路由输入、约束和输出有明确坐标域；同节点参考点退化有稳定策略；
3. Polyline、圆弧或 Bézier 的 bounds、paint、hit、locator、endpoint tangent 一致；
4. endpoint 与 bendpoint 的模型身份和索引不因采样点改变；
5. 路由失败不发布半组结果；解除/重绑恢复依赖不丢失；
6. 现行严格 viewport topology 仍显式拒绝不支持拓扑；
   扩展到跨 viewport 时必须连同 clip region 和 damage 映射一起设计。

不承诺“一种路由算法适配全部拓扑”。ShortestPath 等内置策略缺席和策略接口不能替换
是不同问题，应分别排期。

## 6. 验证矩阵

| 验证层 | 必须证明 | 通过标准 |
|---|---|---|
| 单元/契约 | 具体反例、非法输入、生命周期、顺序、坐标、失败恢复 | 修复前反例可失败，修复后通过；断言输出行为而非内部结构 |
| 外部扩展 crate | 自定义 Figure/Layout/Anchor/Router/Locator/Tool/Request/Policy | 不修改引擎源码；使用公开 API；含一次状态更新、失效和销毁 |
| Headless 编辑事务 | create/move/resize/delete/connect/reconnect/bendpoint/undo/redo | 模型最终状态与投影一致，历史不保存活 FigureId |
| Native / Web | DPI、scroll/zoom、capture/cancel、键盘、窗口失焦 | 同一操作序列产生相同模型；视觉差异有明确允许范围 |
| Rendering | state stack、嵌套 clip、旧/新 damage、文本资源更新 | 命令与像素证据共同验证；包括 retained partial |
| 恢复 | 扩展 panic、未知状态、backend session 重建 | faulted 拒绝后续写入；重建恢复完整资源和场景基线 |
| 深度/性能 | 10,000 层协议与已有性能基线 | 不恢复迭代渲染主线；不以未测量“更快”作为迁移依据 |

每个语义 family 的状态拆成“规范已定义 / 公共 API 可达 / 行为已测 /
外部替换已测 / 平台已验收”。单一 verified 标签不足以支持全量语义兼容承诺。

## 7. 文档与提交落地

先修改最窄的设计 SSOT，必要时修订 ADR-014/015；再更新 Draw2D/GEF family 账本，
最后更新现行 roadmap 与验证记录。本文不能直接把 G5.4 或 G6 标成完成。

审计报告保留原判断与快照。修复后追加关闭证据，不能重写历史为“从未存在问题”。
各主题分别提交，摘要使用中文。大范围变更按接口、实现和验证分步组织，
避免同时重写事件、布局、渲染和编辑历史。

工期需在批次 A 固定最终差异与接口影响后估算。当前不具备团队容量与稳定变更范围，
因此不提供缺乏依据的日期承诺。
