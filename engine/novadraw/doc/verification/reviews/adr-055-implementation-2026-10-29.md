# ADR-055 容器编辑实施验证

类型：`implementation-verification`

日期：2026-10-29。依据用户批准的 [ADR-055 修订 2](../../adr/adr-055-container-editing-protocol.md)。
公共协议、移动与创建/drop 已接入；无窗口契约和独立消费者通过。
Native/Web 实际输入验收尚未执行，P2-E03 保留 `in_progress`。

本轮续验已补创建视口变化与已取得反馈的清理拒绝，修复同步重复分配；
最新证据和剩余范围见[续验复核](adr-055-followup-2026-10-29.md)。

## 实现边界

`crates/novadraw-editor/src/container/` 提供批次、候选、只读声明、资格与受检计划。
`ContainerMoveTool<A, P>` 固定活动协议，由各容器 policy 将同一候选转换为 XY 或
Ordered 约束；`ContainerCreationTool<T>` 与 `drop_into_container` 共用独立模板协议。
两种 Tool 都经公开 ToolContext 服务求解、显示反馈和提交命令，外部 Tool 可复用。

`aggregate_at<R>` 校验显式 host 与真实 source，直接聚合该 host 的注册 handler。
`AtMostOne` 在 reducer 前后拒绝重复接纳；普通 dispatch 的重定向语义保留。
统一 `CheckContainerPermission` 按本地修改、各原父移出查询精确有序子批，
回执由框架签发。批次、候选、回执和 checked wrapper 均不能从外部直接构造。
源父分组与 generation 去重使用身份索引，保留有序操作语义，避免多父批次的平方级分组扫描。

唯一锚点产生一次 surface 平移，各 source 使用 `T⁻¹ · D · Sᵢ` 映射。
实际 content pane、inset、scroll 与 scale 来自 Viewer/Core 坐标查询。
数学奇异和数值失败分开返回，XY 拒绝不可表示的 skew/旋转，不静默取包围盒。
源快照使用共享 Arc；视口变化重算映射，model revision 和相关 policy generation
变化取消手势。成功模型命令与 history 不因后续投影失败而回滚。

提交前先完成反馈清理，再校验当前候选，最后构造一次模型命令。
命令包装器在 ViewerEditSession 完成同步后的实际 execute 边界再次检查 revision。
取消、换工具以及正常结束的错误路径释放 capture；清理失败保留原异常并阻止构造。
execute/undo/redo 以稳定 ModelId 保存并恢复完整 selection、primary 与独立 focus。

## 自动覆盖

主夹具：`crates/novadraw-editor/tests/p2_e03_container_editing_contract.rs`。
独立 workspace：`tests/consumers/editor-open-protocol/`，直接包含同一公开 API 夹具。
独立消费与同 Viewer 跨布局是两组断言：前者使用不同 Request/Payload/Plan，
后者保持同一个活动 P；另一个 family 的容器只在无贡献时向祖先探测。
创建使用独立 Payload/Plan，通过相同 Tool/drop 提交。

| 矩阵项 | 已执行证据 | 实际范围 |
|---|---|---|
| CE-01/08/12 | ancestor_dedup、mixed_connection_selection、mixed_local_and_incoming、multi_original_parents | 祖孙去重、后代锚点、连接保留、不同原父精确资格、完整选择/focus 与历史恢复 |
| CE-02/05/10 | checked_services、stale_header、raw_and_reduced_duplicate、rejected_parent_permission | 空/重复/foreign/retired/root/cycle、旧候选声明、reducer 前后重复、拒绝不构造或显示反馈 |
| CE-03/04/16 | source_subtree_and_feedback、unsupported_inner_family、same_tool_accepts；G4 host 聚合对照 | 排除源子树与反馈、普通重定向/显式 host 区分、未支持向祖先、拒绝终止、不同协议和同 P 跨布局 |
| CE-06/07 | actual_content_panes、xy_geometry；Core checked_inverse 单测 | inset、不同 content pane、非均匀缩放、同旋转、反射、轴交换、skew/溢出/极小可逆尺度/零尺寸 |
| CE-09 | ordered_insertion、multi_original_parents | Before/After/空目标/操作顺序/source reference 拒绝、无变化无历史 |
| CE-11 | parent_creation、nested_click_and_drop、creation_rejection | 父域点击/拖动/drop、默认尺寸、显式偏移、创建取消/拒绝无对象与历史 |
| CE-13/14 | cancellation_tool_switch、external_revision、release_recomputes、stationary_pointer、held_candidate、stationary_multi_selection、stationary_creation | 删除/换工具/显式宿主取消、policy invalidation、release 最新位置、固定创建起点；移动与创建在静止指针下通过视口变化续验 |
| CE-15 | retired_acquired_feedback、feedback_cleanup_panic、failed_model_execute、successful_model_and_history；S06 overlay/Core 单测 | 两种 Tool 已取得反馈的提前退休拒绝与 lifecycle panic 阻止构造；模型拒绝无历史；投影失败保留成功 history；真实 Core prepare 拒绝仅有 Core 私有单测 |
| CE-17 | editor.contracts、extensions | 既有 Editor 与公开扩展回归；最终 workspace 门禁另记 |
| CE-18 | release 成本夹具、同配置产物消费者 | 小/宽/深树阶段样本、完整提交与同步、有序扫描、实际产物体积 |

