# Group 1：性能与更新/渲染的文档到代码一致性

scope: full_file

## 边界与方法

- SKILL_ROOT：`/Users/bytedance/.trae-cn/skills/bits-code-guard`。
- REPO_ROOT：`/Users/bytedance/Documents/code/GitHub/drawjs`。
- PROJECT_ROOT：`engine/wasm_rust/novadraw`，下文相对路径均从此处开始；JSONL 的 `file` 则从 REPO_ROOT 开始。
- WORK_DIR：`verification/evidence/goal-alignment-audit-2026-09-30`。
- 本组执行 bits-code-guard 分组步骤 3.1-3.4；已完整阅读技能、general-workflow、review-dimensions、review-rule。Rust 无额外语言规则。
- 先读 AGENTS.md，再读 CLAUDE.md、项目记忆和指定契约，之后才读实现。仅研究和写证据，不修改源码、不生成单测、不运行构建或耗时 probe。最终汇总、跨组去重和 HTML 由主 agent 执行。
- 检查的是读取时工作区现状，不回退最近 commit。记录时 HEAD 为 `841be423815833d97c19862da7b1b0a8ad05591f`；共享 review_files.md 的基线另有记录，不能把本报告理解为该基线的 diff 审查。
- 不分析平台 IME、Figure 公开 API 分组；连接代码只追踪更新/路由工作量的直接依赖。第三方只读取 `org.eclipse.draw2d`，未使用 Zest。
- 以下是定向契约审查，不是所列目录逐行穷尽审查；缺陷均读取完整相关函数、直接调用方和必要类型。复杂度为静态工作量结论，不冒充实测延迟。

## 确定缺陷

### G1-01 [P1][逻辑错误] 构建期失效树转交 Runtime 后未进入首次 validation

- 位置：`novadraw/src/runtime/runtime.rs:758-793`，核心遗漏在 `updates: UpdateManager::with_namespace(namespace)` 与初始化结束之间。
- 置信度：10/10；条件性功能缺陷。
- 契约：`doc/design/rendering/update-manager.md:35-39,77-101` 要求 validity 与更新队列协同，validation 先于录制；`doc/design/architecture/derived-state-convergence.md:14-35,92-107` 要求发布稳定布局；ADR-018 规定 Runtime 为帧发布入口。Builder 的立即验证是可调用操作，并未声明 Runtime::new 只接受已验证树。
- 实现：Builder 的 `set_layout_manager` (`graph/mod.rs:710-718`) 只提交配置；`replace_layout_manager` (`4248-4262`) 标记节点/祖先 invalid。Runtime 构造器创建空 invalid 队列，`activate_attached_figures` (`runtime.rs:4324-4328`) 仅调用生命周期；`graph/mod.rs:939-948` 的 complete_attachment 不入队。`stabilize` (`runtime.rs:4570-4577`) 只在队列非空时验证，结束只检查队列/dirty connection (`4622-4626`)，不检查这些构建期 invalid 节点。
- 具体触发：Builder 创建可见普通 Rectangle 根 `(0,0,100,100)`、普通 Rectangle 子 `(0,0,10,10)`，在根安装 StackLayout，不手工 validate；交给 Runtime::new，调用 prepare_frame 或 prepare_submission_state。无 Label/Image/TitleBar 等额外刷新使队列非空，首帧直接录制。StackLayout 应将子节点填充为 `100x100` (`layout/stack_layout.rs:58-69`)，实际保留 `10x10`，同时 stable_epoch 已晋升。
- 不是猜测的空值或非法输入：上述均为正常 Builder 输入；生命周期、字体或宿主 resize 恰好触发 revalidate 只能掩盖遗漏，不能代替交接契约。
- 已有测试：`tests/m5_layout_contract.rs:363-401` 的 1024 节点事务显式调用 `runtime.figure(root).revalidate()`；`tests/m10_label_contract.rs:104-133` 从 Runtime::empty 经运行期 setter 构建；`runtime.rs:6294-6325` 的 10k 测试先修改 preferred size。均不覆盖“不加补充 mutation 的 Builder -> Runtime 首帧”。
- 建议：Runtime 接管树时把附着树的初始 validation root 纳入统一更新事务，不能让宿主额外 revalidate 才获得正确布局；无需修改递归 renderer。
- 未验证：未运行复现程序。最小复核即上述两节点 StackLayout 首帧，比较 child bounds 与 validity；同时保留已经显式验证过的 Builder 树路径。

