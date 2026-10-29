# S08-B Border 受检构造实施记录

类型：`implementation-verification`

本记录对应 [ADR-056](../../adr/adr-056-detached-input-invariants.md)的 S08-B。
Border 构造、builder、Margin 配置与描边复用已实现，公共消费者与本切片验收通过。
S08 的执行状态只在 [P2 backlog](../../roadmap/p2-delta-backlog.md)维护。
Detached Figure、动态指标准入和整批关闭仍归 S08-C／D。

## 构造与冻结边界

Line／Rectangle 以私有 StrokeStyle 保存描边，以自动或显式配置保存 inset 意图。
直接构造和 builder 默认四侧等于宽度；替换描边时自动值跟随，显式值保持。
Margin 只保存受检 Insets，单侧更新也检查完整候选的横纵合计。
Bevel 拒绝零整数宽度，TitleBar padding 拒绝非有限、负值和二倍计算溢出。
Compound 构造读取每个直接 child 的基础 inset 和 preferred 各一次，
检查同一组值及合成运算，不执行 owner 字体测量。

错误从 `figure::border::BorderInputError` 返回，保留字段、side、child role
与 GraphicsInputError 的 source。构造失败不返回可用新对象；
共享外部 child 在构造后改变内部状态，仍须在后续指标准备时重新检查。
S08-B 没有用构造成功替代动态准入。

原 BoundsDrawing 输入要求 Copy，不能直接保存含共享 custom dash 的 StrokeStyle。
公开 `BoundsDrawing::stroked` 接收 Copy 的绘制输入与受检描边，
明确计入 dash 数组和 Arc 元数据。绘制快照克隆共享整个 payload。
普通 Copy 入口保持原限制；内置和外部消费者使用同一描边冻结入口。
RectangleShapeBorder 继续冻结颜色、宽度、圆角和边配置，按 ADR-052 区域填充。

BorderStyle 已删除，Solid／Dash／Dot 由 DashPattern 表达。
历史 DashDot 的可见行为仍按 Dash 迁移，完整 dash-dot 对等项没有因此关闭。
原 RectangleBorder literal 兼容要求由 ADR-056 替换，
ADR-052 的逐角与分边绘制行为保持。

## 契约与公共消费者

对应 [S08 矩阵](../plans/p2-s08-detached-input-invariants.md)：

| 矩阵项 | 本切片证据 | 范围 |
|---|---|---|
| V05 | `p2_s08_border_input_contract` | direct／builder 等价，自动跟随与显式保留 |
| V06 | 同上与 Graphics 回归 | 零、负、非有限、有限派生溢出，Graphics 零宽合法 |
| V07 | 同上 | 每侧非法 inset、横纵合计、单侧更新及二倍 padding |
| V08 | 同上、`scheme_border_contract` | 默认、空组合、optional、空色带和零边距 |
| V11 | 同上 | 构造基础回调次数和不测 owner；挂载后的动态复查待 S08-C |
| V12 | 描边公开录制与既有回归 | Solid／Dash／Dot 和 custom dash；历史 DashDot 映射记录 |
| P05 | 构造失败、挂载值保持与有效替换 | 本切片验证构造拒绝；动态发布拒绝与恢复待 S08-C |
| R01 | 六项 compile-fail doctest | 私有标量、旧 literal、Margin 兼容入口、build alias、BorderStyle、旧直接返回 |
| R02 | Border／Label／圆角／测量／动画契约 | client area、共享标题、边配置、累计 inset 和冻结描边 |
| R03 | Native Graphics 与独立圆角 GPU | DPI 1／2 的离屏像素、分边透明接缝和关闭边；窗口呈现待平台验收 |
| R04 | 独立默认／无默认消费者及 Native／Wasm API | 公开领域路径，未增加 feature 或依赖 |
| R05、R06 | 下节成本与产物记录 | CPU Border 探针与可达消费者；完整帧成本待 S08-D |

[定向 Core 门禁](../evidence/p2-s08-b-core-contracts-2026-10-29.log)：
Core lib 428 项、圆角 18 项、Label 13 项、可复用 Border 16 项、
动画几何 41 项、准备测量 18 项、新 Border 10 项、Scheme 2 项，共 546 项通过，
2 项忽略。冻结绘制单测检查 custom dash 存储计费和 payload 共享。

独立 `tests/consumers/style-input` 在
[默认最终复测](../evidence/p2-s08-b-external-default-final-2026-10-29.log)与
[无默认最终复测](../evidence/p2-s08-b-external-off-final-2026-10-29.log)配置下各通过 15 项，
其中 10 项为 Border 契约，5 项为 S08-A 样式契约。
新契约只导入公开领域接口，不使用 Core 私有字段或 concrete 准入分支。
原有限 inset 合计溢出的准备阶段反例改用外部 Border，
继续证明错误先于测量回调发生，没有改成内置构造反例。

[第三方冻结描边执行](../evidence/p2-s08-b-external-freezing-execution-2026-10-29.log)
通过实际 Runtime 动画提交，检查采样后的 60×50 几何与原 custom dash 保持。
原独立入口创建检查已扩展为执行检查；测试数量保持不变。
该 target 的[严格 Clippy](../evidence/p2-s08-b-border-clippy-2026-10-29.log)及
[独立消费者 Clippy](../evidence/p2-s08-b-external-clippy-2026-10-29.log)通过。

[API 检查](../evidence/p2-s08-b-api-2026-10-29.log)通过 Native／Wasm rustdoc
与依赖边界，Runtime 方法仍为 28／28，Kurbo 仍为 0.13.1。
该快照检查领域公开符号，构造返回值与私有字段由消费者和 compile-fail 验证。

