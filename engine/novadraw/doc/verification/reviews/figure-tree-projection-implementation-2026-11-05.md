# FigureTree 投影分离实施记录

类型：`verification`

能力归属：Core FigureTree／Runtime 的派生回滚、活动源状态交接、能力成员工作集
与通用 Layout 输入证明、依赖声明及属性局部重验。
本片不新增 Editor、backend 或 platform 能力。

日期：2026-11-05。状态：活动交接复制删减已实现，定向正确性通过；
成员索引已实现并通过两个身份与失败反例；首片离屏交互未证明稳定净收益，
通用布局输入已接入并通过公开接口合同及独立消费者验证；
静态依赖声明已原子准入，guard、局部队列与内部输出已接入真实事件；
ScrollBar／ScrollPane／Fill／Stack 与外部自有容器共用接口，
晚期冲突恢复、构造期 session 和隐藏布局已通过定向验证。
生产消费者成本及通用 FigureTree 验收尚未完成。
用户已授权按[总体方案](../../design/architecture/figure-tree-projection-separation.md)
开始实施，仍归入唯一 FR-02 Execute。

最新切片处理输入检查的样式与成员构造；完整读取时冻结成员集合的修正已通过
正确性验证，匹配成本采样已完成。相邻中位数多项下降，但部分尾部仍上升，
净成本及通用投影门禁未关闭。后文按冻结版本保留各轮证据。
本地采样根为 `target/verification/figure-projection-2026-11-05/`。
本次样式直接读取、按需成员及后续修正的 Perfetto 分析使用
`tools/scripts/textflow_scroll_perfetto.sql`；SQL 已冻结为采样根下的
`projection-stage-queries.sql`，hash 在 `projection-stage-query-receipt.json`。
每个 analysis 目录保存逐条查询结果，trace receipts 保存可重跑命令和 trace hash。

`api_semantics`：`figure.tree`、`validation.protocol`、`frame.preparation`、
`notification.ancestor`、`notification.layout_update`。

## 改动及失败合同

原活动交接在回调前后各调用一次完整派生快照构造。
这会复制全树的 preparation、布局输入证明及既有通知，
但 source rebase 并不使用这些字段。
其中 `FigurePresentation` 的命中片段是 `Vec`，复制成本随片段数增长。

现在用独立的 `PreparationSourceCheckpoint` 保存比较输入：
节点几何、布局版本、连接路由与 decoration、viewport 状态及通知起始位置。
回调后直接比较当前节点，将实际变化合并到原派生回滚前缀，
只追加本次活动产生的通知。既有节点不复制 prepared 资产或输入证明；
新增节点仍保存完整前缀，退休身份不恢复。

原回滚前缀的完整保存与失败恢复保持。
活动已提交的源移动不会因后续 preparation 失败丢失；
失败的派生 geometry、prepared identity 与输入证明仍恢复到原前缀。
这里没有给任意 Figure、布局或动态 reader 增加免检查资格。
坐标查询继续沿当前父链组合；必需的后代通知仍执行。

## 已通过的验证

新增两个内部反例覆盖源移动、未提交 child resize、原 prepared identity 与证明恢复、
当前父链坐标、源 size／visibility／layout generation、新增节点与身份退休。
四组独立消费者共 55 项通过：
`p2_scene_preparation_contract`、`p2_preparation_measurement_contract`、
`graph_projection_contract`、`p2_prepared_scene_routing_contract`。
其中包括活动源写入之后失败、viewport feedback 回滚、
连接路由与外部 decoration 恢复，以及 preparation 原始错误。
Core `--lib` Clippy `-D warnings` 通过。

两个内部反例的第一次运行中，退休断言失败：
测试误用只卸载的 `remove_child`，节点身份仍存活。
已改为受检 `disposal_ids`／`extract_subtree` 路径，重新运行通过；
没有改变卸载或退休的实现语义。

## 无 tracing 阶段对照

冻结前后 release 测试可执行文件，五轮交替，每侧每轮 200 个计时样本，
每轮先预热 20 次。树含一个父节点和 300 个子节点，其中 100 个保有
synthetic prepared 资产。比较活动捕获、父移动及 source rebase 的合计时间，
通知仍按原规则生成；不包含 paint、backend 或 GPU。

表中数字为五轮各自分位数的中位数，单位为微秒。

| 每份 prepared 命中片段 | 分位数 | 修改前 | 修改后 | 变化 |
|---|---|---:|---:|---:|
| 0 | P50 | 124.208 | 51.958 | -58.17% |
| 0 | P95 | 201.792 | 68.750 | -65.93% |
| 0 | P99 | 494.917 | 114.917 | -76.78% |
| 1024 | P50 | 288.708 | 50.958 | -82.35% |
| 1024 | P95 | 359.292 | 68.334 | -80.98% |
| 1024 | P99 | 1017.875 | 123.167 | -87.90% |

这个对照只证明交接阶段减少了复制与整理工作。
synthetic 资产不是 shape／image／text 完整绘制场景，不能据此关闭整树门禁，
也不能把这些比例写成整帧渲染提速。

本地产物位于 `target/verification/figure-projection-2026-11-05/`：
`before-test`／`after-test` 为冻结二进制；
`before-derived.rs`／`after-derived.rs` 为对应源码；
`probe-receipts.json` 保存 SHA-256 与逐轮结果；
`unit.log` 和 `verify-*.log` 保存检查输出。
阶段探针可直接运行冻结文件，参数为
`projection_source_handoff_cost --ignored --nocapture`。

## 交互对照与剩余工作

当前依赖锁定文件与既有 step2b 应用基线不同，旧应用不能用于本片独立归因。
已将相同源码、依赖和 `activity-source-capture`／`activity-source-rebase`
插桩冻结到本地产物的 `frozen-source/`。
应用前后仅替换 `graph/derived.rs`；构建与收据由
`frozen-source-receipt.json` 追溯。
首轮构建因冻结目录遗漏并行新增样例的入口文件，在编译前停止；
已补齐相同入口并记录在源码收据。另一轮应用二进制 SHA-256 相同：
`copy2` 保留源码旧时间戳，Cargo 未重建修改后的源码。
该轮 `interaction-reuse-on/` 只作同产物波动记录，已在
`rejected-sampling.json` 中拒绝用于修改归因。比较脚本现在提前检查
相同配置下的二进制哈希，遇到同产物即拒绝启动。

刷新变体源码时间戳后，前后哈希分别为
`45e01340dfbd20b8c4c266e28a19974ce044913f8816e9064e77c666d0e3d2e5`、
`0690aece329882580f10aedbd5279055652b57ac473cdfaf4f6c35233be45ed5`。
三轮交替离屏 GPU 回放，每轮两个 cycle，关闭 tracing、开启自然 damage、
assembly reuse 开启，纹理缓存与 GPU wait 关闭。753 个操作的文本摘要和
viewport 位移一致；这项检查没有证明像素等价。
数据见 `interaction-reuse-on-rebuilt/` 的收据、几何检查与 `comparison.json`。

下表单位为毫秒，数字为合并有效样本的“修改前 → 修改后”。
active 是主线程操作、输入分发与帧处理的合计耗时；frame 是帧处理耗时，
latency 还包含请求到帧之间的等待。本次没有测量 GPU 完成或屏幕扫描延迟。

| 操作 | 指标 | P50 | P95 | P99 |
|---|---|---:|---:|---:|
| 拖拽 | active | 26.361 → 26.076 | 31.040 → 35.166 | 37.238 → 47.725 |
| 拖拽 | frame | 16.654 → 16.957 | 20.367 → 25.186 | 25.938 → 36.148 |
| 拖拽 | latency | 26.547 → 26.321 | 31.751 → 36.597 | 37.326 → 67.249 |
| 滚轮 | active | 26.387 → 26.720 | 31.906 → 34.046 | 43.738 → 39.672 |
| 滚轮 | frame | 16.748 → 17.520 | 20.609 → 22.459 | 31.532 → 29.430 |
| 滚轮 | latency | 26.576 → 26.931 | 32.435 → 36.340 | 46.538 → 59.725 |
| resize | active | 52.525 → 51.554 | 60.374 → 65.656 | 73.046 → 87.602 |
| resize | frame | 17.589 → 18.248 | 21.197 → 24.492 | 27.612 → 41.310 |
| resize | latency | 54.463 → 53.626 | 63.095 → 68.011 | 77.364 → 90.754 |
| 追加 | active | 36.182 → 34.110 | 40.326 → 41.545 | 40.326 → 41.545 |
| Stable | active | 11.328 → 10.979 | 18.438 → 14.872 | 18.438 → 14.872 |

首片没有证明稳定的整帧净收益。拖拽 active P50 下降 1.08%，
但 P95 上升 13.29%，frame P95 上升 23.66%；滚轮 active P50 上升 1.26%。
resize active P50 下降 1.85%，P95 上升 8.75%。
尾部回退尚未归因，不能据阶段微基准排除实现或运行环境影响。
追加与 Stable 的样本较少，其 P95／P99 不作为尾部稳定性的充分证据。
下文补齐默认 assembly 路径、独立 Perfetto 与成员工作集版本对照。

## 能力成员工作集

FigureTree 按 descriptor 的 `TypeId` 维护全部已准入 capability 的节点成员，
成员集合只保存 FigureId，由树持有，不保存 callback 结果或输入证明。
活动与 clickable 查询改从对应成员读取，仍执行真实 descriptor 解析；
检查是否存在 preparation 时也使用同一索引。
实际活动仍检查当前挂载、可见性、启用状态并保留原节点调用顺序。

预留节点加入索引，准入失败或真正退休时统一移除所有成员；
卸载节点仍存活，因此保留登记，重新挂载无需注册第二份能力。
两个新反例核对外部 descriptor、卸载与重新挂载、退休后槽位复用，
以及预留校验返回错误或 panic 后的索引与原扫描一致，均已通过。
这里没有增加内置节点特权，也没有替换外部布局的 invalidate 回调。

Layout 后活动成员为空且 pending mutation 为空时，
Runtime 不构造活动源比较快照；存在任一活动 descriptor 或 mutation 时仍保存。
不从“不可见”“暂时无需唤醒”或“上次未产生 effect”推断这次不会写入源状态。
Core `--lib` 487 项通过，六个手动成本探针保持 ignored；
九组独立消费者共 126 项通过，覆盖 ScrollBar／外部 activity 与 panic、
外部 capability、preparation 回滚与测量、图投影与路由、Freeform、坐标与 viewport。
Core lib Clippy `-D warnings` 与 Wasm `--lib` check 通过。
输出位于 `workset-verify-0.log` 至 `workset-verify-3.log`。
文档门禁通过 508 份 Markdown；交互成本与 Perfetto 结果见下文。
Core fmt check 通过；workspace fmt check 在并行新增 RichText 示例的
`interaction.rs`／`main.rs` 上发现格式差异，未宣称全 workspace 格式门禁通过。

本片仍在活动前扫描 source 几何，初始派生回滚也仍保存全树。
能力成员索引只缩小消费者发现范围，通用布局失效依赖与剩余工作集、
局部绘制表达、可见录制及 backend 复用未完成。
不得把本次复制下降解释为任意拖动已经进入完整纯投影路径。

### 成员查询成本

五轮关闭 tracing 的 release 探针，每轮每侧 200 个样本，先预热 20 次，
每个样本执行 100 次查询。两侧在同一冻结测试程序中交替运行：
原全树扫描与新成员索引都解析实际 activity descriptor、生成结果 Vec，
并确认恰有一个外部活动消费者。其余节点没有 activity。
表中为五轮各自分位数的中位数，单位为微秒／查询。

| 无活动节点数 | 查询方式 | P50 | P95 | P99 |
|---|---|---:|---:|---:|
| 300 | 全树扫描 | 3.476 | 3.788 | 4.083 |
| 300 | 成员索引 | 0.087 | 0.092 | 0.115 |
| 10000 | 全树扫描 | 160.632 | 231.898 | 447.918 |
| 10000 | 成员索引 | 0.090 | 0.100 | 0.175 |

300 节点时 P50 下降 97.51%，10000 节点时下降 99.94%。
新增无活动节点没有使索引查询出现原扫描的线性增长。
这只验证消费者发现，不包含 capability 注册／退休、树修改、布局、
坐标通知、paint 或 backend 成本，也不证明增加屏幕外内容后的整帧成本。
首轮扫描存在较大的尾部波动，全部五轮原始样本摘要均保留；
没有删除该轮或用 traced 时间替代净成本。
冻结程序及哈希见 `workset-receipt.json`，
原始输出为 `workset-query-0.log` 至 `workset-query-4.log`。

### 下一片的实际依赖边界

本次定向审阅确认，`mark_validation_path_invalid_for` 仍无条件使父链失效；
`perform_validation_cycle_measured` 从旧 FigureId 队列取出项后再次标记父链。
`revalidate_children_with_update` 仍复制并检查容器的全部直接 children。
这些位置是通用失效工作集的后续入口，尚未改变其行为。

现有 `PreparationLayoutRead` 已观察几何与 measurement source，
但 `LayoutSceneQuery::children` 同时返回 child 身份和当前 bounds，
不能将这个查询解释为只依赖拓扑。布局器还可以持有外部私有输入，
`LayoutManager::invalidate` 也可以修改其内部缓存。
因此现有读取记录不足以给任意 manager 授予跨回调免检查资格。
下一片需要区分拓扑、位置、尺寸、测量和私有输入的完整依赖；
同时处理即时失效、排队后重验、内部布局输出反馈与失败恢复。
未知 manager 保留原传播，Freeform 包络与连接依赖继续按当前契约更新。
属性级候选接口仍未接受，不以这次查询优化替代该契约的复核。

## 补齐的交互对照

三组比较各运行三轮，每轮两 cycle，交替启动冻结的 before／after；
每组各匹配 753 个操作的文本摘要与 viewport 位移。
拖拽、滚轮、resize 每侧各 240 个样本，追加每侧 6 个，Stable 每侧 9 个。
全部关闭 tracing，使用离屏 GPU、自然 damage，关闭纹理缓存和 GPU wait。
这里的默认路径指 assembly reuse 关闭，不表示已验证 Native 呈现。