### G1-02 [P1][性能问题] 缓存命中的文本树仍逐节点重复扫描祖先样式

- 位置：`novadraw/src/graph/mod.rs:3595-3600`；同类路径 `3645-3651`。
- 置信度：9/10；特定树形/规模下的确定性多余工作，不声称已测得卡顿毫秒数。
- 契约：`derived-state-convergence.md:125-139` 明确按 subject/revision/generation 去重、不重扫无 dirty 工作；ADR-014 的测量缓存要求只重算受影响输入。D4.5 修复了递归 paint 的祖先扫描，但不等于完整 Runtime 路径也线性。
- 完整调用路径：prepare_submission_state_inner / prepare_frame_inner / record_full_frame_inner -> stabilize (`runtime.rs:4538-4565`) 每次无条件排入 IntrinsicMetrics -> refresh_label_intrinsic_metrics (`1024-1033`) -> graph.refresh_label_intrinsic_layouts (`3585-3611`) 收集所有 Label -> 每个 Label 调用 resolved_style (`3556-3570`)。
- resolved_style 为每次调用分配完整祖先链，再反向逐层合并。Label 的缓存命中检查在 `figure/label.rs:208-228`，发生在祖先扫描及 FontDescriptor 解析之后，不能避免这部分工作。
- 具体触发：已完成首帧且文本/字体不变的 N 层 Label 树，显式全帧录制或局部 repaint 再准备帧；所有 Label 再扫描祖先。祖先访问量为 `sum(depth(label))`，链状为 O(N²)，即使没有一次新 shaping。TextFlow 每次稳定化的 IntrinsicMetrics 和 Presentation 都调用同类刷新 (`runtime.rs:4564,4605-4611`)，`graph/mod.rs:3636-3662` 同样先解析祖先，再在 `figure/text_flow.rs:299-313` 命中缓存。
- 已有测试：`tests/m10_label_contract.rs:136-177` 统计的是 TextEngine::layout 次数，不能观察祖先扫描；r8-perf 的深树是 Rectangle，文本场景是自定义 TextProbeFigure，见 `benchmarks/r8-perf/src/main.rs:98-136,367-405`，均避开真实 Label intrinsic 路径。
- 建议：缓存样式/字体的有效 generation，并由受影响 subject 驱动刷新；或一次父子遍历携带样式计算脏节点。不要修改公共 resolved_style 查询语义，也不要恢复迭代渲染主线。
- 未验证：未测 1k/10k 真实 Label/TextFlow 树时间；建议先计数干净帧、单叶内容变更、祖先字体变化的访问工作量，再决定 release probe。

### G1-03 [P2][性能问题] 独立连接自动路由为每条边复制完整兄弟顺序

