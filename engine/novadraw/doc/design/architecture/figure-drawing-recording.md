# Figure 局部绘制录制与不可变命令段

类型：`proposal`

能力归属：Core Graphics 拥有进入状态、命令段与错误；Core Figure／Runtime
拥有 typed 准入、输入证明、资产留存和帧冻结；Vello backend 拥有命令消费与编码。
platform 沿用提交和呈现协议，Editor 沿用当前 Core 几何，不新增编辑能力。

日期：2026-11-06。状态：具体协议候选与验证矩阵已形成，待独立复核。
本文细化[FigureTree 总体方案](figure-tree-projection-separation.md#局部-drawing-的录制与组合候选)
的录制门禁，不改变总体实施授权。本文尚未接受，所列新增 API 尚未实现；
现行公开契约仍由[公共 API 规范](public-api-contract.md)负责。
执行队列仍为 [P2 backlog](../../roadmap/p2-delta-backlog.md)中的唯一 FR-02。
验收见[录制验证矩阵](../../verification/plans/figure-drawing-recording.md)。

`api_semantics`：`figure.tree`、`paint.protocol`、`graphics.context`、
`clipping.strategy`、`border.protocol`、`damage.repaint`、
`frame.preparation`、`render.backend_session`。

## 本片要取得的资格

已生成的局部内容可能适合重复绘制，但不一定适合省略 paint。
paint 可以读取进入状态、当前 clip、先前 Figure 的文字统计或私有可变值。
本片分别处理回调省略和命令存储共享：先证明回调的完整输入有效，
再按原顺序借用不可变命令段。两项分别验收，不用调用次数替代复制证明。

首片采用总体方案的完整进入状态 proof。命令几何保留 node-local 表达，
进入 transform、clip 与 opacity 仍由当前遍历建立。命中时要求完整进入状态相等，
所以平移通常使录制资产 miss。首片可以减少稳定重绘的回调和命令复制，
不承诺平移后的 paint 命中。

位置无关的录制是后续独立资格：需明确受控环境允许哪些状态读取、
文字可见选择和设备覆盖如何处理，再另行接受。首片不通过删掉 transform、
clip 或统计字段来获得平移命中，也不改变原 paint 看到的状态。
机制面向任意 FigureDrawing；准入由输入与效果决定，不按文字或形状类型分支。

## 现行语义与接入位置

[Draw2D API 账本](../../parity/draw2d/api-coverage.md)要求
self → client-area／children → border 的顺序、状态隔离和既有裁剪。
Draw2D 的绘制协议不提供任意回调的纯计算保证；新的省略资格由显式 opt-in 提供。

本轮按已接受总体合同定向核对实现，得到以下接入约束：

| 入口 | 当前行为 | 本片约束 |
|---|---|---|
| `FigureDrawing::paint` | 只有不可变 Rust 接收者，没有纯计算与完整输入承诺 | `Arc` 或 `&self` 本身不授予回调省略权 |
| `PaintContext` | 读取 AA、stroke paint、有效 clip 与累计文字统计；可以修改状态 | 读取值、隐含状态和效果都进入资格 |
| `NdCanvas::replay` | 遍历并克隆 command kind，调整进入 transform 与 opacity | 保留显式 RecordedDrawing 回放语义；新段引用不调用此回放来证明零复制 |
| `RenderSubmission.commands` | 公共 `Vec<RenderCommand>` | 不可变段消费需要显式迁移公共容器；不能填空 Vec 后另藏真实命令 |
| `render_recursive.rs` | 分别绘制 content、children 与 border | 保留主循环；通过 FigureDrawing 包装与 NdCanvas 录制 scope 接入 |
| Vello Native／离屏消费 | capability、资源与编码均读取命令切片 | 所有入口共同迁移，不只改最终编码循环 |

## Typed 准入与候选 API

准入来源是 registry 的完整身份，包含 namespace、generation、注册 key、
payload 代次及存活状态。诊断字符串不能作为身份。内置与外部 crate 使用同一入口。
不为现有 FigureDrawing 增加只有内部实现能覆盖的缓存 hook。

下表冻结本次待评审的 API 名称、权限与返回值；接受后才进入公开 API 规范。
key 的类型参数遵守现有 typed identity 模型，跨域误用在编译期或受检绑定时拒绝。

| 候选 API | owner／权限与合同 |
|---|---|
| `graphics::DrawingRecordingKey<P>` | Core typed key；`P: DrawingRecordingPayload`，字符串只用于诊断 |
| `graphics::DrawingRecordingPayload::paint(&self, context: &mut DrawingRecordingContext<'_>) -> Result<(), DrawingRecordingError>` | opt-in 纯计算回调；只依赖 owned 不可变 payload、context 状态和受检输入，不发送事件、不写共享值 |
| `graphics::DrawingRecordingContext::paint(&mut self) -> &mut PaintContext<'_>` | 返回原进入环境中的绘制上下文；不提供 measurement、tree mutation 或提交权限 |
| `graphics::DrawingRecordingContext::input<P>(&self, source: FigureId, key: DrawingInputKey<P>) -> Result<PreparationInputSnapshot<P>, DrawingRecordingError>` | 保存完整首次值与嵌套依赖；source 受当前捕获 root 限制，不能借输入 reader 获取 Figure 可变引用 |
| `graphics::DrawingInputKey<P>`／`DrawingInputRegistry` | 共用现有 typed input 基础，作为独立 drawing family；注册 reader 的身份、容量和退休语义不能借用 projection 的资格 |
| `graphics::DrawingRecordingRegistry::register<P>(key, provider: Arc<P>) -> Result<DrawingRecordingBinding<P>, DrawingRecordingError>` | 准入纯计算 provider，返回 owned binding；注册替换产生新代次，旧 binding 不自动绑定新 provider |
| `graphics::DrawingRecordingBinding<P>::drawing(&self) -> Arc<dyn FigureDrawing>` | 返回同一受检包装；可作为 FigurePresentation content、geometry content 或 border，调用仍处于原绘制位置 |
| `runtime::RuntimeFrames::set_drawing_recording_budget(&mut self, budget: DrawingRecordingBudget) -> Result<(), DrawingRecordingError>` | 沿用 `Runtime::frames()` 配置 Runtime 自有容量；变更只作用于后续尝试，不能撤销旧 lease |
| `render::RenderCommandStream` | 不透明、不可变命令容器；替换 submission 的 Vec，支持 borrowed 顺序遍历与浅 clone |
| `render::RenderCommandStream::iter()`／`chunks()` | 分别返回 `&RenderCommand` 与 `&[RenderCommand]` 的借用迭代；不分配、克隆命令或 glyph backing |

binding 本身不含 frame／session 身份，也不授予出版权限。
Runtime 在进入既有 renderer 前给 NdCanvas 安装受限的录制 scope，
其中保留本次冻结 root、受检 reader、资源视图和预算。
wrapper 在该 scope 内取得复用资格；独立 CommandRecorder 没有该 scope，
仍执行纯计算回调并受自身 Graphics 错误和资源合同约束，不使用 Runtime 缓存。
需要共享输入却没有受检环境时显式报错，不能返回旧资产。

现有 live Figure::paint 与未知 FigureDrawing 保留必要调用。
采用者可以把 binding 的 drawing 交给现有 presentation，或在 live paint 中委托它；
直接将一个未知回调放入 Arc 不等于注册。
注册的 owned payload 必须不可变；内部可变性和可变服务只能从 input reader 获取。
不能用“revision 未变”掩盖共享值变化。

`DrawingRecordingPayload` 要求 `Send + Sync + 'static`。
DrawingInput payload 沿用 `PreparationInputPayload` 的完整值与留存要求；
reader 只接收受限 drawing 输入视图。registry 构造时显式给定留存字节容量，
不能以 Runtime cache budget 替代 provider／reader 注册存储预算。
cache 关闭时 wrapper 执行 live 回调，不尝试分配缓存候选；
启用后按本合同预留 scratch 和输出容量。

## 完整进入状态与实际输入

proof 记录当前 transform 及 transform floor、有效 clip 的完整结构和坐标域、
clip stack 身份、opacity、fill／stroke paint、stroke style、fill rule、
AA／text AA、font、局部 bounds、当前 path 状态和累计文字统计。
clip bounds 为 None 仍是实际值；两个不同 clip 只有相同包络不构成相等。
隐含默认 paint 和服务缺失也进入 proof。

进入状态由 Core 生成，provider 不能声明“没有观察”以删字段。
资源记录完整 ResourceId、revision、kind、ready 状态与 payload lease；
provider、字体／轮廓服务及注册身份同样受检。尺寸、解析样式和私有输入
只要实际参与计算就必须在 owned payload 或传递 proof 中出现。
保存 state 时允许复制小型元数据；动态 path、paint backing 和证明存储仍计费。

input reader 按现有统一 input-proof 合同保存完整首次值、None、条件分支和
嵌套读取。历史差异是 miss；本次操作首次值冲突是错误，不能重新读取覆盖。
验证中新发现的依赖加入传递闭包。reader 退休、预算耗尽和环错误保持 sticky，
即使 provider 忽略返回错误也拒绝采用。缓存输出及读取集原子替换；
失败不安装部分命令，不拆掉旧资产的依赖。

## 文字统计带来的效果限制

`PaintContext::text_paint_statistics` 当前表示本次录制的累计 CPU selection 工作，
并且是 painter 可观察的输入。若命中时不执行文字选择，计数不能冒充仍执行了选择；
若又把旧计数加回去，统计就不再表示当前 CPU 工作。

首片只允许缓存文字统计增量为零的录制段。cold 回调产生非零增量时，
本次输出照常使用，但不安装可省略回调的 cache entry；不能为探测资格调用两次。
任何 provider 产生该效果都采用同样规则，资格不按文字 Figure 类型划分。
普通文字绘制仍调用真实 callback；其生成存储可按新容器持有，不能宣称 glyph
生成已经省略。后续若分离 painter 可观察的逻辑状态与实际工作诊断，
必须单独评审 getter 的语义及未知 painter 的兼容性。

## 录制段、栈与组合

录制器保存 callback 新增的原始操作，不把进入状态序列复制到段中。
当前遍历的前置命令已经建立进入状态；完整进入 proof 相等时，直接消费原段。
不额外重放 world transform，不再次乘 opacity，不对矩阵求逆。
段内绝对 SetTransform 保留其原语义；状态不同则重录，有限奇异矩阵不因缓存而拒绝。

每段独立记录 state-stack floor、进入状态和退出状态。
Pop／Restore 不能跨 floor；回调完成检查平衡。退出时 Core 采用已验证的退出状态，
后续命令及外层 Pop 的行为必须与 live 绘制相同。当前 path 和隐含状态也必须恢复，
不能仅恢复 transform。self 的外层隔离、children 的 client-area clip、border 的
进入位置仍由原 traversal 决定，未知 callback 按原位置执行。

成功的段同时包含不可变命令 backing、资源闭包、输入证明及留存 lease。
provider 返回错误、忽略 Graphics 错误或栈错误时，保留首个原始原因，
回收候选命令并恢复该段环境；整帧沿现行失败链拒绝，不执行未知回调来掩盖错误。
错误恢复不能清除原先已有的 Graphics sticky error。

候选 `DrawingRecordingError` 保存结构化 kind、注册／source 身份及 owned 原始
`Error::source`。kind 区分注册、输入、资源、预算、状态、无受检环境与 provider 失败；
不把 miss 当作错误，也不将所有原因改成诊断字符串。
来自 PaintContext 的 GraphicsError 保留原值；其他录制错误经新增
`GraphicsError::DrawingRecording(Box<DrawingRecordingError>)` 进入现有
`FramePreparationError::Graphics`。这两个包装的 `Error::source` 必须继续传递，
包括补齐当前帧 Graphics 分支的 source；最终校验错误走相同通道。
DR06／DR08 验证能够追溯原始 source，不能只验证最外层 kind。

## Submission 容器迁移与所有消费者

`RenderSubmission.commands` 的候选类型改为 RenderCommandStream。
内部采用顺序 chunk 表：本次直接生成的 owned 段与既有不可变段的 Arc 引用，
保留 self／children／border 的实际顺序；不建立嵌套树形命令解释器。
stream clone 只共享 chunk 表及 backing。引用项复制与新分配分别计数。
`iter`、`chunks`、命令数和空判断均不自动 materialize Vec；
需要 owned flat Vec 的旧调用方必须显式迁移并承担复制成本。

这项修改影响公开 struct 构造及所有命令检查，需在实现批次同步迁移：
Runtime 帧冻结、独立场景捕获与回放适配、host background、Vello capability 检查、
资源扫描、Native／离屏编码，以及测试 backend 和公开 API 消费者。
原 RecordedDrawing 的显式 snapshot／replay API 保持，转换边界明确记录复制；
不能用该兼容边界作为新热路径。

新容器仅改变所有权，不增加新的绘制操作，backend 仍逐条消费原 RenderCommand。
所有 backend 必须使用统一迭代检查和编码；若某 backend 尚不支持新容器，
在资源采用和输出修改前显式 Unsupported。不能仅校验 direct chunk、
跳过共享 chunk，或在编码时静默丢段。
Vello 仍可能为编码遍历全部命令；本片不授予 backend 编码或栅格缓存资格。

## 峰值留存与失败

候选 `DrawingRecordingBudget::new(cache_bytes, in_flight_bytes, scratch_bytes,
chunk_entries)` 使用显式字节与条目单位。零值不赋予对应缓存／留存资格；
默认不启用回调缓存，非零生产默认值需以 cold／warm 成本和容量证据另行确定。
已有 preparation work／allocation budget 继续累计，不能因每段开始而重置。

cache eviction 只撤销查找资格；prepared snapshot、submission、Presented 恢复前缀
和外部 clone 所持 lease 最后释放时才归还实际留存。
每个独立留存责任保守计费资源闭包和 backing，单一责任内按完整身份去重。
计费包含 command capacity、path／gradient／dash backing、完整 glyph／variation
buffer、Arc／chunk 元数据、输入证明与资源 payload；不能只按选中的 glyph slice 长度。
若与现有 preparation retention 共用 allocation，必须明确唯一计费 owner，
不能让两个账本都认为由另一个持有。

重建前同时预留旧资产、candidate scratch 和新输出所需峰值容量。
普通 cache miss／淘汰在预算内重录；cache 准入不足可以不保存 entry，
本次成功命令仍按现行帧预算采用。in-flight、scratch、条目上限或算术溢出
导致无法安全构造当前帧时显式失败，保留原输出与恢复工作。
不得在失败后无限制展开共享段成 Vec 来绕过预算。

## 最终检查与推进顺序

录制命中仍验证当前资源、输入闭包、Graphics 错误和 backend capability；
冻结时绑定当前资源因果前缀与 session，不把旧 segment 的 frame id 带入新帧。
匹配 Presented 才更新显示恢复前缀，失败或 Skipped 保留旧前缀。
未知墨迹仍 Full；录制命中不提供新的可见剔除或 damage 包络资格。
session／device 恢复重建资源基线和完整输出，不能因 Core 命中跳过。

独立复核应同时查看本合同与验证矩阵，重点确认完整状态、文字效果限制、
公开容器迁移和峰值计费。复核通过后，在已授权 FR-02 内依次完成：

1. 先迁移不可变命令容器及全部消费／拒绝入口，证明新旧像素和失败行为一致。
2. 再接入 typed provider、完整进入 proof、资源和留存；未知回调保持 live。
3. 最后测量 cold、warm、state miss、淘汰与恢复，分别报告调用、生成、
   深复制、引用复制和净成本；再评估后续位置无关资格。

该顺序保护已完成投影切片。原 workset 冻结基线未恢复时，
相邻版本只能证明当前切片，不替代 FigureTree 总体净成本，不关闭 FR-02。