CE-15 的 prepare 拒绝采用既有 `viewer::failure::tests::s06_cleanup_driver_keeps_real_refusal_and_continues_independent_roots`
与 overlay preflight 测试证明共享服务行为。前者是 synthetic-root dispatcher 拒绝，
不是容器反馈正常取得资源后的真实 Core prepare 拒绝。续验增加了提前退休反馈的
真实 Tool 清理准入拒绝；Core revision 耗尽端到端场景仍缺证据。
外部 header 字段只读，compile-fail 检查身份/有序子批字段篡改以及完整凭证构造，
不使用缺字段的空 initializer 证明私有边界。运行时测试拒绝来自旧候选的声明，
不把不可公开构造的逐字段伪造计为已执行反例。
静止指针下双轴 auto-expose、scroll、resize 已有无窗口移动与创建测试，
创建还组合 zoom；真实 DPI/capture 的全部组合仍待平台续验。

## 性能

环境：macOS/aarch64，Rust 1.96.0；主 workspace release 配置。
每场景每阶段 64 个样本，无窗口、无 GPU/present；并发构建期间采集，
数值仅用于本次成本核验，不作为稳定性能基线或优化前后比较。
最终原始数据见 [release 日志](../evidence/p2-e03-container-cost-final-2026-10-29.log)。

| 场景 | nodes / sources / depth | 几何 p50/p95（µs） | policy p50/p95（µs） | feedback p50/p95（µs） | 完整提交 p50/p95（µs） | revision 同步 p50/p95（µs） |
|---|---|---|---|---|---|---|
| small | 18 / 2 / 2 | 2.833 / 3.083 | 4.083 / 4.750 | 10.167 / 11.584 | 60.458 / 74.500 | 12.375 / 14.042 |
| wide | 522 / 32 / 2 | 19.708 / 25.625 | 30.875 / 37.000 | 715.458 / 846.166 | 1235.542 / 1407.583 | 658.125 / 710.291 |
| deep | 144 / 2 / 128 | 13.708 / 16.125 | 18.250 / 21.292 | 78.458 / 85.291 | 458.708 / 566.500 | 263.042 / 298.541 |

无变化同步走 revision 短路；有变化同步各次调用 children 18/522/144 次，
明确仍是完整模型快照。Ordered helper p95 分别为 0.708/18.667/1.209 µs。
源排除传入视觉根，由 Core 剪枝，不展开所有后代。
反馈阶段包含创建、附着、清理；完整提交包含计划重算、命令构造/执行、投影和稳定化，
不包含此前 Tool activation。应用夹具命令复制完整模型，其成本也包含在提交内。
首批未采集分配；续验另记多父与大源子树的分配次数和 bytes，
handler 内部扫描与初始化阶段仍未拆分计时。

## 最终产物

消费者 `editor-container-delivery-cost` 先初始化同一 Viewer。
`container-cost` 关闭时既不注册容器 policy，也不调用容器 Tool；
启用时注册 policy，并执行一次父域创建与一次换父。
此 feature 仅属于测量消费者，Editor 不新增默认 feature、运行依赖、字体或图片。
两版均使用 release、LTO、单 codegen unit 和 strip，未使用能力由链接器裁剪。

| Native 能力 | 原始 bytes | gzip-9 bytes | 新进程 p50/p95（ms） |
|---|---:|---:|---|
| 未使用 | 3,116,608 | 1,477,147 | 36.679 / 54.926 |
| 使用 | 3,431,088 | 1,642,256 | 35.374 / 61.125 |