最终 `cargo fmt --all -- --check`、
[workspace Clippy](../evidence/p2-s08-b-workspace-clippy-2026-10-29.log)和
[workspace tests](../evidence/p2-s08-b-workspace-tests-2026-10-29.log)通过：
171 个目标／doctest 汇总，1,686 项通过、0 失败、21 忽略，含六项新增 compile-fail。
全量门禁之后仅加强外部冻结描边的测试体，再运行该 target 和两种独立消费者；
没有再次扫描无关目标。
[最终文档检查](../evidence/p2-s08-b-docs-final-2026-10-29.log)通过。

[Graphics 离屏 GPU](../evidence/p2-s08-b-native-gpu-2026-10-29.log)通过 DPI 1／2
像素断言，`surface present: false`。
[独立圆角／分边 GPU](../evidence/p2-s08-b-border-gpu-2026-10-29.log)通过半透明接缝、
关闭边和 Border 快照像素回归。窗口呈现、浏览器和其他平台结果保持待验收。

## 成本与产物

迁移前已保存 Native／Wasm、默认／无默认的同配置 release 产物。
配置为 opt-level=s、LTO、单 codegen unit、abort panic 和 strip；
Native 使用 Rust 1.96.0，Wasm 使用 stable Rust 1.99.1。
五轮初始基线见
[迁移前成本](../evidence/p2-s08-b-cost-before-2026-10-29.log)。

每种 Border 模式预热一次，再运行 101 个样本，每个样本 120 次操作。
模式依次为嵌套构造、基础指标查询、固定尺寸录制、连续宽度变化录制。
冻结绘制在后三种模式每个样本构造一次。记录 allocator 请求次数和请求字节，
包含构造与 teardown；固定尺寸和 resize 模式只测 CPU 录制。
完整 Runtime 稳定帧、缓存命中、输入响应和 GPU 帧延迟待 S08-D。

编译与测试结束后，默认和无默认配置各做十轮交替复测：
偶数轮先旧后新，奇数轮先新后旧。全部样本与 SHA-256 见
[配对原始记录](../evidence/p2-s08-b-cost-paired-2026-10-29.log)及
[产物收据](../evidence/p2-s08-b-artifacts-2026-10-29.json)。
下表为每轮 P50／P95 的中位数，单位 ns。

| 配置／模式 | 迁移前 P50／P95 | 迁移后 P50／P95 |
|---|---|---|
| 默认／构造 | 24,791.5／25,625.5 | 31,000／32,333.5 |
| 默认／指标 | 2,812／3,437.5 | 2,875.5／3,458.5 |
| 默认／固定尺寸录制 | 98,917／102,604 | 98,583.5／100,917 |
| 默认／宽度变化录制 | 99,854／103,500 | 98,437.5／100,812.5 |
| 默认／样式对照 | 151,583／157,542 | 150,479／156,895.5 |
| 无默认／构造 | 23,499.5／25,520.5 | 33,062.5／35,416.5 |
| 无默认／指标 | 2,687／3,438 | 3,062.5／3,250 |
| 无默认／固定尺寸录制 | 99,916.5／103,437.5 | 97,895.5／101,916.5 |
| 无默认／宽度变化录制 | 100,125／103,062.5 | 98,646／100,458.5 |
| 无默认／样式对照 | 149,271／155,020.5 | 151,479／159,042 |

各模式的 allocator 次数前后相同。构造模式总请求字节由 4,072,320 增至
4,848,000，增加 19.05%，相当于每组两条 Line、两层 Compound 和一条 Margin
多 64 字节。指标与录制模式每批 101 个样本多 8,080 字节，
即每次构造嵌套对象与冻结快照多 80 字节；后续 120 次操作没有新增分配次数。
样式对照的 224,422 次分配和 54,632,112 请求字节前后相同，
不替代 S08-A 与旧字体文本实现的成对比较。

默认构造模式 P95 增加 26.18%，每组约增加 56 ns。
新增基础指标检查与更大的受检配置只发生在构造阶段；
指标与录制的中位数接近原值，尾延迟范围仍重叠，
不能据此宣称整体帧性能提升。完整帧与 custom dash 复制成本继续由 S08-D 验证。

| 产物 | 迁移前原始／gzip 字节 | 迁移后原始／gzip 字节 |
|---|---|---|
| Native 默认 | 1,959,328／953,281 | 1,975,872／954,692 |
| Native 无默认 | 1,959,328／953,117 | 1,975,856／954,577 |
| Wasm 默认 | 2,136,643／713,269 | 2,142,010／715,325 |
| Wasm 无默认 | 2,136,663／713,294 | 2,142,026／715,315 |

Native 默认 gzip 增加 1,411 字节，Wasm 默认 gzip 增加 2,056 字节。
没有新增依赖、crate、默认 feature 或内置字体；当前产物只包含消费者可达代码，
不包含 Native 窗口、GPU 宿主或完整产品的首次加载资源。

## 后续

S08-C 继续收口同类 Detached Figure 的数值入口与 Bounded 兼容 setter，
并完成初始值单次捕获、外部 Border 动态准入、组合／owner 溢出及失败后恢复。
S08-D 再完成完整场景成本和整批文档关闭。当前记录不关闭 R03、TC-03、
API-F04 或 API-10，也不把应用侧网格吸附或分组计为 Editor framework 能力。

成稿复核保留接口、命令、数字与验收范围，检查事实、措辞和阅读顺序。
机械扫描的章节编号、对比句模板、空泛元话语与中文正文半角标点均为零，
没有豁免项，原始计数见[成稿扫描](../evidence/p2-s08-b-chinese-review-2026-10-29.log)。
