# FR-02 输入证明传递实施记录

类型：`verification-record`

能力归属：Core（`novadraw`）Preparation、布局协调与 Runtime 派生事务。
状态：实施中，2026-11-04。本文记录共享内容基础片之后的输入证明传递，
以及不可变 Flow 消费迁移，不代表贡献组装复用或生产启用完成。
合同见 [FR-02](../../design/architecture/flow-assembly-reuse.md)，
执行状态由 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。

测量结果原先只带尺寸与 Core 观察记录。扩展的私有输入即使在测量返回前有效，
也可能被后面的回调改变；如果没有保留首次证明，布局或最终发布便可能采用过期结果。
本片补上自动收集和传递路径，继续保持现有布局反馈与派生事务提交顺序。

## 实际接口与状态

公开的 typed key、payload trait 和 reader registry 位于
`crates/novadraw/src/figure/preparation/input.rs`。内置与外部消费者使用同一注册入口。
payload 是受检读取所得的 owned 不可变输入值，完整相等比较不依赖名称或散列。
`PreparationScene::input` 同时返回只读 evidence 并保留首次证明。

registry clone 保留已有注册 token；证明用弱引用读取注册实例。
最后一个注册 owner 销毁后，旧 evidence 不变，重验拒绝退役 reader。
reader 返回的原始错误与输入变化分开处理。输入变化错误保留首次和当前 typed
payload，可以按 key 读取；诊断不对任意 `Any` 值执行 `Debug` 格式化。

| 检查点 | 实际状态与传递 |
|---|---|
| 测量返回 | 子测量场景收集首次证明；返回尺寸前重验，成功回执进入调用方收集器 |
| 测量缓存命中 | cache record 保留相同证明，命中时合并并重验，冲突不能当作 miss |
| 布局消费 | manager 返回后、采用 `LayoutOutput` 前重验；成功后证明随容器布局派生状态留存 |
| 准备结果 | 新候选携带证明；本批复用的旧结果也进入批次检查，防止后面的回调改动其输入 |
| 最终发布 | Runtime 在协调和路由检查结束后，统一检查当前布局与准备结果的首次证明 |
| 失败恢复 | 派生 checkpoint 恢复旧布局证明、旧几何和已成功发布的准备结果；source mutation 按既有规则保留 |

移除或替换布局管理器会清理其旧测量证明。子图的准备证明不随该操作清理；
最终检查只采用当前挂载树的布局与准备证明，分离节点不进入当前发布依赖。

`ValidationError` 与 `FramePreparationError::Validation` 原先没有转发布局原因的
`source()`；本片补齐转发，让输入原因经过布局、验证和帧错误后仍可追溯。

## 不可变组装的实际消费

`FlowPlan::freeze` 把本次组装结果转为不可变 `FrozenFlowPlan`。
完整段落请求使用 `StyledTextRequest::from_content`，读取共享文本与 spans。
准备快照通过 `Arc<[FlowSourceRange]>` 共享来源数组，ancestors 不因快照输出复制；
快照还持有组装 backing，服务销毁后旧快照仍能读取完整内容与来源。

宽度相关的 inline box 数据单独生成。前缀排版、当前剩余宽度下的 Figure 测量、
超宽重测及边框 marker 顺序继续执行。此时每次调用仍运行完整贡献组装，
不能把内容共享解释成整页命中或已省掉 contributor／handler。

`FlowParagraphSnapshot::sources` 的公开类型从 Vec 改为 Arc slice；
已核对仓库消费者的索引、slice 与迭代使用，没有修改其来源读取逻辑。
外部构造或修改该字段的调用方须迁移到共享只读来源。

## 验证状态

`cargo test -p novadraw --lib figure::preparation::input::tests` 的 7 项测试已通过：
首次值冲突、退役 reader、原始错误、测量缓存、准备批次、布局消费，以及最终 Runtime
检查。最后一项由父容器的 lifecycle 回调在布局采用测量后改变输入；后续准备重新
捕获新输入，但保持相同绘制尺寸，Runtime 仍拒绝旧测量所产生的布局并恢复旧绘制。
这验证了证明不能在后续准备时被新值替换。

随后完整 Core lib 回归通过：454 项通过、1 项忽略。
共享内容与公开注册合同的 6 项测试、结构化 Flow 的 22 项回归均通过；
不可变消费的 5 项定向测试通过，其中新增用例直接比较内容、spans、来源数组和
ancestors 的 backing 地址，并验证服务销毁后旧快照仍可读。
这些测试分别对应基础证明、共享消费和既有 Flow 行为，不覆盖整页缓存命中。

新增反例继续验证同 generation 的完整 payload 改变、foreign Figure 和预算拒绝时
不调用 reader，以及布局管理器退役与子图证明的独立寿命。
公开消费者另增加 Runtime 内实际读取、发布和保留旧 evidence 的用例。
新增后输入证明的 10 项测试、公开合同的 7 项测试已通过。
测量、布局、准备、准备场景路由、路由恢复及外部 Flow 引擎共 50 项回归通过；
workspace check、Core lib Clippy（`-D warnings`）、Native／Wasm 架构与公开符号门禁
均通过。独立消费者的 7 项测试及 Wasm 编译通过，文档门禁检查 488 篇 Markdown
通过；格式和 diff 空白检查通过。

本地日志放在 `target/verification/text-fr02-2026-11-04/input-proof-tests.log`。
扩充反例为 `input-proof-expanded.log`，公开合同为 `input-proof-contract.log`，
受影响回归为 `input-proof-regression.log`；同目录保存 API 与消费者检查日志。
此前编译夹具修正与测试发现的批次旧结果遗漏，均不作为通过证据。

## 尚未交付的范围

FlowContributor／handler 的完整可选声明、registry 配置绑定、
整页 assembly asset 的跨调用采用和命中，以及服务家族留存账本仍待实施。
本片只使用 Preparation 的调用预算，尚未完成 proof 的跨调用容量与唯一 backing
计费。输入 reader 的并发写入域不具备当前复用合同；现有接口要求稳定化期间无并发写入。

长文本样例尚未接入贡献组装复用，因此本阶段不提供滚动提速结论，也不要求用户
手动验证复用效果。样例接入并通过自动回归后，再交付启动命令和手验范围。
