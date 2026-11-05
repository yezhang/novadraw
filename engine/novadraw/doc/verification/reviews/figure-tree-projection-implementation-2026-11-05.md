# FigureTree 投影分离实施记录

类型：`verification`

能力归属：Core FigureTree／Runtime 的派生回滚、活动源状态交接与能力成员工作集。
本片不新增 Editor、backend 或 platform 能力。

日期：2026-11-05。状态：活动交接复制删减已实现，定向正确性通过；
成员索引已实现并通过两个身份与失败反例；首片离屏交互未证明稳定净收益，
通用 FigureTree 验收尚未完成。
用户已授权按[总体方案](../../design/architecture/figure-tree-projection-separation.md)
开始实施，仍归入唯一 FR-02 Execute。

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
active 是操作、输入分发与帧 CPU 执行时间之和；frame 是帧执行时间，
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