- 位置：`novadraw/src/runtime/runtime.rs:1424-1432`。
- 置信度：10/10；确定多余 O(E²) ID 处理，尚无端到端延迟证据，因此保守列 P2。
- 契约：`derived-state-convergence.md:196-205` 按稳定 routing group 计算完整 batch；无 group 的 DirectRouter 不需要全体兄弟信息。
- 实现：`resolve_dirty_connection_routes` (`4509-4535`) 对每条仍 Dirty 的连接调用 resolve_connection_route。每次 inner 都先 `tree.child_order(parent)`；该方法直接 clone 全部 children (`graph/mod.rs:2697-2701`)，再转换/collect。随后 `ConnectionRuntime::route` (`connection/runtime.rs:703-780`) 才查询分组；`routing_group_members` (`958-969`) 对 RoutingGroupScope::None 立即返回单连接，丢弃刚构建的列表。DirectRouter 使用默认 None (`connection/router.rs:186-208`)。
- 具体触发：同一个 parent 下 E 条有效 DirectRouter 连接首次全部 Dirty，正常准备帧。每条只解出自身，因此循环执行 E 次；每次复制至少 E 个兄弟 ID，至少 O(E²) ID 工作量，与路径几何复杂度无关。例如 E=4096 至少复制约 1678 万个兄弟 ID。
- 完整 route 成功/失败、locator preflight、batch commit 分支均已阅读 (`runtime.rs:1419-1588`)，没有复用或提前读取 router scope 的路径。
- 已有测试：`tests/m9_connection_runtime.rs:609-663` 检查自动路由正确性；`tests/p2_c02_shortest_path_contract.rs:231-307` 检查单个 64 边障碍组只建一次 snapshot，且走显式 resolve；这不覆盖大量独立 group 的兄弟列表工作量。
- 建议：无 group router 不构造 routing_order；需 group 时按 parent/domain 在本次路由事务中冻结并复用顺序。维持现有批次原子性。
- 未验证：未跑大图路由 probe；现有 route_calculations/obstacle_snapshot_builds 不统计此 clone 成本。

### G1-04 [P2][性能问题] 已完整录制的帧因 Full 标记被再次完整录制

- 位置：`novadraw/src/runtime/runtime.rs:4698-4718`。
- 置信度：10/10；确定冗余录制，不声称画面错误。
- 契约：`update-manager.md` 的 Paint Recording 允许 Partial 采用全量录制；Full 需要完整场景，不要求为了转换 damage 再录一次。
- 最简触发：Runtime::new(已构建普通矩形树) 的首次 submission，无 update queue。`4700-4701` 因 full_redraw_pending/session_sync_pending 执行 tree.render；同一标记在 `4713-4718` 仍为 true，再次 tree.render 并丢弃第一次结果。纯 request_full_redraw、surface resize 或 session reset 同样可进入该分支。
- 第二条触发：FULL_FRAME_ONLY backend 上局部 repaint，`tree.perform_update` -> `UpdateManager::perform_update_transaction` (`deferred.rs:543-586`) 在 `576-577` 已完整 render_to；之后 capability promotion 再 render。`graph/mod.rs:2576-2591` 和完整 `graph/render_recursive.rs` 证明两者使用相同完整可见树遍历，damage 没有在 recorder 上过滤节点。
- 已有测试：`runtime.rs:5616-5660` 只断言 Partial/Full 模式；`benchmarks/r8-perf/src/main.rs:183-190,208-219` 用 record_full_frame，不经过 submission 的重复录制分支。
- 建议：先确定有效 damage/recording 策略，完整命令只录一次；当前全量录制策略下提升 damage 可复用命令，但仍需保持更新队列消费及通知顺序。
- 未验证：未新增 paint 计数器测试；建议用一个只记录调用次数的 Figure 核验首帧、强制 Full、局部提升为 Full 分别只 paint 一次。

## 现有证据能证明什么

| 入口/证据 | 实际覆盖 | 不能推出 |
| --- | --- | --- |
| `benchmarks/r8-perf/src/main.rs:148-159,183-279,281-318` | 同一个已构建对象上 warmup 后重复 CPU 操作；独立记录 setup；flat 4096、deep 1k/10k render/hit/validate、text recording 1000、viewport 1024 | 不测 Vello lowering/GPU/present；不测真实 Label/TextFlow 稳定化、连续拖拽、路由规模、局部帧总延迟 |
| deep validation (`236-258`) | Builder deepest-leaf preferred-size 失效 + validate_subtree | 不是 Runtime typed-worklist、resource/route/freeform 混合事务基准 |
| R8 基线文档 | 同机器、工具链、release 的命名迁移回归；15% 阈值超出后人工复测规则 | 无跨机器结论；无 Draw2D 对照；非当前 HEAD 的性能证明 |
| D4.5 文档 `:41-73` | 历史同机递归 paint 样式传播与 validation visibility 修复；明确披露 10k hit-test 增加约 0.24ms | 不证明 setup/build、GPU 或 present 收益；不证明所有含文本树线性 |
| `verification/suites.toml:401-412,682-700` | full 为 fmt/dependency/build/clippy/test；core.runtime 为 update/event 应用契约 | 无 r8 release benchmark 命令或性能退化断言 |
| `examples/native/update-app/src/main.rs:208-234` | 1024 节点 prepare_frame 完成、下一次 Idle、非空 commands；输出单次 elapsed_us | 无耗时上限，默认非 release，无基线比较，不是性能门禁 |
| `tests/p2_c02_shortest_path_contract.rs:231-307` | 64 条连线同一组，route_calculations=64、obstacle_snapshot_builds=1 | 不是全体工作量计数，也不是大量单连接 group 的性能 |