第一组比较原交接与删减后交接，assembly reuse 关闭；
第二、三组比较删减后交接与增加成员索引后的版本，
分别开启、关闭 assembly reuse。三组是不同时间段的独立配对，
不能跨表相减推导从原版本到最终版本的总收益。
完整 active／frame／latency P50、P95、P99 与逐轮样本分别保存在
`interaction-default/`、`workset-interaction-reuse/`、
`workset-interaction-default/` 的 `comparison.json` 与 `receipts.json`；
汇总入口为 `cost-summary.json`。

下表为 active 毫秒，单元格表示“修改前 → 修改后”。

| 比较 | 操作 | P50 | P95 | P99 |
|---|---|---:|---:|---:|
| 交接删减，reuse 关闭 | 拖拽 | 20.226 → 18.775 | 25.487 → 22.363 | 30.569 → 25.059 |
| 交接删减，reuse 关闭 | 滚轮 | 19.440 → 18.487 | 22.934 → 22.795 | 26.409 → 25.390 |
| 交接删减，reuse 关闭 | resize | 43.431 → 42.758 | 53.258 → 51.427 | 60.556 → 58.816 |
| 增加成员索引，reuse 开启 | 拖拽 | 25.783 → 25.746 | 32.219 → 29.938 | 46.670 → 32.507 |
| 增加成员索引，reuse 开启 | 滚轮 | 25.556 → 26.013 | 31.729 → 35.078 | 38.618 → 50.245 |
| 增加成员索引，reuse 开启 | resize | 49.977 → 50.005 | 58.982 → 61.485 | 68.319 → 69.720 |
| 增加成员索引，reuse 关闭 | 拖拽 | 20.580 → 21.361 | 31.359 → 27.330 | 46.258 → 37.353 |
| 增加成员索引，reuse 关闭 | 滚轮 | 20.357 → 20.357 | 25.362 → 23.549 | 27.039 → 26.044 |
| 增加成员索引，reuse 关闭 | resize | 43.870 → 43.897 | 50.204 → 51.979 | 55.695 → 56.675 |

交接删减的默认路径拖拽 active P50 下降 7.17%，滚轮下降 4.91%；
三轮各自的拖拽和滚轮中位数也都下降。resize active P50 下降 1.55%，
但 frame P95 从 17.263 升到 18.771，P99 从 20.240 升到 25.353。
滚轮 latency P99 从 31.742 升到 33.876；追加 active P50 上升 2.52%。
这些结果与前述 reuse 开启时的尾部回退一起保留，
不能将本组拖拽改善写成所有交互均获净收益。

成员索引的 reuse 开启路径拖拽 active P50 下降 0.14%，
滚轮 P50 上升 1.79%，P95 上升 10.56%，P99 上升 30.11%。
reuse 关闭路径拖拽 P50 上升 3.79%，三轮中位数分别为
20.615 → 21.592、20.465 → 21.384、20.616 → 21.284；
不能仅凭合并 P95 下降把这个中位数回退归为已排除的环境波动。
两个配置下 resize P50 都约上升 0.06%，P95 分别上升 4.24% 和 3.53%。
因此成员查询的独立成本收益成立，整帧净收益仍未成立。
尾部与中位数回退需要后续归因；本片保持未关闭。

## Perfetto 阶段归因

分别采集原版本、交接删减版、成员索引版各一份 trace，
三份都开启 assembly reuse。它们与关闭 tracing 的比较分开运行，
不能用 traced 时间抵消净成本回退。
官方 Trace Processor 通过命名 warm session 执行
`tools/scripts/textflow_scroll_perfetto.sql`，解析一次后复用；
分析结束已清理三个 session。
三份 trace 的 stats 均没有 `error` 或 `data_loss`，
未闭合 scope 与负 exclusive duration 均为零。
原始 SQL、schema 与逐查询输出由 `analysis-before/`、
`analysis-after/`、`analysis-workset/` 保留。
`collection-receipts.json` 保存二进制和 trace SHA-256、执行命令；
trace 本身位于 `trace-before/`、`trace-after/`、`trace-workset/`。
三个 trace 的 250 个操作摘要与位移逐项比较，两个变体共 500 对均一致；
该比较仍只检查文本摘要与位移，没有像素等价结论。

下表为每操作 capture＋rebase 的平均毫秒；这两个 scope 不嵌套，
可以相加。每份 trace 每项各 80 个操作，resize 每操作有三次活动交接。

| 操作 | 原交接 | 交接删减 | 再增加成员索引 |
|---|---:|---:|---:|
| 拖拽 | 0.852 | 0.122 | 0.144 |
| 滚轮 | 0.824 | 0.121 | 0.137 |
| resize | 2.540 | 0.406 | 0.452 |

交接 scope 的下降与无 tracing 阶段探针方向一致，
但索引没有减少这个场景的活动交接次数，因为 scrollbar 仍是实际活动消费者。
三个版本的拖拽／滚轮每操作均有两次 stabilization、两次 Flow measurement
cache hit、29 次 `flow-qualification`、23 次 input checkpoint、
27 次 `preparation-input-validate`；索引没有删除这些输入消费。
resize 的 Flow build 共 1036 次、qualification 共 6776 次，
三个版本的次数也一致；不能将 Flow build 总次数解释成相同次数的真实 shaping。

成员索引版拖拽的 Flow qualification exclusive 平均为 5.121 ms，
input validate exclusive 为 1.403 ms，IR recording exclusive 为 0.909 ms。
每帧仍提交 7609 个 command，glyph 中位数为 51232.5，
三个版本相同，本片没有减少这个场景录制的 command 与 glyph 数量。
backend encode 平均 1.091 ms，Vello render scope 平均 10.808 ms；
后者只表示宿主 API scope，未等待 GPU 完成，不能作为 GPU 执行时间。

这些实测结果支持继续推进真实布局依赖工作集、检查有效区间和可见录制；
初始完整回滚、全树 style 构造与 backend 输出仍有成本。
当前采样没有通用混合 Figure 像素对照、OS 连续输入、GPU 完成等待、
设备恢复或完整空间索引验收，不能关闭总体方案。

## 下一布局切片的合同复核

本次只读核对 `Figure.setBounds/revalidate`、`ScrollBar.revalidate` 和
`Viewport.localRevalidate`。Draw2D 区分平移与 resize，也提供局部重验入口；
这些事实不能证明 Novadraw 任意外部布局与 child 内部属性独立。
现行 bounds 更新也已在固定尺寸平移时避开普通 resize 失效，
因此下一片不批量替换几何入口。

Rust 定向核对确认三个传播点需一起处理：`mark_validation_path_invalid_for` 的
即时父链标记、`perform_validation_cycle_measured` 的出队再次标记、
`apply_layout_output` 提交 child resize 后的普通失效。
同时，`children` 返回完整 bounds，`LayoutState::invalidate` 调用 manager
私有失效逻辑；当前读取记录不能作为自动隔离依赖的完整依据。