Native 原始增量 314,480 bytes（约 10.09%）。gzip 只记录归档压缩可比值，
不能代替 Web 传输；新进程 32 次样本包含进程启动、初始化和启用版两次编辑，
不能解释为纯初始化时间或性能提升。原始数据与 SHA-256 见
[Native 产物数据](../evidence/p2-e03-native-capability-cost-2026-10-29.json)。

Wasm 两版使用 Rust 1.99.1 的已安装目标、相同 release 配置及
wasm-bindgen 0.2.129。CLI 原始 Wasm 分别为 3,206,445 / 3,546,914 bytes；
打包后计入运行所需 Wasm 与 JavaScript 绑定。外部字体和图片文件均为 0 bytes，
库内数据已随 Wasm 计入；宿主页面与 GPU 渲染不在此消费者中。

| Web 能力 | 打包 Wasm bytes | JavaScript bytes | 总 bytes | gzip-9 总 bytes | Brotli-11 总 bytes |
|---|---:|---:|---:|---:|---:|
| 未使用 | 3,043,691 | 4,923 | 3,048,614 | 1,025,806 | 740,872 |
| 使用 | 3,365,506 | 4,923 | 3,370,429 | 1,152,398 | 828,058 |

打包后原始增量为 321,815 bytes，gzip 增量 126,592 bytes，
Brotli 增量 87,186 bytes。JavaScript 两版字节与 SHA-256 相同。
压缩值按各资源分别压缩后求和，实际部署传输量取决于服务器编码。

每版另外启动 32 个 Node 新进程，加载生成的 JavaScript 和 Wasm，调用同一个
消费者入口。关闭版返回 8，启用版返回 11，全部通过结果断言；
启用版确实提交创建和换父两次命令。下表排除文件读取与 Node 进程启动，
记录 V8 中的 CPU 阶段，不代替 Chrome 网络、GPU 或 present 证据。

| Web 能力 | JS 导入 p50/p95（ms） | Wasm 编译 p50/p95（ms） | 实例化 p50/p95（ms） | 消费者运行 p50/p95（ms） |
|---|---|---|---|---|
| 未使用 | 1.131 / 2.290 | 3.306 / 3.757 | 21.400 / 23.867 | 0.711 / 0.868 |
| 使用 | 1.186 / 1.849 | 3.823 / 5.155 | 36.274 / 47.459 | 1.819 / 2.886 |

消费者运行包含 Viewer 初始化，启用版还包含两次编辑；没有将纯 Viewer 初始化
单独计时。逐样本数据、资源体积和 SHA-256 见
[Web 产物数据](../evidence/p2-e03-web-capability-cost-2026-10-29.json)。

## 验证与剩余范围

首批容器契约 27 项、独立消费者 31 项通过；续验分别为 29 项和 33 项，
两项性能测试默认忽略，新增 scaling 单独显式执行通过。
editor.contracts 与 extensions 已通过。workspace fmt/check 与严格 Clippy 通过；
最终源码补充后，容器 lib/test 严格 Clippy 与全部容器契约再次通过。
Native/Web 公开架构门禁通过，Runtime 方法数 28/28，Kurbo 单版本 0.13.1；
Core 公开符号快照只增加 `geometry::InverseFailure`。

`cargo test --workspace` 的 Rust 单元和集成测试通过，唯一失败项是 xtask
`repository_documentation_is_valid`；docs 同样被既有长文本报告引用的缺失产物阻塞：
`target/verification/long-text/scroll-resize-shared-20261028/hot-comparison.json`。
未改写该历史引用或补造证据，原错误见 [docs 日志](../evidence/p2-e03-docs-2026-10-29.log)。
以上是首次门禁的快照。续验已将缺失链接改为明确的历史缺失记录，
修正两处文档类型；docs 门禁恢复通过，原始产物仍未找回。
本轮最终 workspace 测试退出码为 0，1653 项通过、21 项忽略；
最终 fmt、严格 Clippy 与 diff check 通过，详见续验复核。
workspace doctest 全部通过，其中 11 项容器反例验证私有字段和凭证构造边界。
最终 workspace 严格 Clippy、fmt 与 diff check 通过。
Web 打包结果另记于产物部分。Native/Web 用户输入、跨显示器、
窗口外 capture 与系统 DnD 不由无窗口测试或 Wasm 编译推断，E03 不关闭。