历史原始数据已读取：本地 `target/performance/d4.5-before-full.json` 与 `d4.5-after-final.json`，10k render median 为 787099708ns -> 9860083ns，10k validation 为 413111834ns -> 1491792ns，输出计数一致，与 D4.5 文档吻合。这是历史证据复核，未重新运行。R8 baseline 原始 JSON 本次未找到于 target/performance；只读了文档中的记录。

r8-perf 只保存 output 而不 assert 预期 output/阈值 (`main.rs:296-310`)；P95 使用 floor 索引 `(len-1)*95/100` (`320-323`)，5/7 次采样的统计口径不足以表示交互尾延迟 SLA。此处仅列证据限制，不把小样本或未接门禁强行记作代码 bug。

在 `benchmarks/`、`scripts/`、`tools/xtask/`、`verification/suites.toml` 的定向检索中未发现 Draw2D 同场景计时 runner/比值断言，也未发现 r8 基准接入 suite。不能由此宣称整个仓库不存在任何历史实验；本组现有证据不足以证明“性能对等或超过 Draw2D”。

## Full / Partial Retained 的实际代价

1. Runtime：Partial 带 damage 元数据，但 recorder 仍从 contents 递归遍历所有 visible 节点；`render_recursive.rs:92-144,217-264` 不做 damage/effective-clip 的几何剔除。区域很小不代表 CPU paint 更少。
2. Vello：`lib.rs:1165-1180` 新建 Scene、重置状态、用向外取整的 union 外层 clip，逐条 lower 全部 commands。不是 retained command tree。`709-771` 的状态栈还复制 RenderState/clip 列表，深层 clip 成本不在 r8 中。
3. GPU/纹理：`lib.rs:1221-1237` 对全 surface 尺寸 scratch 调 render_to_texture，scene 受 union clip；`1256-1288` 按 regions 复制回 retained；`1291-1301` 整张 retained blit 到 surface 并 present。不能把 regions 面积等同于所有 GPU 工作量，也不能把它称为 platform partial present。
4. 常驻至少两张 RGBA8 surface-size texture (`lib.rs:532-562,618-644`)，仅像素数据约 `8*W*H` 字节，不含 Vello scratch、swapchain、资源和驱动内存。Full 同样走 scratch -> retained -> surface 的复制路径。
5. `repair.rs:165-222` 先合并再在最终 region 数超过 8 时退化 union；离散大批脏区在退化之前仍有成对比较成本，阈值不是算法工作量硬上限。
6. 与契约关系：`update-manager.md` 第 7/8 节允许 Full 重录与 retained raster 策略，所以“没有局部 CPU 录制”本身不是正确性 bug。真正收益与不同 damage 分布、场景命令量、像素尺寸有关，需要分段测量。
7. 当前 backend 单测 `lib.rs:1678-1738` 覆盖背景、fractional rounding 与数组局部拷贝等价；其中 `partial_copy_produces_the_same_pixels_as_full_replacement` 是 u8 数组模拟，不调用 VelloRenderer::submit 或 GPU。`render_for_screenshot` (`1318-1384`) 直接全量画 retained，不覆盖正常 partial scratch/copy 路径。不能把这些测试说成复杂场景的真实 GPU Full/Partial 像素差分。