`ScrollBarLayout::layout` 在可替换 thumb 测量之后直接读取共享 RangeModel。
请求隔离 guard 不包含目标 value，不能替代这次完整布局输入的消费与发布证明。
已在[候选合同](../../design/architecture/property-scoped-layout-revalidation.md#定向复核布局私有输入必须进入证明链)
补充通用 `LayoutSceneQuery::input`、完整首次值、无 preparation Figure 的
受检 session，以及三个步骤的执行与验收边界。
这组新接口仍待具体确认，没有在本次代码中实现，也没有新的性能结论。

## 通用布局输入切片

上述合同补齐后，用户要求“提交最近修改，然后继续通用投影分离”。
已提交 `d6c417f4` 与 `4408a847`，随后按输入查询、局部队列和消费者的顺序继续。
本节记录第一个步骤；没有提前缩小普通失效范围。

`LayoutSceneQuery::input` 接入既有 `PreparationScene::input`，返回完整 owned
snapshot。输入 proof 仍保留 reader 注册弱引用与首次值，消费和最终发布继续重读
实际输入。`LayoutError::Input` 保留原始 reader、注册和预算错误；
兼容的无会话查询返回 `InputSessionUnavailable`，不会执行无预算 reader。

Runtime 不再因没有 preparation capability 而关闭布局输入会话。
这类普通树使用延迟输入上下文：纯几何布局不构造样式表或 subtree proof log，
第一次实际 input 读取才准入；上下文构造失败也会被锁存。
嵌套 preferred／minimum manager 计算保留同一查询。旧尺寸缓存缺少完整输入 proof，
因此这些受检嵌套计算不采用旧缓存；直接的只读兼容查询仍保持原缓存行为。
这项选择保留证明完整性，也可能增加无 preparation 容器的重复测量成本，
不能仅以零样式构造成本推断整体无回退。

`p2_layout_input_contract` 的 10 项测试及同一组独立消费者测试通过，覆盖普通树、
存在其他 preparation 节点的混合树、preferred／minimum、跨帧模型变更、
被忽略的错误、完整首次值与当前值、child 测量回调冲突、弱注册退役、
reader 预算拒绝和原始错误链。失败候选不改变已发布的 child bounds，重试采用新值。
混合文字场景首次失败是测试未注册字体，补齐 Inter 后通过；没有据此修改文字实现。
该混合测试只验证输入会话选择，不作为 shape／image／text 完整像素验收。

新增内部测试确认纯几何布局在零预算下仍不构造输入上下文，
以及会话准入失败时 reader 不执行、重复读取保留第一次错误。
Core 回归本轮通过 489 项、6 项 ignored，后新增的准入失败测试定向通过。
lib 与公开合同测试 Clippy `-D warnings` 通过，独立 Native 消费者和实际调用新接口的
Wasm lib check 通过，文档门禁通过 510 份 Markdown。
Core 测试输出中的既有 dead-code 与未使用 acknowledgement 警告仍存在；
本节不把标准测试通过写成测试目标零警告。

匹配性能与 Perfetto 采样使用
`target/verification/figure-projection-2026-11-05/build_input.py`。
本片沿用 `workset-app` 的冻结源和依赖，只替换 layout 查询与测量实现，
不混入同期独立富文本文档改动。首次构建因发现下述成本回退而停止，
尚未产出新的 release 对照或 Perfetto trace；修正后重新冻结和采样。
当前正确性结果不代表通用投影分离或生产性能验收完成。

### 普通嵌套测量成本回退

`ordinary_layout_input_query_cost` 对同一棵无 preparation 的树交替调用旧兼容查询
与 Runtime 受检查询。每个 child 容器使用普通 FillLayout，内含一个 RectangleFigure；
上层布局依次查询每个容器的 preferred 与 minimum。两侧各预热 20 次、采样 200 次，
同一进程中交替先后顺序。此轮使用 debug 测试可执行文件，没有 tracing、绘制或 GPU。
下表仅记录首轮阶段探针，单位为微秒，不能作为 release 整帧增幅。

| child 容器数 | 查询路径 | P50 | P95 | P99 |
|---|---|---:|---:|---:|
| 1 | 旧查询，尺寸缓存已预热 | 1.416 | 1.459 | 1.542 |
| 1 | 受检查询，不采用旧缓存 | 4.583 | 4.667 | 4.750 |
| 100 | 旧查询，尺寸缓存已预热 | 110.458 | 124.750 | 173.583 |
| 100 | 受检查询，不采用旧缓存 | 380.750 | 425.000 | 454.917 |

证据位于 `target/verification/figure-projection-2026-11-05/input-ordinary-debug-0.log`。
代码中受检嵌套计算明确避开旧尺寸缓存，这个分支与阶段回退一致。
该回退仍属于同一输入切片的整改，不新增并行 Execute。

### 普通测量缓存的保守复用

定向反例发现，仅给尺寸缓存附上输入 proof 仍不足以跳过未知测量回调：
child 的 minimum callback 可能修改本轮已被其他布局读取的模型。
缓存 proof 即使完整且当前相等，也不能证明尚未执行的 callback 没有副作用。
因此没有采用“重验 proof 后直接返回缓存”的实现。

现在为通过 Runtime 查询计算且没有准入私有输入的 preferred／minimum 结果
保留独立受检资格。只在本轮尚未构造 input scene 时采用这份结果；
旧兼容查询生成的尺寸缓存
不获得这份资格。首次 private input 准入以后，后续嵌套测量继续执行真实 callback，
不采用或写入这份缓存，完整输入仍在消费及发布处检查。
这保留普通纯几何测量的既有缓存行为，没有授予已读取私有输入的会话免检查资格。
缓存归 FigureTree 的按需测量表，generation 仍由 LayoutState 持有。
普通失效、manager 替换、节点退休和派生失败恢复均清理对应记录，
不把失败候选的缓存恢复为有效数据。

修正后相同 debug 探针的一轮结果如下，单位为微秒。
两个时间段的绝对值受运行环境影响；本表的同进程配对才是可比较分母。

| child 容器数 | 查询路径 | P50 | P95 | P99 |
|---|---|---:|---:|---:|
| 1 | 旧查询，尺寸缓存已预热 | 2.084 | 2.208 | 2.250 |
| 1 | 受检查询，输入未准入 | 2.667 | 2.791 | 2.834 |
| 100 | 旧查询，尺寸缓存已预热 | 116.000 | 139.250 | 154.375 |
| 100 | 受检查询，输入未准入 | 117.542 | 140.833 | 157.542 |

100 个容器的 P50 配对差异从约 244.6% 降至约 1.3%。
单个容器仍有约 0.583 微秒的固定查询成本；本结果没有证明 release 整帧净收益。
10 项输入合同重新通过，包括已预热 child cache 后 callback 改写模型的失败回滚。
release 对照与独立 Perfetto 结果待下文补齐，局部队列尚未接入。

第一版将受检测量结果与兼容测量缓存分开存储，增加每个 LayoutState 的常驻记录。
匹配三轮 release 对照中，默认 assembly 路径的拖拽 active P50 基本持平，
但 reuse 开启时从 22.814 升至 23.782 ms，增加约 4.25%；
滚轮从 22.732 升至 23.164 ms，增加约 1.90%。
这些数据保留在 `input-interaction-default/` 与 `input-interaction-reuse/`，
二进制及源码收据保留为 `input-duplicate-cache-app` 与
`input-duplicate-cache-receipt.json`，不作为该切片的生产净收益证据。

独立 Perfetto 两份 trace 的健康检查通过：没有 error／data_loss、
未闭合 scope 或负 exclusive duration。拖拽与滚轮每侧各 80 个操作，
每操作仍有 29 次 qualification、27 次 input validate 和两次 Flow build；
resize 的 qualification 共 6776 次、input validate 共 5740 次、
Flow build 共 1036 次。输入接口没有删减这些真实检查。
trace 收据为 `input-trace-receipts.json`，分析在
`input-analysis-workset/` 与 `input-analysis-input/`。
这些 traced 成本不用于抵消关闭 tracing 后的回退。

随后将资格放进原测量缓存，减少重复记录，正确性反例重新通过。
第二组三轮采样的 reuse 拖拽 P50 从 22.654 升到 23.066 ms，
滚轮从 22.782 升到 23.194 ms，仍分别回退约 1.82%／1.81%。
该组记录在 `input-compact-interaction-default/` 与
`input-compact-interaction-reuse/`，产物及收据保留为
`input-compact-cache-app` 与 `input-compact-cache-receipt.json`。
不能据回退变小将重复存储认定为已证明的全部根因。

当前将普通受检测量缓存移到 FigureTree 的按需表：只有无 preparation 的
测量会话实际查询 manager 尺寸时分配记录，混合文字树不为每个 Figure 增加尺寸记录
或资格字段。表准入按本轮预算计费；清理覆盖父链失效、退休和派生回滚，
尺寸 generation／constraints 匹配保持。10 项公开输入合同、缓存准入反例、
Core lib Clippy 及独立 Native／Wasm 消费者重新通过。
普通 HashMap 表版的三轮数据在 `input-table-interaction-*`：
reuse 拖拽 P50 为 23.294 → 23.038 ms，下降约 1.10%；
滚轮为 23.271 → 23.320 ms，上升约 0.21%，resize 为 46.456 → 45.813 ms。
默认路径拖拽为 17.421 → 17.538 ms，resize 为 37.577 → 37.972 ms，
仍有小幅回退。这说明恢复节点存储大小后没有再次出现先前的大幅拖拽回退，
但不能据这一组中位数认定所有新增成本均已消除。

这张表的 debug 阶段探针仍有成本：100 个容器的旧查询 P50 为 104.625 微秒，
完整 FigureId 哈希查表版为 154.375 微秒。当前改用
`SparseSecondaryMap` 的版本化本地键，先验证完整 Runtime 身份再查询，
退休节点仍清理记录；没有用槽位索引代替完整身份准入。
稀疏表只保留实际查询的节点，不为整个 arena 预分配测量记录。
同一 debug 探针的 100 容器结果为 107.417 → 147.041 微秒，
保留这项阶段代价；该差异不等于 release 整帧增幅。
稀疏表版的两种配置各完成三轮匹配采样，各核对 753 个操作的摘要和位移；
每侧拖拽、滚轮、resize 各 240 个样本。下表为 active 毫秒，
单元格表示“修改前 → 修改后”，tracing 关闭、自然 damage、纹理缓存和 GPU wait
关闭，仍是离屏 GPU 场景。

| assembly reuse | 操作 | P50 | P95 | P99 |
|---|---|---:|---:|---:|
| 关闭 | 拖拽 | 17.444 → 17.444 | 18.975 → 21.800 | 23.188 → 25.544 |
| 关闭 | 滚轮 | 17.301 → 17.427 | 18.090 → 19.394 | 18.995 → 22.601 |
| 关闭 | resize | 37.018 → 37.965 | 41.938 → 42.358 | 43.154 → 44.396 |
| 开启 | 拖拽 | 22.482 → 22.647 | 23.556 → 23.889 | 25.151 → 24.505 |
| 开启 | 滚轮 | 22.585 → 22.640 | 23.692 → 24.037 | 26.425 → 24.608 |
| 开启 | resize | 43.959 → 44.570 | 50.916 → 48.333 | 52.625 → 49.100 |

默认 resize P50 上升 2.56%，三轮中位数分别为
37.420 → 37.886、36.938 → 37.442、37.018 → 38.361；
reuse resize P50 上升 1.39%，三轮中位数也全部上升。
默认拖拽 frame P95 为 13.182 → 15.694，滚轮 frame P99 为 13.127 → 14.864。
原先较大的拖拽中位数回退有所收敛，但 resize 仍稳定回退，默认尾部也没有通过。
这不是当前输入切片的生产性能验收结果。

完整 active／frame／latency 分位数、逐轮中位数和匹配记录由
`input-sparse-interaction-default/`、`input-sparse-interaction-reuse/` 及
`input-cost-summary.json` 保存。当前冻结二进制 SHA-256 为
`730d2d1101b06a6e3e957f9852d2513bd07403b60be1ccc664e1c9dffa2e5ed6`，
收据已归档为 `input-sparse-cache-receipt.json`，二进制为
`input-sparse-cache-app`；没有覆盖此前失败版本的二进制及收据。

稀疏表版的独立 trace 健康检查通过。qualification、input validate 和
Flow build 次数仍与 workset 基线一致；resize 的 layout exclusive 平均增加约
0.140 ms，其他阶段也存在小幅变化。这仅定位了成本变化所在阶段，
不证明一次 trace 已解释全部无 tracing 回退。
trace 收据在 `input-sparse-trace-receipts.json`，
分析在 `input-sparse-analysis-workset/` 与 `input-sparse-analysis-input/`。

最新整改给空测量表增加清理前判断，避免在没有普通测量记录的父链失效中
继续查询该表。缓存清理反例及 lib Clippy 通过。该版本的两种配置已分别完成
三轮匹配采样，结果没有支持“空表清理是全部根因”的判断。

| assembly reuse | 操作 | active P50，ms | 变化 |
|---|---|---:|---:|
| 关闭 | 拖拽 | 17.337 → 17.935 | +3.44% |
| 关闭 | 滚轮 | 17.288 → 17.985 | +4.04% |
| 关闭 | resize | 37.210 → 38.756 | +4.15% |
| 开启 | 拖拽 | 22.713 → 22.745 | +0.14% |
| 开启 | 滚轮 | 22.831 → 22.883 | +0.23% |
| 开启 | resize | 45.207 → 45.075 | −0.29% |

默认配置的三个操作逐轮中位数均上升，不能据 reuse 配置接近持平宣布通过。
完整分位数、753 个匹配操作及收据在 `input-empty-interaction-default/`、
`input-empty-interaction-reuse/` 和 `input-cost-summary.json`。
该二进制 SHA-256 为
`8be491e0b77238944b35881bec274a253298205cb9d7d75dfe9843c62e5965b4`，
已归档为 `input-empty-cache-app` 与 `input-empty-cache-receipt.json`。

独立 Perfetto 分析在 `input-empty-analysis-workset/` 与
`input-empty-analysis-input/`，收据在 `input-empty-trace-receipts.json`。
traced resize 的 layout exclusive 平均为 4.952 → 4.927 ms；
拖拽为 1.396 → 1.422 ms。这组 trace 没有复现关闭 tracing 时的默认配置增幅，
不能拿 traced 平均值抵消上述回退，也不能据此确定运行环境是原因。

继续核对布局入口的 ARM64 指令后，确认新增普通树会话与既有 preparation 会话
共用函数栈帧：局部栈空间从 `0x3b0` 增至 `0x470`，
且新的入口在每次布局时查询 preparation 能力成员。这个事实说明原热路径
受到了新增构造逻辑的影响，但尚未证明它能解释整帧回退。
随后将两种会话构造拆为独立函数，保持输入准入、首次值与消费检查不变。
10 项完整输入合同及预热缓存后 callback 改写输入的反例通过，
两种配置也分别完成三轮关闭 tracing 的对照。

| assembly reuse | 操作 | active P50，ms | 变化 |
|---|---|---:|---:|
| 关闭 | 拖拽 | 17.580 → 17.630 | +0.29% |
| 关闭 | 滚轮 | 17.592 → 17.746 | +0.88% |
| 关闭 | resize | 37.823 → 38.613 | +2.09% |
| 开启 | 拖拽 | 23.220 → 23.474 | +1.09% |
| 开启 | 滚轮 | 23.097 → 23.627 | +2.29% |
| 开启 | resize | 45.942 → 45.833 | −0.24% |

拆分后的 preparation 函数局部栈空间为 `0x3c0`，但外层分派仍有自己的
`0x110` 栈帧。关闭 tracing 的 reuse 拖拽 P99 为 25.969 → 31.243 ms。
独立 trace 的 layout exclusive 平均却下降：拖拽 1.420 → 1.413 ms，
滚轮 1.427 → 1.379 ms，resize 5.119 → 4.966 ms。
所以函数拆分没有证明整体成本改善，栈帧变化也不能作为回退的已确认根因。
已撤回这次函数拆分及两个 `inline(never)` 标记，保留完整失败数据。
产物为 `input-split-cache-app` 与 `input-split-cache-receipt.json`，
SHA-256 为 `e0ff4a679571562f239ce4f8d3c0ea8af5fa395977039475b2039b6927d22b7f`。
成本在 `input-split-interaction-default/` 和 `input-split-interaction-reuse/`，
trace 在 `input-split-analysis-*`，收据在 `input-split-trace-receipts.json`。

通用输入接口的正确性验证不等于性能优化验收。输入切片仍保留成本未通过状态；
后续局部队列必须继续对照原 `workset-app`，同时保留输入切片的独立成本，
不能用更慢的中间版本充当优化前基线。父链测量、完整输入检查及默认配置尾部
继续作为下一片的实测对象；生产消费者接入和 FR-02 关闭仍须净成本门禁。

## 静态布局依赖声明准入

Core 已将父布局静态声明接入 manager 安装与替换、Builder 和整支子树准入。
`LayoutManager::register_child_layout_dependencies` 接收 Core 创建的受限 collector；
声明表随 manager 由 `LayoutState` 持有，运行时查询只比较完整 typed property 身份。
旧 manager 没有声明时仍保持原父链传播。此片没有接入局部队列或生产消费者，
因此没有减少布局工作或新增性能收益结论。

`p2_layout_dependency_admission` 的 11 项公开合同通过：
完整 typed 身份、一次注册、忽略重复错误、非法直接归属与跨 Runtime 身份、
重叠声明归并、替换退役、预算失败、忽略预算错误、注册 panic、
子树注册与拒绝隔离、预留 child 身份及子树多个 manager 的累计预算。
pending child 由本次准入中的完整身份关系验证，不能用未来 arena 槽位代替。

Runtime manager 替换及子树准入使用当前 preparation 限额，
子树所有 manager 共用一个预算。Builder 和内部构造使用默认限额。
预算计费涵盖声明条目、归并索引、表 metadata 和 pending child 身份索引；
这些是逻辑集合单位，不是 allocator 字节或进程 RSS。
注册错误在安装前返回，注册 panic 保持既有传播方式；
两者都保留原 manager 和原表，子树失败时撤销全部预留节点。

Core lib 回归为 491 通过、7 忽略；修正子树准入的参数组织后，
lib Clippy 通过，声明准入、原子子树及完整布局输入三组合同分别为
11／28／10 通过，Wasm lib 检查通过。设计文档检查覆盖 106 篇，0 错误。
接下来仍须实现请求 guard、带范围队列和内部 LayoutOutput 提交边界，
再接入 ScrollBar 与外部自有容器。输入切片的独立成本记录继续保留；
最终对照原 `workset-app`，不得改用输入切片的较慢版本作为总基线。

## 请求 guard 与局部队列基础接入

Core 已接入 `layout::LayoutRevalidationGuard` 和
`EventEffects::revalidate_layout_for_property`。guard 构造绑定完整 source、
owned 非目标输入首次值及原注册弱引用，不调用 reader。执行局部项之前，
Runtime 在 preparation 预算内准入首次值和 proof metadata，再调用原 reader。
值不同时升级普通失效；注册退役、原始读取错误与预算失败仍返回原错误。
采用局部项后，原 first proof 进入布局消费与发布检查；source 生命周期回调
返回后也检查原 proof，不以新读取值替换它。

UpdateManager 仍持有一个待执行队列，并保存每项局部范围。
普通失效优先，同属性请求合并首末 property 值但保留最早 guard；
不同属性升级普通失效。即时标记和出队复核按当前父链查静态表，
未知 manager、lifecycle、preparation 与 freeform 状态保持保守传播。
每个待执行子树的 guard 资格检查放在该子树首次回调之前，
不跨越此前另一个子树的外部回调复用成功结果。

内部 output 先完整检查，再决定范围，再写第一项几何。
首片接受 child bounds、repaint 和没有变化的 visibility 写入；实际 visibility 变化、显式 invalidate、
property、viewport 与其他效果都会升级普通传播。
内部 child resize 继续更新真实几何、坐标、通知、damage 及 child 布局；
重验到 source 为止，并以原请求排队下一次局部收敛。
普通输出入口保留既有行为。队列删除、清空、公开 drain 会移除局部 metadata，
checkpoint 和失败恢复可以保守重排为普通项，不能丢失待执行 source。

8 项 guard 定向合同通过，覆盖延后读取、首次值、提前变化、采用后 sticky 冲突、
注册克隆与退役、未知注册、原 reader 错误、预算拒绝和计费溢出。
11 项内部队列合同通过，覆盖 child resize 收敛、未声明祖先、提前变化、
普通与不同属性优先、同属性首次值、visibility 升级、退役失败恢复、
清空／drain／删除、checkpoint、声明替换与 validation 预算耗尽。
完整输入 proof 模块为 20 项通过，Core lib Clippy 与 Wasm lib 检查通过。
此前基础接入后的 Core lib 回归为 497 通过、7 忽略；
后续队列检查调整由上述定向测试覆盖，尚未重复全量回归。

上述基础验证之后已补齐真实事件和消费者，见下文；
混合 FigureTree、净成本和总体分离仍未验收。

## 真实事件、普通消费者与晚期恢复

`p2_layout_input_contract` 的 18 项公开测试通过，独立消费者已执行其中前 17 项；
覆盖自有 typed 属性、未知祖先、普通失效、实际 visibility 输出、提前输入变化、
layout 回调冲突、manager 替换、reparent、source 生命周期冲突和退休 focus 回调。
生命周期反例曾发现“帧准备返回错误，但内部 child 几何没有恢复”的缺口：
原恢复前缀仅由 preparation capability 开启，普通布局没有进入该事务。
现在待执行布局与已有布局输入证明也开启既有恢复前缀，
source 私有模型保持新值，未发布的 child 几何恢复，下一帧执行普通重验。

`FigureTreeBuilder::validate_subtree` 已使用有预算的输入 session 和相同恢复机制。
独立 measurement 查询的无 session 拒绝合同保持。
Viewport 对照发现隐藏 scrollbar 保留的旧布局 proof 不再代表当前发布消费；
已限制布局 proof 的发布检查到当前有效、可见布局。
可见父布局测量隐藏 child 时取得的 proof 仍由父布局独立保留并检查；
owner 重新显示后先检查缓存输入，再复用可见 placements。

ScrollBar 注册完整 `LayoutRange` 和非 value `RangeShape` 两类 owned payload，
字段先按 RangeModel 有限、normalized 规则验证，再按字段的完整位值比较。
完整 range 在 thumb 的可替换 minimum callback 之前通过 `LayoutSceneQuery::input` 读取，
几何只使用这份受检值；请求隔离 guard 不替代它。
ScrollPane 仅为自己的两个 scrollbar 登记 `SCROLL_VALUE` 独立性；
Fill 使用同一静态声明接口。外部自有容器使用自己的 property、payload 和 registry，
没有 Core downcast 或内置专用资格通道。

10 项 ScrollBar、28 项 Viewport、11 项声明准入和 18 项公开输入测试通过；
Core lib Clippy、Core Wasm lib 与独立消费者 Wasm lib 检查通过。
最近 Core lib 全量回归为 510 通过、7 忽略，发生在构造期 session 与隐藏布局修正之前；
后两项由上述定向合同覆盖，最终全量回归仍需补验。

## 基础队列阶段成本

冻结 `input-local-queue-app` SHA256 为
`7717f972c640823b14c2f90d4a3a04c8f9c6da7406433a02952e86e4c988da99`。
沿用原 `workset-app`，两种 assembly 配置各三轮交替运行、每轮两 cycles，
关闭 tracing 采耗时；每配置匹配 753 项操作。独立 Perfetto 捕获使用 reuse 配置。
该冻结版尚未迁移 ScrollBar 消费者。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | active P99 差值 |
|---|---:|---:|---:|
| default／DragMove | 17.4201 → 17.3358 | -0.48% | +2.63% |
| default／Wheel | 17.3875 → 17.4342 | +0.27% | +1.45% |
| default／Resize | 37.2404 → 37.4141 | +0.47% | +5.00% |
| reuse／DragMove | 22.6314 → 22.8724 | +1.07% | +10.96% |
| reuse／Wheel | 22.6538 → 22.8504 | +0.87% | +9.40% |
| reuse／Resize | 45.0083 → 44.6924 | -0.70% | +0.32% |

这些结果未证明稳定净收益，尾部回退仍保留。
Perfetto 的 80 项 DragMove 仍有 2320 次 Flow qualification 和 2160 次 input validate，
即每次 29／27 次；没有由此声称删除上层重验。
原始产物在 `target/verification/figure-projection-2026-11-05/`：
`input-local-queue-receipt.json`、`input-local-queue-interaction-{default,reuse}/`、
`input-local-queue-trace-receipts.json` 与 `input-local-queue-analysis-{workset,input}/`。
分析命令和 SQL 由 `tools/scripts/analyze_textflow_scroll.py` 保留。
几何比较只覆盖文本摘要和 viewport 位移，不能替代像素等价验收。

## 消费者阶段成本与未激活的祖先链

冻结 `input-scrollbar-consumer-app` SHA256：
`24c0cc569fd1b14a70a55452e4e79d01ad1200eda31a4f5c58a126f5c157f8af`。
该版包含普通消费者、晚期恢复、构造期 session 与隐藏布局修正，
仍与原 `workset-app` 做两配置、各三轮的关闭 tracing 对照，每配置匹配 753 项操作。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | active P99 差值 |
|---|---:|---:|---:|
| default／DragMove | 17.2961 → 18.2557 | +5.55% | +0.25% |
| default／Wheel | 17.3651 → 18.2125 | +4.88% | -15.45% |
| default／Resize | 37.4413 → 37.8498 | +1.09% | +0.06% |
| reuse／DragMove | 22.8204 → 22.7875 | -0.14% | +0.33% |
| reuse／Wheel | 22.7480 → 22.6271 | -0.53% | +0.82% |
| reuse／Resize | 44.6661 → 44.9946 | +0.74% | +5.85% |

独立 reuse trace 中，80 项 DragMove 仍有 2320 次 Flow qualification 和
160 次 Flow measurement reuse，即每次仍为 29 次资格读取、2 次文字测量。
总 input validate 从 2160 次增至 2880 次，新增普通 range proof 的成本已进入分母。
DragMove 的 layout inclusive 均值从 3.1567 ms 变为 3.2132 ms；
该结果未证明净收益。

定向源码核对确认文本样例根节点使用 `StackLayout`，它尚未登记内部
`SCROLL_VALUE` 独立性。声明链按合同退化为普通父链，
所以目前不能将这组数据称为“局部路径成本”。
下一片需先核对 Stack 的测量、placement 和 invalidate 合同，再登记成立的声明。
Rust 依据：`crates/novadraw/src/layout/stack_layout.rs`；只读 Draw2D 依据：
`org.eclipse.draw2d.StackLayout` 的 minimum／preferred 聚合及 client-area placement。
无声明的外部 manager 仍保持完整父链，不因样例回退而放松资格。

原始产物使用 `input-scrollbar-consumer-*` 前缀，包含二进制、源码收据、
两配置比较、trace receipts 与两份 analysis。Perfetto 分析曾因 macOS socket
路径 106 字节超过 103 字节限制失败；原 trace 保留，以短会话名继续分析后通过。
这不是采证缺失，也不把重新分析视为新的耗时采样。

## Stack 祖先链激活后的成本

StackLayout 已通过同一个静态声明接口登记 `SCROLL_VALUE` 独立性。
它没有私有布局缓存，invalidate 使用默认空实现；preferred／minimum 聚合 child
尺寸，placement 只使用容器 client area 和 child 身份。请求合同保证 child 外部
几何与测量不变时，内部滚动值不改变这些结果；未知外部 manager 仍须自行声明。
只读 Draw2D 依据仍是 `org.eclipse.draw2d.StackLayout`，不运行 Java。

新增真实 ScrollBar 事件测试通过：value 改变后 thumb 移动，Stack 根没有
`LayoutEventKind::Started`；extent 改变后，根布局重新执行。
ScrollBar 完整合同现为 11 项。公开输入合同现为 19 项，其中新增构造期反例分别
覆盖可见和隐藏 child：可见父布局测量 child 后，父生命周期回调改变模型，
原输入 proof 在发布处拒绝，child 几何恢复，模型新值保留，下一次验证成功。
此前独立消费者的 17 项结果不据此自动增加。

本阶段二进制 `input-stack-consumer-app` SHA-256：
`b5413c6c3d7f94c143b016667854b6bac773758a3ab981a7e9ee572d7aa5dab7`。
两种配置各三轮、每轮两 cycles，继续对照原 `workset-app`，关闭 tracing 采耗时；
每配置匹配 753 项文本摘要与 viewport 位移，每侧每种主要操作 240 个样本。
active 包含主线程操作／输入分发与帧处理耗时，不能代替 frame 或 GPU 完成成本。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | active P99 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|---:|
| default／DragMove | 17.3193 → 16.4684 | -4.91% | +3.77% | +8.41% | +20.00% |
| default／Wheel | 17.3784 → 16.2410 | -6.54% | -17.35% | +5.91% | -2.84% |
| default／Resize | 38.0596 → 38.9983 | +2.47% | -4.10% | +4.08% | -5.96% |
| reuse／DragMove | 22.7450 → 20.4257 | -10.20% | -13.75% | -0.64% | -4.06% |
| reuse／Wheel | 23.1069 → 20.5526 | -11.05% | -33.71% | -0.30% | -30.67% |
| reuse／Resize | 46.0257 → 45.1012 | -2.01% | -3.55% | -1.37% | -9.97% |

两配置拖拽／滚轮逐轮 active 中位数均下降；default resize 三轮分别为
37.0290 → 41.0424、39.0888 → 38.1865、37.1627 → 38.2605 ms。
因此不能将 reuse 的收益写成默认配置或整个切片已经通过。
default Append 仅有 6 个样本，P50 为 25.1608 → 26.6036 ms；
Stable 仅有 9 个样本，P50 为 4.4585 → 5.3412 ms。这些小样本保留为回退线索，
不能据其尾部分位数判断稳定延迟。

独立 reuse Perfetto 的健康检查通过，无 error／data_loss、未闭合 scope 或负
exclusive duration。80 项自然 damage 的 DragMove／Wheel 每侧分别记录：
Flow measurement reuse 和 Flow build 由 160 次降到 0；
Flow qualification 由 2320 次降到 1440 次，即每操作 29 → 18 次。
普通 range proof 仍必须重验，总 input validate 为 2160 → 2640 次，
不能把这项计数写成所有输入检查都减少。
DragMove layout inclusive 均值为 3.2024 → 0.8563 ms，
Wheel 为 3.2493 → 0.8261 ms。
resize 的 Flow build 仍为 1036 次，qualification 仍为 6776 次；
layout inclusive 均值为 22.6657 → 23.8977 ms。
traced layout 成本支持父链局部化已生效，不抵消关闭 tracing 的 frame／resize 回退。

产物、两配置比较、trace receipts 和两份 analysis 均以
`input-stack-consumer-*` 为前缀，保存在既有本地验证目录。
后续整改先跳过没有待检查 proof 的节点，避免为它们反复遍历可见性父链；
有 proof 的 owner 仍执行原 reader、首次值比较与发布校验。
该删减的定向输入／ScrollBar／Viewport 测试为 19／11／28 项通过；
受影响的 Core lib 与三个 integration target 的 Clippy 通过。
新 release 的采样结果见下节，FR-02 与通用投影分离保持未完成。

## 空 proof 节点过滤后的成本与像素

冻结 `input-proof-filter-app` SHA-256：
`1914c624e8bce22d8ee1e6c1edf167db1722f47fdd940eb0a13101d3936d8a48`。
仍与原 `workset-app` 做两配置各三轮、每轮两 cycles 的交替对照，
关闭 tracing，每配置匹配 753 项文本摘要与 viewport 位移。
这组数据衡量当前实现相对总基线的成本；不能通过与上一组不同时间段的
分位数相减，认定过滤这一步的独立收益。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | active P99 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|---:|
| default／DragMove | 17.4069 → 15.8298 | -9.06% | -14.27% | +2.17% | -3.20% |
| default／Wheel | 17.4687 → 15.9319 | -8.80% | -14.41% | +3.29% | -6.06% |
| default／Resize | 37.0556 → 38.8180 | +4.76% | +4.27% | +4.02% | +7.88% |
| reuse／DragMove | 22.6602 → 20.7160 | -8.58% | -11.73% | +2.51% | -3.63% |
| reuse／Wheel | 22.6194 → 20.6240 | -8.82% | -2.43% | +2.33% | +8.39% |
| reuse／Resize | 45.2593 → 44.1422 | -2.47% | -4.91% | -1.86% | +1.54% |

default resize 的三轮 active 中位数均上升：
37.1033 → 37.9769、36.9562 → 38.0231、37.3165 → 39.8391 ms。
拖拽／滚轮的总 active 改善已在两配置中复现，frame 中位数仍增加，
resize 和部分 frame 尾部仍回退，过滤没有解决整体成本问题。
原始耗时、收据及独立 Perfetto 分析保存在 `input-proof-filter-*`。
后续继续检查普通布局 proof 重验时的全树 style／scene 构造；
这些重读必须保留，构造成本与 callback 结果的有效性分开处理。

像素回放使用独立的 `collect_projection_pixels.py input-proof-filter`，
没有混入耗时或 tracing 对照。两配置分别运行相同 raster validation 序列，
覆盖往返滚动、追加、变窄、恢复宽度及重置。
每配置 43 对 retained GPU target 截图全部逐像素相等：
changed pixels 为 0，max channel delta 为 0。
命令、二进制及每张图片 SHA-256 保存在
`input-proof-filter-pixel-receipts.json`；比较在
`input-proof-filter-pixel-comparison-{default,reuse}.json`。
截图不是桌面窗口采集，范围仅为这个长文本回放，不等于混合大树或整棵
FigureTree 的像素、性能及设备恢复验收。
上述修正之后的 Core lib 完整回归为 510 通过、7 忽略，0 失败，
日志在 `input-proof-filter-core-unit.log`；既有测试目标警告仍存在。
独立 `novadraw-style-input-consumer` 已重新运行全部 19 项布局输入合同，全部通过；
Core 与该独立消费者的 Wasm lib check 均通过。
独立消费者自行解析依赖，这两项检查证明公开合同可消费，不用于替换冻结性能分母。

另做一组相邻版本对照，单独测量过滤步骤：
`input-stack-consumer-app` → `input-proof-filter-app`，两配置各三轮、每轮两 cycles，
关闭 tracing，每配置匹配 753 项操作。此组不替代相对原 `workset-app` 的总成本。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|
| default／DragMove | 15.9335 → 15.8595 | -0.46% | -0.21% | -3.11% |
| default／Wheel | 15.8310 → 15.8944 | +0.40% | +0.43% | +25.82% |
| default／Resize | 38.1942 → 38.0803 | -0.30% | +0.31% | +18.48% |
| reuse／DragMove | 20.4570 → 20.2101 | -1.21% | -1.43% | -6.45% |
| reuse／Wheel | 20.4391 → 20.2960 | -0.70% | -0.91% | -16.65% |
| reuse／Resize | 45.0441 → 44.4543 | -1.31% | -0.69% | -13.04% |

过滤步骤在 reuse 中有小幅改善；default 中位数接近持平，滚轮和 resize 的
frame 尾部仍回退。因此没有将这一步记为独立性能验收通过，也不据此认定尾部
回退的全部原因。匹配记录与收据在 `input-proof-filter-adjacent-{default,reuse}/`，
采样命令由 `collect_proof_filter_adjacent.py` 保留。

下一片开始前新增继承样式反例：普通父布局测量 child 的私有输入 reader，
reader 读取 child 的继承字体大小；两次独立稳定借用分别观察 20 px 与 30 px，
生成的 child 宽度同步更新。公开输入合同现为 20 项并全部通过；
独立消费者的最近一次运行仍为上述 19 项。混合树测试目标 Clippy 通过。
样式延后构造尚未实现，本结果固定的是改动前必须保持的行为。

## 输入检查按需构造样式

此片已将 guard、布局消费、缓存重验和最终发布检查中的样式构造延后到首次
`PreparationScene::style` 查询。`PreparationInputs` 保存当前不可变树借用；
已有 producer 可继续借用提前构造的 styles，普通检查则在首次读取时构造并计费，
在同一借用中共享结果。不同布局输出或树借用重新建立 inputs，
外部 reader 和首次值比较仍真实执行。

发布检查使用当前 scene 已冻结的根子树成员判断 attachment，与原 styles 中的
成员范围相同；不为没有样式读取的 proof 额外生成全树样式表。
Runtime 的普通测量缓存仍以是否已准入 input scene 判断资格，
不会因样式尚未构造而跳过已准入私有输入的 callback。

Core lib 回归为 511 项通过、7 项忽略；混合平移 2 项和 ScrollBar 11 项通过。
公开输入合同增加至 21 项并全部通过，覆盖继承字体的跨借用变化、
隐藏 child 的父测量输入和晚期冲突恢复。
新增内部反例确认同一借用只构造一份样式，首次分配预算失败保留；
公开反例确认 reader 即使忽略该失败、返回 payload，Runtime 仍拒绝发布。
日志分别为 `input-lazy-style-tests.log` 与 `input-lazy-style-input-tests.log`。
独立消费者的 59 项测试通过，其中包含全部 21 项公开输入合同；
Core Wasm lib check、受影响目标 Clippy 和 workspace fmt check 通过。
既有 Freeform、Viewport、Connection contract/runtime 共 96 项回归通过，
日志在 `input-lazy-style-geometry-tests.log`。这些回归不替代混合大树的成本验收。
独立消费者 Wasm lib check 也已通过，完整日志在
`input-lazy-style-consumer-wasm.log`。

冻结 `input-lazy-style-app` SHA-256：
`e83818f93cd8be9db82389f446ca280c8d0d1d49c2a48dcf801577717845d7ba`。
依赖 lock 与原 `workset-app` 分母保持不变；两配置各三轮、每轮两 cycles，
关闭 tracing，每配置匹配 753 项操作。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | active P99 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|---:|
| default／DragMove | 17.6633 → 15.4150 | -12.73% | -7.64% | +2.01% | +7.97% |
| default／Wheel | 17.9135 → 15.4668 | -13.66% | -18.32% | +0.47% | -7.19% |
| default／Resize | 37.8392 → 37.5445 | -0.78% | -5.39% | +2.69% | +7.52% |
| reuse／DragMove | 22.7003 → 20.1057 | -11.43% | -16.07% | +0.93% | -9.16% |
| reuse／Wheel | 22.6833 → 19.9758 | -11.94% | -13.11% | +0.23% | -1.46% |
| reuse／Resize | 44.6174 → 43.6361 | -2.20% | -1.36% | +1.26% | -1.60% |

表中 active 为主线程输入与帧处理的合计耗时，frame 为对应帧处理耗时；
未启用 GPU wait，不代表 GPU 完成或真实窗口端到端延迟。
累计 active 中位数下降，default 拖拽与 resize 的 frame 尾部仍上升。
同一采样中的逐轮值、P95／P99、命令与二进制收据保存在
`input-lazy-style-interaction-{default,reuse}/`。
独立 Perfetto 总基线对照的健康检查通过：无 error／data_loss，
未结束 scope 与负 exclusive 时间均为 0。
每 80 次 DragMove／Wheel 的 Flow build 和 measurement reuse 仍为 160 → 0 次，
qualification 仍为 2320 → 1440 次，普通 input validate 为 2160 → 2640 次。
layout inclusive 均值分别为 3.2731 → 0.3784 ms、3.3017 → 0.3739 ms。
resize 的 Flow build 与 qualification 仍为 1036／6776 次，
layout inclusive 均值为 23.5412 → 20.9920 ms。
这是相对原总基线的归因，没有单独证明延后样式这一步的收益，
也不抵消关闭 tracing 时的 frame 回退。收据和分析在
`input-lazy-style-trace-*` 与 `input-lazy-style-analysis-*`。

相邻版本另做两配置各三轮的无 tracing 对照：
`input-proof-filter-app` → `input-lazy-style-app`，每配置匹配 753 项操作。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|
| default／DragMove | 16.0645 → 14.9906 | -6.69% | -3.20% | -29.55% |
| default／Wheel | 16.0633 → 14.9533 | -6.91% | -3.60% | -2.54% |
| default／Resize | 38.5883 → 36.1432 | -6.34% | -2.41% | -4.97% |
| reuse／DragMove | 20.2141 → 20.6622 | +2.22% | +4.81% | +38.80% |
| reuse／Wheel | 20.3580 → 20.4293 | +0.35% | +2.22% | +1.66% |
| reuse／Resize | 44.0913 → 43.3270 | -1.73% | +1.08% | +7.73% |

reuse 拖拽的三轮 active 中位数均上升：
20.2573 → 20.6971、20.2289 → 20.9998、20.1784 → 20.3167 ms。
默认路径有改善，reuse 的独立拖拽与尾部回退仍需定位，
不将这一步记为独立性能验收通过。全部分位数和逐轮值在
`input-lazy-style-cost-summary.json`。
相邻 Perfetto 健康检查通过，产物前缀为 `input-lazy-style-adjacent-*`。
其中 `workset` 标签实际使用 `input-proof-filter-app`，
`input` 标签使用 `input-lazy-style-app`，真实命令与 hash 由 receipts 保留。
DragMove／Wheel layout inclusive 均值分别从 0.8384／0.8138 ms
降到 0.3893／0.3967 ms，但 Flow qualification 从 3.0019／2.9641 ms
升到 3.3756／3.4789 ms，次数保持不变。
此证据将新增 qualification 成本列为整改线索，未证明所有 frame 回退都来自它。

延后构造时移除了 scene 中对既有 borrowed styles 的直接引用，
使该类样式查询也通过 inputs 间接取值。后续候选恢复通用 borrowed snapshot
的直接读取，仅在没有提前提供样式表时查询 deferred snapshot；
两种存储遵守同一作用域和预算，按数据所有权选择，不按 Figure 类型区分。
该修正已通过 10 项 preparation scene、21 项公开输入、11 项 ScrollBar、
2 项混合平移定向测试，以及受影响目标 Clippy、workspace fmt 和 Core Wasm 检查。
日志在 `input-lazy-direct-checks.log`；新的冻结 release 已构建，
`input-lazy-direct-app` SHA-256 为
`13213ebed7acb68f6dd4a4d9d1a2159f7aa6dd0544ee1d153caa78bd079dc10c`。
依赖 lock 保持，收据在 `input-lazy-direct-receipt.json`；
后续成本见下文，不能从定向测试推断回退已解决。

独立像素回放也已完成：两配置各 43 对 retained GPU target 截图全部逐像素相等，
changed pixels 和 max channel delta 均为 0。命令、图片与二进制 hash 在
`input-lazy-style-pixel-receipts.json`，结果在
`input-lazy-style-pixel-comparison-{default,reuse}.json`。
已目视核对包含正文、不同字号及滚动条的非空白 frame；
覆盖范围仍是长文本回放，不替代混合大树或真实窗口验收。
FR-02 和通用门禁保持未完成。

## 冻结样式直接读取的成本复核

`input-lazy-direct-app` 的两组对照均已完成：总基线使用 `workset-app`，
相邻基线使用 `input-proof-filter-app`。每组的 default／reuse 配置各三轮，
每轮两 cycles，关闭 tracing，每配置匹配 753 项操作。
相邻组没有使用已出现回退的 `input-lazy-style-app` 作为分母。

| 对照／配置／操作 | active P50 前 → 后（ms） | 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|
| 总／default／DragMove | 17.6369 → 15.3932 | -12.72% | +3.05% | -1.82% |
| 总／default／Wheel | 17.7785 → 15.1046 | -15.04% | -0.34% | +8.92% |
| 总／default／Resize | 37.4526 → 36.2504 | -3.21% | +1.25% | +0.69% |
| 总／reuse／DragMove | 23.0527 → 19.9326 | -13.53% | -1.76% | -5.01% |
| 总／reuse／Wheel | 22.9700 → 20.0271 | -12.81% | +0.33% | -41.16% |
| 总／reuse／Resize | 45.2341 → 43.2595 | -4.37% | +0.70% | -11.70% |
| 相邻／default／DragMove | 15.8356 → 14.9615 | -5.52% | -1.98% | -3.08% |
| 相邻／default／Wheel | 15.9095 → 14.9867 | -5.80% | -1.93% | +0.11% |
| 相邻／default／Resize | 39.1192 → 36.1025 | -7.71% | -4.33% | -13.72% |
| 相邻／reuse／DragMove | 20.2959 → 19.7973 | -2.46% | -0.27% | -8.89% |
| 相邻／reuse／Wheel | 20.3051 → 19.8474 | -2.25% | -0.21% | -13.99% |
| 相邻／reuse／Resize | 44.6833 → 43.5257 | -2.59% | +0.95% | -0.60% |

相邻组 reuse 拖拽的三轮 active 中位数均下降：
20.3467 → 19.8609、20.2243 → 19.8503、20.3106 → 19.7525 ms。
这组数据支持按需样式版本相对上一片有 active 中位数收益。
总基线 default 滚轮的 frame P99、default 拖拽的 frame P50 和
相邻 reuse resize 的 frame P50 仍上升，整帧净收益门禁保持未完成。
全部 P50／P95／P99、逐轮值和真实二进制收据保存在
`input-lazy-direct-cost-summary.json` 与两组采样目录。

独立 Perfetto 的总组、相邻组均通过健康检查，无 error／data_loss，
未结束 scope 和负 exclusive 时间均为 0。相邻组 DragMove／Wheel 的
layout inclusive 均值为 0.8086／0.8038 → 0.3827／0.3766 ms；
Flow qualification 均值仍从 2.9677／2.9468 升到 3.3875／3.3283 ms，
各 80 次操作的 qualification 仍为 1440 次，input validate 仍为 2640 次。
因此没有将关闭 tracing 时的改善归因于 qualification 成本已解决。
数据在 `input-lazy-direct[-adjacent]-analysis-*`，可重跑 SQL 见开头的冻结入口，
命令与 hash 在对应 trace receipts；这些 trace 不用于替换无 tracing 耗时。

独立 retained GPU target 像素对照为两配置各 43 对截图，全部逐像素相等；
changed pixels 和 max channel delta 均为 0。
结果在 `input-lazy-direct-pixel-comparison-{default,reuse}.json`，
命令、二进制及每张图片 hash 在 `input-lazy-direct-pixel-receipts.json`。
验证范围仍为长文本回放，未包含真实窗口或通用混合大树。

## 成员与 route 范围按需确认

根 scene 不再预先枚举完整子树。首次查询某个 Figure 时，Core 沿其当前父链
找到 root 或本借用已经确认的成员，保存完整 Figure 身份与继承的 route 归属。
相同 root 的后续查询复用这些不可变结构事实；范围之外的来源仍拒绝。
scoped measurement 和 input scope 各自持有 root 的范围，不能因上层 scene
确认了 sibling 就扩大下层 reader 的读取权。

待插入的完整父链在写入共享表前计费。预算拒绝不安装该次查询的部分成员；
失败仍留在同一个累计 budget，忽略错误不能使候选获得发布权。
发布阶段的 attachment 查询同样受检并传播预算错误。
缓存没有保存 reader 值、provider revision 或成功结果，完整 source prefix 和
revision 的初次读取仍遍历全部子树，后续仍重读每个外部 provider。

新增两项反例通过：添加 512 个无关 sibling 后，单输入 scope 的 work 与 allocation
均不增加；在父链插入前耗尽预算，成员表保持原样，新借用可重新查询。
既有共享 root、下层范围、继承 route、reparent、继承样式与 provider 重读反例通过。
preparation scene 共 12 项、公开输入 21 项、ScrollBar 11 项、
混合平移 2 项全部通过，日志在 `input-member-tests.log`。
Core lib 513 项通过、7 项忽略，独立消费者 59 项通过；
受影响目标 Clippy、Core Wasm lib check 和 workspace fmt check 通过，
日志在 `input-member-checks.log`。后续几何回归、消费者 Wasm 与冻结成本待补。
通用投影分离仍未完成，结构成本反例不作为整帧性能验收。

这一版的几何回归 96 项、消费者 Wasm 检查随后通过，日志在
`input-member-geometry.log`。冻结 `input-member-app` SHA-256 为
`7f842a4d13903303c2823f436a72b36dbe005096c09a19c55af12534694ff728`，
lock 与总基线不变。两配置的三轮总对照及相邻对照完成，
每配置匹配 753 项操作，原始样本、命令与 hash 由 receipts 保留。
相邻分母为 `input-lazy-direct-app`，这组单独测量成员构造调整。

| 配置／操作 | active P50 前 → 后（ms） | 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|
| default／DragMove | 14.9894 → 14.6192 | -2.47% | -1.05% | +0.92% |
| default／Wheel | 14.9193 → 14.6499 | -1.81% | -0.81% | -8.57% |
| default／Resize | 36.3395 → 36.6825 | +0.94% | +0.37% | +16.88% |
| reuse／DragMove | 20.0588 → 19.8725 | -0.93% | -0.18% | +2.40% |
| reuse／Wheel | 20.2227 → 19.8575 | -1.81% | -0.98% | -9.36% |
| reuse／Resize | 43.2874 → 43.2480 | -0.09% | -0.16% | +4.92% |

两种配置的逐轮拖拽／滚轮 active 中位数均下降，default resize 的三轮有两轮上升，
且两配置的 resize frame 尾部仍回退；这版没有通过净成本验收。
总组 default 拖拽／滚轮／resize active P50 下降 17.25%／15.60%／3.69%，
reuse 下降 12.06%／13.14%／2.48%，但总组也有 frame 回退。
完整分位数与逐轮值在 `input-member-cost-summary.json`，
不能用总收益替代相邻步骤的成本判断。

独立相邻 Perfetto 健康检查通过。各 80 次 DragMove／Wheel 的
layout inclusive 均值为 0.3902／0.3888 → 0.1593／0.1578 ms，
但 Flow qualification 为 3.4020／3.3923 → 4.1292／4.1187 ms。
resize qualification 为 12.2095 → 14.2153 ms；
qualification／input validate 次数保持，未发生删去外部重读。
这将完整读取路径的成员查询成本列为下一整改线索，不归因全部 frame 回退。
总与相邻数据和收据在 `input-member[-adjacent]-analysis-*` 及对应 receipts，
可重跑 SQL 见开头的冻结入口。
独立两配置各 43 对 retained GPU target 截图逐像素相等，
结果和图片 hash 在 `input-member-pixel-*`。
此试验的两份改动源码也按 receipt hash 保存为 `input-member-scene.rs`
和 `input-member-presentation.rs`，后续调整不覆盖原始证据。

后续版本保留单输入的按需父链查询；首次请求完整 source 或 revision 时，
一次性冻结完整子树及 route 成员，后续直接读取集合。
构造仍计费，失败不安装部分完整集合；外部 provider revision 和 input reader
仍实际重读。它减少完整输入读取中的重复成员查询，不新增免检查资格。
该调整的验证结果如下，不提前判断收益。

完整读取修正已通过 Core lib 514 项（7 项忽略）、21 项公开输入、
11 项 ScrollBar、2 项混合平移、96 项几何回归和独立消费者 59 项。
新增完整集合预算失败反例确认：失败不安装 member set 或 source prefix；
单输入的无关 sibling 成本反例仍通过。
受影响目标 Clippy、Core／消费者 Wasm lib check 和 workspace fmt check 通过，
日志在 `input-member-bulk-checks.log`。
冻结 `input-member-bulk-app` SHA-256 为
`3ab0b2128581862563b66e6d7bd46ea9ccb372add175eca46e1681604ae55007`，
lock 保持，收据在 `input-member-bulk-receipt.json`。
总组分母仍为 `workset-app`，相邻分母仍为 `input-lazy-direct-app`；
未使用较慢的首次成员试验作分母。两组采样完成，每配置匹配 753 项操作。

| 对照／配置／操作 | active P50 前 → 后（ms） | 差值 | frame P50 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|
| 总／default／DragMove | 17.4208 → 14.6584 | -15.86% | +0.57% | -6.55% |
| 总／default／Wheel | 17.3993 → 14.6232 | -15.96% | +0.58% | -3.49% |
| 总／default／Resize | 38.3016 → 36.0096 | -5.98% | -0.59% | -8.55% |
| 总／reuse／DragMove | 23.0156 → 19.0363 | -17.29% | -3.84% | +23.38% |
| 总／reuse／Wheel | 22.6223 → 19.1066 | -15.54% | -1.11% | +2.06% |
| 总／reuse／Resize | 44.3228 → 40.9475 | -7.62% | -1.79% | +0.02% |
| 相邻／default／DragMove | 15.0394 → 14.5776 | -3.07% | -1.27% | -9.49% |
| 相邻／default／Wheel | 14.9272 → 14.6094 | -2.13% | -0.23% | +0.22% |
| 相邻／default／Resize | 36.1049 → 36.3796 | +0.76% | +0.59% | -0.19% |
| 相邻／reuse／DragMove | 19.7913 → 18.9651 | -4.17% | -2.16% | -15.53% |
| 相邻／reuse／Wheel | 20.0465 → 19.0889 | -4.78% | -2.93% | -12.84% |
| 相邻／reuse／Resize | 43.4098 → 40.4342 | -6.85% | -1.81% | -16.22% |

相邻组 default 的拖拽／滚轮及 reuse 的拖拽／滚轮／resize，
三轮 active 中位数均下降。default resize 三轮为
35.7911 → 37.2901、36.1049 → 35.8360、36.4256 → 35.8245 ms；
合并 P50 仍上升，且总组 reuse 拖拽 frame P99 上升 23.38%。
因此不关闭净成本或整帧尾部门禁。完整分位数、逐轮值、命令和 hash 在
`input-member-bulk-cost-summary.json` 及两组采样目录，原始不利样本保持。

独立 reuse 相邻 Perfetto 健康检查通过，DragMove／Wheel 的
layout inclusive 均值为 0.3778／0.3730 → 0.1480／0.1457 ms，
qualification 为 3.2781／3.2876 → 3.5211／3.5287 ms，
input validate exclusive 为 0.3272／0.3203 → 0.0881／0.0869 ms。
qualification 和 validate 各为 1440／2640 次，次数没有删减。
resize 的 layout 为 20.9439 → 19.5132 ms，qualification 为
11.6144 → 12.1027 ms。完整成员查询的新增 qualification 成本尚未完全消除；
这些 reuse 数据不解释 default 的 resize 波动。
两配置各 43 对独立 retained GPU target 截图逐像素相等，
像素结果、命令与图片 hash 在 `input-member-bulk-pixel-*`。
针对 default resize 的未解决变化，追加同一对冻结产物的三轮匹配复验，
并另采 default 独立 trace；追加结果不替换上述首组数据。

default 无 tracing 复验同样匹配 753 项操作，结果如下。

| 操作 | active P50 前 → 后（ms） | 差值 | active P95 差值 | active P99 差值 | frame P99 差值 |
|---|---:|---:|---:|---:|---:|
| DragMove | 15.5485 → 15.1216 | -2.75% | +4.70% | +1.53% | +2.01% |
| Wheel | 15.5637 → 15.1971 | -2.36% | +6.92% | +12.82% | +14.39% |
| Resize | 37.6443 → 36.9888 | -1.74% | +0.67% | +5.46% | -12.32% |

三项操作的逐轮 active 中位数均下降，resize 为
39.8348 → 37.7150、37.4428 → 36.6412、36.7428 → 36.1265 ms。
首组 resize 中位数上升没有在此次复验中重复，但拖拽、滚轮和 resize 的部分尾部
仍上升；不能将这组改善写成整帧稳定收益已通过。
两组 resize 结果均保留，追加样本与收据在 `input-member-bulk-recheck-default/`。

default 的首次两-cycle trace 超出 256 MiB ring buffer：
bytes overwritten 为 90,468,352，chunks overwritten 为 22,087，
137,992 个 packet 因缺失 incremental state 被跳过，早期 FullDamage 记录丢失。
该 trace 已拒绝用于归因。`analyze_textflow_scroll.py` 现显式拒绝 overwrite 和
上述 skipped packet 计数，即使 stats 将它们标为 info／notice；
实际失败 trace 的负例验证通过，拒绝记录及 hash 在
`input-member-bulk-default-overwrite-rejection.json`，原始 trace 和失败结果保留。

另采 default 一-cycle 匹配 trace，前后 buffer bytes written 均为 181,702,656，
无 overwrite／skipped packet、error／data_loss，未结束 scope 与负 exclusive 时间
均为 0。各 40 次 DragMove／Wheel 的 layout inclusive 均值为
0.4185／0.3972 → 0.1560／0.1581 ms，input validate 仍各为 600 次，
没有 Flow qualification scope。resize 的 layout inclusive 为
67.2890 → 67.8678 ms，exclusive 为 3.2820 → 1.5282 ms，
签名阶段为 1.9685 → 2.0049 ms。Tracing 放大了默认完整组装成本，
这些值只用于阶段定位，不替代无 tracing 的耗时。
有效数据和真实命令在 `input-member-bulk-default-short-adjacent-*`；
此组只有一 cycle，不与前述两-cycle trace 混用调用总次数。
通用投影、稳定尾部与混合大树的门禁继续保持未完成。

## 混合树平移的定向合同

`p2_figure_tree_projection_contract` 的两项测试通过，组合节点内包含
RectangleFigure、已完成资源注册的 ImageFigure、LabelFigure 和外部 Figure，
布局由没有新增独立性声明的外部 manager 提供。
固定尺寸移动组合节点后，后代局部 bounds 保持不变，外部 layout 调用为零；
计数文字引擎确认没有新增 layout／shaping 调用。
IR 中的字形数据与局部 origin 不变，共享 glyph buffer 地址保持，
图片资源引用、source rectangle 和局部 destination 保持。
命中与正反向坐标查询反映新位置，每个后代收到一次对应的祖先移动通知。

外部 paint 没有绘制有效性声明，移动后仍真实执行 callback。
另一个反例确认 resize 会执行外部布局，reparent 后坐标与命中使用当前父链。
这些是当前平移合同的正反例，没有新增缓存权或跨回调免检查资格；
没有覆盖像素等价、大树成本、嵌套 viewport、Freeform、跨子树 connection
或 GPU 完成时间，不能作为通用投影分离的最终验收。

## 受检投影的共享输入内核

2026-11-06，按用户接受后的行为合同开始实施。首先将 typed registry、
注册、proof 与验证 scope 按阶段参数化。每个阶段提供自己的 scene 和错误类型，
原 `PreparationInputRegistry` 保留公开包装；完整 snapshot 比较、首次值合并、
注册弱引用和留存 lease 共用实现。当前尚未添加可用的 projection provider，
坐标查询及 Runtime 帧继续在后续切片接入。

同一活动 input 的递归读取在进入 reader 前拒绝，返回
`PreparationInputErrorKind::DependencyCycle`。活动路径随验证 scope 共享，
guard 在退出及 panic 展开时释放。reader 返回后检查所在 scope 的锁存失败：
即使忽略嵌套读取错误并返回成功值，外层 proof 仍不能采用。
失败 session 保持失败；重新建立 session 才允许恢复。

原输入回归 20 项通过。新增环检测与最终 reader 忽略嵌套错误的反例后，
22 项输入回归通过。另增加第二阶段的受限 scene 验证，共用内核不依赖
Tree、Figure、measurement 或 preparation 读取权限，并核对首次值冲突、
注册退休后旧 snapshot 可读及最后一个 lease 释放。
第二阶段测试通过，完整 Core `--lib` 回归为 517 项通过、7 项既有忽略，
包括最大树深度的 dispose 及原子拒绝测试。测试构建报告了既有
`NdCanvas` 私有方法未使用和 Runtime 测试未消费 acknowledgement 的警告；
本轮共享内核没有新增对应警告。独立 `novadraw-style-input-consumer`
的 59 项测试通过，覆盖布局输入、忽略错误、晚期冲突、退休和恢复。
之后把活动路径移到本轮 `PreparationInputs` 持有，验证 scope 只克隆共享路径，
避免每次重验建立一个随后被替换的路径对象。末次 23 项输入回归及 Core
`--lib` Clippy `-D warnings` 通过，`cargo xtask docs` 与
`git diff --check` 通过。中文半角标点及破折号候选扫描为零；
本轮只做措辞自审，未新增独立设计复核。

新日志保存在工作区外的 `/tmp/novadraw-projection-input-*-20261106.log`，
包括 Core、末次 input、Clippy 和 docs。Cargo 使用默认系统工具链，
单 job、关闭 debug info 和 incremental，构建目录复用
`/tmp/novadraw-text-stage-target`。此前冻结的 `target/verification/` 证据保持原样。

首次新增回归运行因并行富文本样例的 workspace manifest 暂未完成而拒绝启动；
样例的 `src/main.rs` 出现后重跑通过，未修改该并行工作。
以上属于共享输入内核的正确性证据，不证明受检投影、整帧收益或 FR-02 已完成。

## 受限投影输入与准入基础

2026-11-06，新增 `FigureProjection`、`ProjectionCapability`／`PROJECTION`、
`ProjectionContribution`、`ProjectionScene` 与 `ProjectionInputRegistry`。
注册必须同时具备可解析的 projection 和 container adapter，否则在树准入前
返回 `FigureCapabilityRegistrationError::InvalidCapability`。
producer 只返回额外 child transform，Core insets 与 placement 的应用方式不变。
当前只完成类型和读取基础，坐标、命中与 Runtime 尚未改用该来源。

scene 只暴露 root 内的 parent、children、origin、size、insets、解析样式及 typed
input，不开放原始 tree／Figure、坐标、measurement、route 或 prepared drawing。
身份先于范围解释；范围失败、输入失败和预算失败锁存，即使回调忽略 Result
仍拒绝输出。完整 Core 事实读取集与局部表现变换尚待接入，现有 tree 不可变借用
只保障本次读取，不能据此赋予跨查询缓存权。输入留存使用显式字节限额，
snapshot 与 proof 沿共用 lease 记账；尚未建立 contribution／prefix 留存预算。

共享内核补入验证 scope 的嵌套读取回传。最终检查逐项克隆 proof 的共享句柄，
不跨 reader 持有收集器借用；新发现的 subject 会继续加入同一次验证闭包。
外层 snapshot 未变时，内部输入变化仍拒绝；验证时首次发现的嵌套输入也不能逃过
本轮检查。reader 的纯、有界及稳定读取前提继续有效。

6 项定向测试通过，覆盖合法与非法准入、外层值不变时内部冲突、验证时新发现
的依赖、忽略范围／预算错误、真实 input 递归与新 scope 重试、退休及最终 lease
释放。完整 Core `--lib` 为 523 项通过、7 项既有忽略，原布局、准备、帧恢复
及深树回归通过。新增代码未完成消费者接入，暂不报告 Clippy、Wasm、
独立投影消费者或净成本验收通过；总体 FR-02 继续未关闭。

本地日志为 `/tmp/novadraw-projection-stage-tests-20261106.log` 与
`/tmp/novadraw-projection-stage-core-20261106.log`。初次编译发现 GAT lifetime
约束及测试 Figure 必需方法遗漏，修正后定向与 Core 回归通过。

## 投影的 Core 事实与历史输入资格基础

2026-11-06，受限 scene 的树读取已记录到首次值日志：完整 FigureId、
成员检查的 parent 路径、有序 children（含空集合）、origin、size、insets、
逐级原始样式及 provider owner 的 component revision。origin 与 size
分别登记；继承来源发生变化，即使最终解析样式相等，历史事实仍可识别差异。
Core subject 用内部类型身份索引，诊断名字不参与匹配。

独立验证 scope 把 Core 事实与嵌套 input 一起回传，Core 批次合并先检查全部
冲突与预算，再采用新记录；冲突锁存为 `ProjectionError::CoreChanged`。
provider 返回后的检查在全部 reader 结束后重验 Core 事实。
children、字体 family、tooltip backing 与记录／索引均按复制前预算计费。
当前只记录 tree 的局部工作值，Runtime 表现层 local transform 仍待接入。

共用输入内核新增历史 subject 的重新捕获：旧 snapshot 保持不变，
本次新值由消费方作为首次值合并。历史差异供缓存判为 miss，
本次首次值不能被其他 consumer 覆盖。这尚未接入 contribution 缓存；
查询读值日志与最终消费证明仍需分别维护，不能让已经退出消费的旧依赖
继续决定最终发布。

11 项投影定向测试通过，包含原 6 项和新增的空 children、origin／size
独立性、继承来源相等值反例、嵌套 Core 事实回传、历史 miss／本次冲突、
大 tooltip 的分配前预算拒绝。末次定向日志：
`/tmp/novadraw-projection-facts-tests-20261106.log`。
此片完成后的 Core `--lib` 回归为 528 项通过、7 项既有忽略、0 失败，
耗时 51.63 s；日志为 `/tmp/novadraw-projection-facts-core-20261106.log`。
尚未报告坐标、Runtime、独立投影消费者、Clippy／Wasm 或净成本验收完成。

## 独立坐标查询采用受检 contribution

能力归属：Core FigureTree；2026-11-06 接续已授权实施。
五个坐标入口在一次 session 中完成父链组合、求逆及最终验证。
已准入 provider 是额外 child transform 的唯一来源，未准入容器仍调用实时
回调。insets 在 Core 一侧组合；有限奇异正向合法，逆向仍为 Ok(None)。
`CoordinateQueryError::Projection` 保留原领域 source，并取消 Copy。
构造入口允许显式配置单次 work／allocation 及整树 contribution 留存字节。

各 owner 的 scene 共享操作首次 input 日志与锁存失败，分别保留实际消费集合。
历史 proof 刷新后逐项比较，不覆盖旧 snapshot；差异触发完整 contribution
重算，退休 reader 等错误拒绝候选。miss 的旧集合退出最终验证，
资格读取的本次首次值仍约束后续消费者。正向／求逆及所有仍消费的证明成功后
才一起安装 contribution，失败不安装本次部分结果。树退休节点释放其缓存。
Core 事实、集合 backing、receipt 与缓存索引逻辑开销由树的 retention lease
计费，snapshot／input proof 另由 registry 域计费。缓存淘汰与 world prefix 尚未实现。

13 项 Core 投影测试及独立 `tests/consumers/figure-projection` 的 4 项测试通过。
后者验证共享值 A → B 在 component revision 不变时 miss 重算、同值省 provider
回调、唯一来源、后续未知容器回调导致输入冲突时拒绝且重试重新计算、
非交换矩阵／非对称 insets／placement、有限奇异与显式预算拒绝。
新增 Core 反例验证 obsolete 消费退出但首次值保留、Core 变化不能掩盖 reader 退休。

初次作用域拆分遗漏了 verifier 新发现事实的回传，2 项已有测试失败；
修正后重新运行 13 项通过。外部测试第一次使用了不存在的 RectangleFigure
builder 方法，改用实现 initial_insets 的外部 Figure 后 4 项通过。
测试日志为 `/tmp/novadraw-projection-query-tests-20261106.log`。
此时 Runtime、命中／事件失败传播、source 原子切换、表现层 local transform、
整帧共享预算／最终复核及净成本仍未完成。总体 FR-02 保持未关闭。

末次复核补入 verifier 新发现的嵌套依赖：缓存安装前比较完整消费集合，
必要时保存最终检查后的 receipt，防止只保存 provider 返回时的旧集合。
同一次查询重复消费同一 owner 使用同一 contribution 和 scope。
最终 14 项 Core 投影测试、6 项独立消费者测试、Core `--lib` Clippy
（`-D warnings`）及消费者 Wasm check 通过。新增外部反例确认
inverse 不可表示时不安装部分 contribution，以及 Retention 原因原样传播。
末次日志为 `/tmp/novadraw-projection-query-completed-20261106.log`。

本片此前的完整 Core 回归为 530 项通过、7 项既有忽略、0 失败，47.67 s；
日志为 `/tmp/novadraw-projection-query-checks-20261106.log`。
最终消费集合补严后按风险运行上述定向回归，没有再次运行全量 Core。
`check_architecture.sh --update-api` 通过并更新符号快照，Native／Wasm rustdoc、
依赖边界和 Runtime 28／28 方法预算通过；Editor 原有
`ExactProjection<NearestGrid>` 未转义 rustdoc 警告仍保留。
API 日志为 `/tmp/novadraw-projection-api-20261106.log`。

## 命中搜索与事件的坐标失败传播

能力归属：Core FigureTree／Runtime 输入；Editor 仅作消费适配。
2026-11-06 接续用户已授权的受检投影方案。

命中入口返回坐标 Result，递归搜索使用同一受检坐标会话。
进入子节点前采用 projection provider 或未准入容器的实时回调，
不再通过 `.ok()` 或原始 inverse 隐藏投影／数值失败。
完整搜索结束后复核 contribution，搜索策略和 precise_hit 的回调都在此边界内。
有限奇异继续解释为无法逆向进入子域；真正的 reader／预算错误仍返回 Err。

事件使用 owned 输入证据及短期不可变坐标会话，各次定位、投递及最终返回
共享首次值。目标解析全部成功后才更新 cursor／hover／mouse 状态；
处理器结束后检查共享值，合法 effects 按已有协议提交。
Runtime 在结构变更、Clickable 同步及末尾 Activity 后再复核。
DispatchOutcome 保存错误原始 source，失败时 target 为 None、handled 为 false，
capture 使用实际状态；该结果不再 Copy。每次新分发重新建立操作边界。

Editor target、容器定位、选择包装和 ToolContext 传播现有 ViewerError 的坐标原因，
不把 Core 失败当作 Contents 命中。相关结果访问器改为借用。
LayoutError／FramePreparationError 的 Coordinate 包装已实现；
帧准备与布局的实际消费链仍需后续逐项接入，不能据此声称领域验收完成。

11 项独立消费者测试通过，新增搜索回调改值、唯一命中来源、预算拒绝、
事件回调改值与重试恢复、合法 invalidate 提交、末尾 Activity 改值后
保留真实 capture、Layout／Frame source 链验证。
Core／Editor `--lib` check 与 5 项现有搜索测试通过。
完整 Core `--lib` 回归为 531 项通过、7 项既有忽略、0 失败，46.19 s；
日志为 `/tmp/novadraw-projection-hit-core-20261106.log`。
随后 22 项 Core 集成测试和 28 项 Editor 交互／编辑循环测试通过。
初次集成编译补齐取消 Copy 后的期望错误 Clone；数值测试发现父链错误身份
归属发生变化，修复为仍归请求的 Figure，Projection 原始 source 不变。
集成日志为 `/tmp/novadraw-projection-hit-consumers-20261106.log`。
末次 Clippy 修正搜索条件的可折叠分支后，Core／Editor `--lib`
`-D warnings`、独立消费者 Wasm check、公开 API 与依赖门禁均通过；
Runtime 方法预算保持 28／28。既有 Editor grid rustdoc 警告继续保留。
API 日志为 `/tmp/novadraw-projection-hit-api-20261106.log`。
来源原子替换、表现层 local transform、按需 world prefix、整帧输入预算与
最终证明、旧画面 damage、共享模型通知和净成本仍未完成；
总体 FR-02 保持未关闭。

## 来源更新与按需 world prefix

能力归属：Core FigureTree／Runtime 输入。2026-11-06 接续已授权实施；
此节不代表 Runtime 帧消费或总体 FR-02 关闭。

`PreparedFigureUpdate::with_capabilities` 接收完整候选 descriptor 注册表。
提交前检查 projection／container 关联和已有附着契约；拒绝时组件值、
component revision、来源和历史贡献不变。成功提交共用一个 revision，
同步更新所有 typed 成员索引，不在 commit 后重跑注册回调。
显式移除 projection 后才恢复实时 container 来源；重新准入后只消费受检 provider。
child policy 或其他附着领域角色变更仍走子树事务；资源值变化仍声明 resource effects。

world prefix 保存父前缀身份、local placement 与 incoming child-content 边，
先受检消费贡献，再比较当前边，仅复用已完成的矩阵组合。
parent 移动时按实际访问链重建，后代 bounds 不改写。
前缀与 contribution 共用树的逻辑字节留存域；weak 父引用避免深链递归释放。
RootFigure 显式通过公开 projection trait／registry 准入 identity contribution，
没有按 Figure 类型判断或内置快路。
含实时 container 的前缀仍不复用，保持原 bottom-up 回调顺序。

独立查询在整个返回检查通过后安装；Runtime 输入先把贡献与前缀留在 owned
operation 候选中，末尾 Activity 与最终输入复核通过后再安装。
失败操作不覆盖旧查询缓存；当前帧 transaction 尚未接入此边界。

13 项独立消费者已通过，新增非法关联拒绝与旧状态保留、合法替换、
显式移除后的重复实时 callback，以及重新准入的唯一来源。
首轮 Core 全量结果为 533 通过、1 失败、7 项既有忽略：
新增前缀测试误用了未声明纯投影的 Rectangle 容器，因此正确走了保守路径，
测试却错误期待存在前缀。已改用只有 Core placement 的外部 Figure；
没有放宽生产准入条件。修正后的 Core 全量为 534 通过、0 失败、
7 项既有忽略，50.37 s；日志为
`/tmp/novadraw-projection-source-prefix-core-20261106.log`。
Core `--lib` Clippy `-D warnings`、13 项更新后的独立消费者、
Wasm check、Native／Wasm rustdoc 和公开 API／依赖门禁全部通过；
Runtime 方法预算仍为 28／28。既有 Editor grid rustdoc 警告保留。
日志为 `/tmp/novadraw-projection-source-prefix-20261106.log`。
随后补计 warm 前缀的候选表项分配，仅重跑前缀／消费者／Clippy 定向验证；
2 项前缀测试、13 项独立消费者及 Core Clippy 均通过，
日志为 `/tmp/novadraw-projection-source-prefix-final-20261106.log`。

局部表现变换 reader、Runtime 所有实际帧消费／最终证明、唤醒与旧显示 damage，
以及无 tracing 净成本、独立 Perfetto 和混合像素验收仍未完成。

## Runtime 共享投影与领域消费接入

能力归属：Core FigureTree／Runtime；动画、布局、无障碍和损伤读取是 Core
内部消费者。2026-11-06 接续已获授权的受检 child transform 实施。

FigureTree 保存自身的临时 operation slot。RAII scope 只持有 slot 与 owned
operation，不长期借用树；嵌套稳定化、输入分发和坐标会话共享首次输入、
累计预算及未发布候选。panic 或退出清除最外层 slot。
每个不可变消费借用仍独立验证 Core facts，合法 source 几何变更不锁在旧 facts；
共享 input 的首次值和数值错误则贯穿整个操作，忽略 Result 不能清除失败。
派生采用、录制返回和 submission 采用分别复核，只有最外层成功边界发布缓存。

Freeform 的额外映射、动画 GeometryInputs／source capture／表现几何、Viewport
transition factors、无障碍、damage repair 和 scene capture 已采用受检
child-content 边。GeometryInputs 仍去除 insets，patch 按原次序恢复；
动画局部变换继续参与完整矩阵组合，没有用 identity 替代。
Viewport 和两类 ScalablePane 通过同一公开 projection trait／registry 准入，
origin／scale 保存为完整浮点位值，各 registry 的逻辑留存域为 64 KiB。
Freeform／Accessibility／Animation／Zoom 错误链保留坐标原因。
动画源自然失效可以取消，输入退休或预算失败不能按自然失效处理。

录制包装向既有 PresentationSnapshot override 提供准入贡献，递归主循环未改动；
未知 container 保留实时 callback。动画几何缓存另保存其实际消费的准入
child transform，即使 source epoch／sample 未变，也重新检查这一输入。
几何重新物化后以数据身份区分新快照，避免版本相同而漏掉旧、新损伤。
这里维护的是原动画 recorded geometry，不将它改称成功 Presented 证据。

帧失败不消耗 FrameId 或资源同步前缀，恢复录制前脏区和旧无障碍快照，
并请求完整重绘。damage repair 的裁剪空结果仍是合法 None，数值／输入失败返回
领域错误。source 变更前冻结损伤仍保留既有提交语义；冻结读取失败会锁存到下一
帧错误通道，不改走原始 callback，也不能将残余区域当作完整恢复依据。
Prepared／Submitted 通知在该 operation 结束后分发。

首次深树回归发现每条损伤父边都创建查询会话，放大了分配与验证成本；
该次未完成的全量运行已停止，不能算通过。已改为每条父链一个会话，
子树移除共用累计预算，耗尽后停止无效读取；source 退休仍按原协议执行，
后续完整重绘负责恢复像素。

此前增加的两项帧消费者测试已通过，独立消费者总计 15 项：
唯一录制来源、paint 晚期输入冲突、帧身份与旧无障碍状态保留、完整重绘重试，
以及 source 移动前的受检损伤冻结。
随后增加共享预算／树实例隔离／几何提交／panic 退出的 Core 反例，以及动画
缓存共享投影改值但版本不变的第 16 项消费者反例。
首个动画 fixture 只启用普通 transform，没有进入完整几何物化，未请求工作时
返回 Idle 符合既有唤醒合同；已改为互不重叠的 transform／bounds 两组动画，
直接覆盖非冻结祖先的投影重验。bounds 组初次终值不等于已提交值，
被原 `FinalValueMismatch` 门禁拒绝；已修正 fixture 的起点／终点，
没有放宽生产门禁。16 项消费者全部通过，日志为
`/tmp/novadraw-projection-frame-consumer-passed-20261106.log`。

最终 Core `--lib` 回归为 536 项通过、0 失败、7 项既有忽略，9.35 s；
4 项共享 scope／前缀测试和 10,000 层移除的定向验证均通过。
Core 与消费者 `--lib` Clippy `-D warnings`、消费者 Wasm check、
Native／Wasm rustdoc、公开 API／依赖门禁与设计文档检查通过，
Runtime 方法预算保持 28／28。Core 测试仍有 14 条既有 `must_use` 警告，
Editor grid rustdoc 警告仍保留。Core 与边界日志为
`/tmp/novadraw-projection-frame-core-boundary-20261106.log`、
`/tmp/novadraw-projection-frame-{consumer-clippy,wasm,api,docs}-boundary-20261106.log`；
Core Clippy 与深树日志为
`/tmp/novadraw-projection-frame-{clippy,deep}-verified-20261106.log`。
回归运行时长不作为无 tracing 场景的前后性能证据。

随后补强晚期失败反例：实际注册可访问节点，先把共享值从 5 改到 10 并请求帧，
使候选无障碍位置从 8 变为 13；paint 再改到 30，拒绝后必须恢复位置 8。
加强后的 16 项消费者全部通过，日志为
`/tmp/novadraw-projection-frame-consumer-accessibility-20261106.log`。
末次 `git diff --check` 通过，受保护的递归渲染文件没有差异。

成功 Presented 的独立投影证据、既有帧请求后的共享投影工作集与必需通知、
两配置三轮无 tracing 净成本、独立 Perfetto 与混合像素验收仍未完成。
本片不关闭总体 FR-02，不表示录制资产复用、投影索引或 backend 专题已验收。
外部模型仍按已批准合同使用 full redraw／事件失效请求推进，
不新增后台轮询、全局 epoch 或信号 API。

## 稳定投影、录制候选与呈现 damage

能力归属：Core FigureTree／Runtime；2026-11-06 接续同一 FR-02 Execute。
以下为本轮实现范围，验证结果在本节末尾单列；不覆盖前文冻结证据。

Runtime 另存稳定化成功时的完整 child transform 与输入敏感性，独立查询缓存
不能改写这份记录。每次显式推进按准入成员检查当前贡献，source revision 不变的
共享 A → B 也产生 `CoordinateSystemChanged` 和后代 ancestor 通知。
连接进入既有依赖检查／路由收敛，Freeform 进入必要 extent 更新；
其他布局仍按自己的实际输入检查，不因平移统一失效。
新稳定记录在派生采用和投影复核后安装；候选分配与留存使用既有投影预算。
本轮仍逐项检查准入成员，不宣称已消除该分母或所有全树 checkpoint 成本。

动画的旧 damage 改为参照最近匹配的 `Presented`。准备好的帧独立持有
候选几何包络；Ready、捕获、独立查询和错配确认均不能晋升显示旧侧。
Skipped／Retry／失败保留旧侧，只有匹配 session／FrameId 的 Presented 才替换。
包络自身在分配前通过动画 submission 留存预算准入；
容量拒绝发生在消费 FrameId／资源前缀之前。
旧包络已冻结，计算损伤不调用当前 reader 重建它；未知包络请求 Full。
仅供内部测试的 recording helper 也不再充当呈现确认。

一般外部 Figure 尚无完整的冻结视觉包络／clip 资格。本轮为上一呈现帧保存
输入敏感投影身份，只用于判定旧像素是否需要 Full，不将这些身份当作实际
录制矩阵或可复用包络。source 变更／退休触及该前缀时，旧 damage 直接请求 Full，
不重新读取共享模型；新侧仍使用当前受检贡献。身份带 namespace／generation，
随对应 in-flight 持有，匹配 Presented 后才替换并释放旧留存。
这条保守路径可能扩大重绘范围，局部 damage 优化仍需完整包络合同和成本证据。

首次回归发现，对所有 PROJECTION 成员统一使用 Full 会破坏普通内置容器
已有的旧、新区域合并。已收窄为上一呈现帧的输入敏感前缀；
无私有 input proof 的贡献没有新增旧 reader 请求，既有局部 damage 继续保留。
两个外部消费者改为明确要求 Full，原因是其 painter 未声明完整冻结包络，
不再用当前共享值计算出的矩形声称旧画面已受保护。

新增外部反例覆盖独立查询 B 不发通知、组件 revision 保持不变、显式请求发布 B、
跳过显示后以 Full 重试且不重复坐标事实，以及下一次共享变化的必要通知。
source 移动反例另核对变更前冻结不调用 provider。
新增 Core 反例在真正的动画帧中核对 Ready B／错配确认／Skipped B 都保留
Presented A 的损伤包络，匹配 Presented 后才退役 A。

本轮全量 Core 回归为 537 项通过、0 失败、7 项既有忽略，4.56 s；
17 项独立消费者全部通过。Core／消费者 `--lib` Clippy `-D warnings`、
消费者 Wasm check、Native／Wasm rustdoc 及公开 API／依赖门禁通过，
Runtime 方法预算保持 28／28。107 份设计文档检查无错误，
定向 rustfmt 与 `git diff --check` 通过。
上述全量和边界日志为 `/tmp/novadraw-projection-display-final-20261106.log`、
`/tmp/novadraw-projection-display-api-20261106.log`。
随后为稳定输入检查的早期失败补上 full recovery 标记，另做定向复验；
Core Clippy、1 项匹配呈现反例及 17 项消费者均通过，日志为
`/tmp/novadraw-projection-display-recovery-20261106.log`。
该复验结果不替代上述全量记录。

无 tracing 两配置三轮采样、独立 Perfetto、
混合 Native／Headless／Query 像素和未知 overflow 的像素反例仍待验；
不据此宣称净收益、局部录制资产复用或总体 FR-02 完成。
本轮检查时，前文引用的 `target/verification/figure-projection-2026-11-05/`
已不在当前工作区，两个 `/tmp` target 中也未找到 `workset-app`。
后续原基线对照须先恢复产物并核对收据；前文冻结证据不重写，
不能以较慢的中间版本替代原总基线。

## 混合 Headless 像素与路由恢复

2026-11-06 补充验证。能力归属：Core 投影、查询、稳定化与帧恢复；
Vello backend 作为实际像素消费者，未新增 backend 复用能力。
公开接口测试位于
`crates/novadraw-backend-vello/tests/figure_projection_pixels.rs`。

在 Apple M1 Pro／Metal 上，三项 Headless GPU 测试与一项无 GPU 的拒绝反例通过。
纹理为 256 × 128，scale 为 1；比较全部 RGBA 字节，不采用截图容差。
参考场景重新创建 Runtime，以未准入 PROJECTION 的实时 container 计算 group
变换，并使用独立 backend session。嵌套 Viewport 仍采用现行 Core 实现，
因此这是受检 group 与实时 group 的对照，不是所有 Core 缓存的消融实验。
每份成功录制都确认实际包含非空 glyph 输出。

| 反例 | 验证结果 |
|---|---|
| 共享 group 的形状、图片、文字与未知 painter 平移 | A、B 分别与新建实时参考场景逐像素相等；组件 revision 未变，B 的命中及反向坐标正确；独立查询未调用 painter |
| 嵌套 viewport 同时 pan | 与建立 range 后应用相同 pan 的新建参考场景逐像素相等；命中仍使用当前投影 |
| 未知 painter 读取 clip、绘制越界墨迹，并在录制中修改共享模型 | 最终投影检查拒绝 B，纹理仍为 A；未知 callback 的必要调用保留 |
| B 被 Skipped，随后修正为 C | 未提交 B 不修改 A；重试为 Full，C 与新建实时参考场景逐像素相等；旧 shape 和越界墨迹位置清空，warm 重绘保持相同像素 |
| Freeform 极值 child 移动及同一 viewport 链内的跨子树连接 | extent 更新，源端点改变而另一子树的端点在 routing domain 中不变；路由点及全部像素均与新建实时参考相等 |
| 连接端点跨越不同 viewport 链 | 保留 `UnsupportedViewportTopology` 的显式拒绝，不新增跨链路由能力 |

夹具初次编译误将非 const 的 Rectangle 构造用于常量，已修正。
首次 GPU 对照复用了不同 Runtime 的 backend session，并误用了容器默认填充色；
两处均已改正，没有修改引擎行为。跨链路由的首次尝试触及现行拒绝合同，
随后将正向对照限定在同一 viewport 链，另建真正跨链的负向反例。
这些失败不计入通过结果；后续 Clippy 提示固定大小像素切片应使用 `as_chunks`，
已按提示修改，没有抑制警告。

复跑入口：

```sh
cargo test -p novadraw-backend-vello --test figure_projection_pixels -- --include-ignored --nocapture --test-threads=1
cargo clippy -p novadraw-backend-vello --test figure_projection_pixels -- -D warnings
```

GPU 测试默认 ignored，需 native GPU adapter；无 GPU 的拒绝反例默认运行。
通过环境变量 `NOVADRAW_PROJECTION_PIXEL_ARTIFACT_DIR` 指定已有目录时，
测试额外保存五份 RGBA 图像。当前收据、日志及图像在
`target/verification/figure-projection-2026-11-06/`；
`pixel-receipt.json` 保存测试源码、Cargo.lock、测试二进制、日志和原始像素的 SHA-256。
白底 PNG 仅为查看透明图像的预览，原始 RGBA 才是像素对照依据。

验证期间并行工作将依赖从 Vello 0.10／WGPU 29 更新到 Vello 0.11／WGPU 30。
当前测试已适配 WGPU 30 的 mapped-buffer Result；新依赖下四项测试和
定向 Clippy `-D warnings` 均通过，未修改并行依赖迁移的实现。
Cargo 仍提示四个与本目标无关的 egui patch 未被使用；编译器／Clippy 检查未报错误。
新依赖的二进制、测试源码、Cargo.lock 与收据已冻结在上述产物根。
旧依赖的二进制和测试源码与原收据哈希一致，保存在 `vello-010/`，
直接复跑旧二进制的四项测试也通过。五份 RGBA 在两个版本下字节相同；
这只覆盖当前小场景，不代表依赖升级的全量像素验收。
旧版首次运行日志已被复验覆盖，旧 Cargo.lock 只留有哈希；
保留原收据及新生成的旧二进制复跑日志，不宣称能够从旧依赖源码重新构建。

108 份设计文档检查无错误；定向 rustfmt 与 `git diff --check` 通过，
递归渲染主循环没有修改。报告成稿按技术文档结构与中文表达规则复核；
测试范围、依赖版本和未完成门禁均保持上述限制。

这批验证未采集净成本，也没有验证 Native 窗口、Web、设备恢复、
大树规模增长、局部录制资格或命令／glyph 不复制。
原 `workset-app` 与收据仍未恢复；两配置三轮成本和独立 Perfetto 继续待验，
总体 FR-02 保持未关闭。

## 局部录制协议的独立复核入口

2026-11-06 已形成[具体录制协议](../../design/architecture/figure-drawing-recording.md)
和[验证矩阵](../plans/figure-drawing-recording.md)，并接入设计、验证计划及 backlog
导航。两份均为候选，不代表独立复核已通过，也没有接入生产录制。
总体方案要求“此节在录制接入前独立复核”，因此先交付可审阅的具体合同。

候选分别定义 typed provider、完整进入状态、原始命令段与公开容器迁移、
资源／输入首错链和峰值留存。当前 PaintContext 的累计文字统计是可观察效果；
候选首片只给统计增量为零的段授予回调省略资格，产生非零增量时仅调用一次并
保留本次真实输出。平移后的完整进入状态可能 miss，未承诺本片关闭拖动性能门禁。
后续位置无关资格及文字诊断语义分离仍须另行接受。

成稿按技术文档结构检查后，再按好好说话规则复核中文。
事实核对保留总体授权、具体合同待复核、无新增采样和 FR-02 未关闭的状态；
措辞复核未改变 API 名称、失败条件或预算单位。
主线是明确录制省略与命令共享各自的资格，验证矩阵逐项对应该主线。
上述是作者自检，不计为独立架构复核。
`cargo xtask docs` 通过 521 份 Markdown，109 份设计文档无错误；
`git diff --check` 通过。中文字面扫描未发现章节编号、空泛元话语或中文半角标点；
一处类型资格说明改为直接陈述。递归渲染主循环 diff 为空。
本轮仅新增具体候选文档，没有新增性能或录制正确性结论。

## TextFlowLarge 启动预算配置修复

2026-11-06 用户运行 `textflow-large-app --natural-damage` 时，
在 `projection Core receipts` 阶段耗尽默认 2,000,000 allocation。
当前 release 的无窗口复现也在投影阶段失败，错误阶段为 `projection Core record`。
样例已经配置 Runtime preparation 的研究预算，但树的投影预算在构造时独立设置，
仍使用默认值；两项不是同一个预算域。

修复在共享 Large fixture 中增加接收已配置空树的入口，研究应用构造树时显式设置
投影 work／allocation 为 128 × 1024²／64 × 1024²，
contribution 留存上限为 64 × 1024² 逻辑字节。普通官方入口继续使用默认树；
研究应用的初次构造和 Reset 使用同一有界策略，关闭组装复用也生效。
没有提高 Core 默认预算、删除计费或改变投影错误合同。

新增回归测试通过：关闭组装复用、自然 damage，验证首次稳定化、Reset、滚动、
变宽、追加和再次 Reset，并检查文档摘要与当前滚动条坐标。
应用定向 check、Clippy `-D warnings`、release 构建和无窗口一轮回放均通过。
Native／Metal 一轮回放的 13 项操作全部成功，12 个帧得到 Presented；
启动时 1 次 Skipped 正常恢复，另有 1 项无需呈现，无错误或 completion error。
首个成功帧实际含 51,232 个 glyph，截图为正常文字和滚动条，未出现黑帧。
该截图不是完整逐帧像素 oracle，不据此关闭通用投影或长文本的其他门禁。

Native 样本与截图位于 `/tmp/novadraw-projection-startup-native-20261106/`：
`timings-83743.json` 和 `native-1793949312833999000.png`。
无窗口回放使用相同 release 程序且关闭 tracing；Native 回放开启宿主 timings，
本轮没有前后净成本对照，不把预算配置修复记为证明收集性能优化。
文档门禁通过 522 份 Markdown，设计检查无错误；定向格式及 diff 检查通过。