Draw2D 静态对照：`/Users/bytedance/Documents/code/GitHub/gef-classic/org.eclipse.draw2d/src/org/eclipse/draw2d/DeferredUpdateManager.java:172-215,272-307` 先 validation、再 dirty snapshot 与 repair；`Figure.java:1250-1270,1296-1317` 使用 local style/state，并在 child.paint 前检查与当前 graphics clip 相交。Novadraw 的状态顺序对齐，但 CPU child 剔除策略不同；该事实不是性能倍数结论。

## 增量工作与设计差距（不计入 JSONL）

- **队列是阶段位图，而非文档中的 subject worklist。** `runtime.rs:49-88` 只有 6 个 bool；每次 stabilize 全阶段启动 (`4538-4548`)。图像/Label/TextFlow arena 扫描与 owner border descendant 扫描即使输入没变仍发生 (`graph/mod.rs:3585-3705; runtime.rs:1092-1133`)。缓存能避免部分重算，不能证明 O(dirty) 调度；严重的祖先嵌套后果见 G1-02。
- **连接有 reverse index，但未完全由 committed facts 推送。** `connection/runtime.rs:836-858` 每轮遍历所有连接及记录的依赖做 generation 拉取；`824-834` 的 dirty_connections 也每次扫描 order。`Runtime::has_pending_update` (`3667-3676`) 使用后者。因此“没有 reroute”不等于“没有依赖检查”。
- **validation 能跳过 valid 子树，但入口仍是向上失效。** `graph/mod.rs:4616-4650` 无论祖先已 invalid 与否均继续走到根；`1915-1969` 排空队列、重复标记路径、找最高 invalid ancestor，然后验证；`2023-2043` 跳过 valid/invisible。重复 invalidate 的结果幂等，不代表调用开销 O(1)。预算按入队 ID 数计，不等于实际遍历/布局提交数。
- **诊断细节弱于 SSOT。** stabilize 的预算仅每阶段 16 次计数，返回裸 DidNotConverge (`runtime.rs:49-50,4538-4555`)；不是文档中的 subject、source_revision、observed_generation、state-changing commit 计数诊断。保留 full_redraw/retry 路径存在 (`4629-4638`)，本组未证明某个合法反馈场景必然丢工作，故不另报 bug。
- **资源因果主要链路吻合。** `runtime/resource.rs:240-310,325-356` 顺序 Delta、恢复旧前缀、Ready snapshot；`runtime.rs:4732-4758,4773-4806` freeze/ack/retry；`render/submission.rs:252-279` session gate；Vello `lib.rs:430-452,1485-1553` 每个 Snapshot 清 cache、顺序应用 ops。未将 snapshot 空集当 no-op；未发现足够证据另报资源正确性缺陷。该结论不覆盖异步 loader、多 consumer 或 GPU 内存预算。
- **热路径日志与规则不符。** Vello submit/render_command 存在 `log::debug!` (`lib.rs:709-771,1149-1153,1182`)，而规则禁止渲染热路径日志。仅能证明 debug 日志开启时会逐命令发日志；未核验部署过滤器或测量开销，单列规则风险，不按性能故障定级。

## 阅读清单

完整文档：

- AGENTS.md、CLAUDE.md；用户 profile 与当前项目 project_memory.md。
- 指定的 update-manager、derived-state-convergence、resource-lifecycle。
- R8 baseline、ADR014 D4.5 performance；ADR-014/018/020/023 全文。
- bits-code-guard/SKILL.md、general-workflow、review-dimensions、review-rule。
- WORK_DIR/review_files.md、review_groups.md；verification/suites.toml 全文。

源码全文或完整目标函数（非整目录穷尽）：

- `benchmarks/r8-perf/src/main.rs` 全文。
- `novadraw/src/graph/render_recursive.rs` 全文；`graph/mod.rs` 的 Builder/layout 安装、Runtime attachment、invalidation/validation、render、样式解析与 Label/TextFlow/Image 刷新函数；`graph/search.rs` 的 descendant 与 hit-test 完整调用链。
- `novadraw/src/runtime/runtime.rs`：构造器、derived work 类型、各 refresh、resolve_connection_route_inner、自动路由、stabilize、全部 frame prepare/complete/record 入口、activation。
- `runtime/update/deferred.rs`：状态/队列、perform_update_transaction、perform_validation_phase、perform_update 完整函数；`runtime/update/repair.rs` 全文。
- `runtime/resource.rs` 的 complete/fail/remove/delta/snapshot；`render/submission.rs` 的 session gate/damage/submission；`render/traits.rs` 全文。
- `novadraw-backend-vello/src/lib.rs` 的 resource sync、resize/recovery、texture 创建、effective damage、render_command 全文、submit 全文、screenshot recording、cache sync 和相关单测。
- `connection/runtime.rs` 的 route/batch commit/reject、dirty/dependency 扫描、routing_group_members；`connection/router.rs` 的 scope 协议和 DirectRouter。
- `figure/label.rs` 的 intrinsic/presentation；`figure/text_flow.rs` 的 refresh_layout；`layout/stack_layout.rs` 全文。
- `tests/m5_layout_contract.rs` 的 1024 更新与 nonconverging 完整测试；`tests/m10_label_contract.rs` 的首帧/缓存完整测试；`tests/p2_c02_shortest_path_contract.rs` 的 64 边 snapshot 完整测试；Runtime 的 capability promotion 与 10k 深度完整测试。
- `graph/update_integration_test.rs`、`tests/m9_connection_runtime.rs` 做测试入口定位，未逐项读完；不能据测试名声称已经验证。
- `examples/native/update-app/src/main.rs` 的 stress_1024、submission_lifecycle 完整函数；`examples/scenes/src/{update,layout}.rs` 的预验证调用定位。
- Draw2D DeferredUpdateManager 和 Figure 的前述完整相关函数；本地 D4.5 before/after JSON。

仅检索：`scripts/`、`tools/xtask/` 的 benchmark/性能入口，Cargo workspace 成员。未分析平台实现或其他组未提交改动。

## 推荐最小验证入口

本组没有运行以下入口；主 agent 应先评估构建缓存与耗时，再选择执行，勿直接跑 workspace full。

1. 首先按 G1-01 的两节点 StackLayout 输入做短 probe，分别检查 prepare_frame 与 prepare_submission_state；这是现成测试未覆盖的关键路径，不必先做 10k 实验。
2. 已有更新正确性回归：suite `core.runtime`（含 update/event 应用）；更窄为 `cargo test -p novadraw --test m5_layout_contract update_manager_completes_a_1024_figure_layout_transaction`。此测试通过不能消除 G1-01。
3. 已有文本缓存入口：`cargo test -p novadraw --test m10_label_contract label_cache_reshapes_only_when_measurement_inputs_change`。它只覆盖 shaping 次数，不能消除 G1-02。
4. 已有路由工作量入口：`cargo test -p novadraw --test p2_c02_shortest_path_contract one_obstacle_snapshot_is_reused_for_a_sixty_four_connection_batch`；完整相关 suite 为 `core.p2-c02-shortest-path-routing`。需额外独立 group 计数才能覆盖 G1-03。
5. 已有 promotion 入口：`cargo test -p novadraw --lib backend_capability_promotes_partial_damage_to_full`；已有纯拷贝入口：`cargo test -p novadraw-backend-vello --features native --lib partial_copy_produces_the_same_pixels_as_full_replacement`。前者不数 paint，后者不测 GPU。
6. 需提前报告并获主 agent 调度的耗时项：release r8-perf；1k/10k Label/独立连接规模 probe；Native/Web 的实际 Full/Partial GPU 像素差分与计时。r8 新结果应写独立 report，不覆盖历史基线。

本次证据文件仅做 JSON 结构/路径/行号校验与 `git diff --check`，不把静态阅读表述成测试通过。
