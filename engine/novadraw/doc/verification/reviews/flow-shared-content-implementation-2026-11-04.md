# FR-02 共享内容基础迁移

类型：`verification-record`

能力归属：Core Text 的不可变内容／request 与阶段分析留存。
日期：2026-11-04。状态：共享内容基础迁移已完成，FR-02 整批未完成。
用户已通过滚动方案修订 4 并授权执行；本记录只覆盖 FR-02 的共享内容基础步。

## 交付边界

`StyledTextContent` 共享文本、默认样式和 spans，构造时检查 span 顺序、范围及
UTF-8 边界。`StyledTextRequest::from_content` 组合当前 boxes 和 constraints，
两者仍逐次校验；旧 owned 构造和 engine 读取方法保持。
request clone 和 `TextAnalysisRequest` 保留同一内容 lease，只复制 request／box
metadata。相同 backing 以受检内容句柄识别，内容相等仍比较完整值，不按散列命中。

`TextContentStore` 的 clone 共用容量配置和留存账本。新增 retained record 前准入，
拒绝不产出内容；缩容只拒绝正 charge 新增，旧合法 lease 不变，最后 lease 释放扣账。
owned 兼容入口的独立无上限 owner 直接由内容 record 承载，无额外可变账本；
受限 store 显式持有账本。内容用于阶段分析时，原域 lease 保留，分析 registry
在自己的责任域将其登记为 shared backing，多个 entries 不重复收费同一内容。
这不是全 Runtime 内存上限，也尚不覆盖 assembly、proof 和服务家族 cache。

容量、溢出及 poison 错误通过 `TextError::Content` 保留结构化 source。
公开 `TextError` 增加 variant，使用穷尽匹配的外部调用方需要补分支；
构造器与读取方法的源代码兼容不代表 enum 穷尽匹配自动兼容。
新符号已登记公开 API 快照，独立 crate 位于
[flow-assembly-reuse](../../../tests/consumers/flow-assembly-reuse/README.md)。

本步没有省略 contributor／handler／border，没有缓存 inline 的测量值。
Flow 仍按现行合同组装贡献；声明、整页资产、测量回执和 Runtime 最终证明链是
后续交付范围，不能据本步关闭 FA-G03／G04 或声明滚动卡顿已经解决。

## 验证入口与实际范围

`flow_assembly_reuse_contract` 覆盖内容 backing／字体名／spans 共享、request／analysis
clone、缩容与旧 lease、非法输入、当前 boxes／constraints，以及 owned／shared 布局等价。
独立消费者使用同一测试夹具，只通过公开路径接入。
`p2_text_stages` 增补两个不同分析输入共享同一内容的唯一计费反例。
现有 styled、完整 Flow、真实 inline／边框及外部 engine 回归继续执行。

另修复 `textflow-large-app` tracing 测试的 Config 初始化缺少六个字段；
补齐关闭值，保持原 SDK 测试场景。

Core `--lib` 为 447 passed／1 ignored；新内容合同 5 项通过，
既有 styled 15 项、Flow／inline／border／外部引擎 30 项和 Parley 操作 5 项通过。
独立消费者首次 5 项通过，其 Native／Wasm 编译通过；消费者与成本探针 Clippy 通过。
Core Clippy（`--lib -D warnings`）、SDK 夹具 1 项及独立消费者续验 5 项通过。
架构／公开 API 门禁通过 Native 与 Wasm，Runtime 方法 28／28 保持；
检查仍报告既有 Harfrust qualification 与 Editor rustdoc HTML 警告，未混入本片修改。
本批 Rust 定向 fmt 和 `git diff --check` 通过，文档门禁检查 487 篇 Markdown 通过。

阶段故障矩阵首次续验出现 1 项失败：夹具在注入预算耗尽后才调用 Core 下界构造，
新增的受计费 backing 扫描因此返回预算错误，把预期 Value 分支变成了 Error。
夹具现改为先构造受检返回值，再注入耗尽，保留三类返回值的绑定／预算检查。
生产下界仍按实际扫描收费，未放宽粘滞失败或删除原 source 断言；
修复后 `p2_text_stages` 的 20 项全部通过，包括三类返回值的完整故障矩阵。

## 成本证据范围

独立消费者的 `content-clone-cost` 在同一 release 进程交替比较此前 request 存储形状的
derived clone 和新 request clone；参考形状为共享文本、owned 默认样式和 Vec spans。
两组输入分别为 1280 字节／1 span、51232 字节／40 spans，内容和 spans 完整相等。
每模式 400 个样本，每样本 64 次 clone；统计包含 clone 及释放。
这是调用级存储对照，不是旧引擎二进制、原 Large 更新或窗口性能基线。
构造、完整比较、组装、布局、GPU 和 present 不在其分母。

原始 CSV 放在 `target/verification/text-fr02-2026-11-04/content-clone-cost.csv`；
其结果仅用于检查 clone 成本，不提前判定 FR-02 净收益和 cold／不合格回退。
整批性能仍按验收矩阵重新冻结实施前后配置。

Rust 1.99.1、macOS 14.7.8、release、offline／locked；两种 clone 在同一进程按样本
交替执行。下表单位为 ns／clone，分位数来自每组 64 次操作的平均值。

| 内容／span | 存储形状 | P50 | P95 | P99 |
|---|---|---:|---:|---:|
| 1280 字节／1 | 旧存储 clone 参考 | 123.69 | 131.52 | 968.09 |
| 1280 字节／1 | shared request clone | 10.42 | 11.72 | 13.67 |
| 51232 字节／40 | 旧存储 clone 参考 | 2374.36 | 4064.45 | 11216.16 |
| 51232 字节／40 | shared request clone | 10.42 | 12.38 | 13.67 |

结果支持内容 clone 不随文本／spans 规模增长；后台编译和负载未受控，
尾延迟仅作本次采样记录，不推算原 Large 的净提速。
`source-receipt.json` 留存源码／锁文件散列、HEAD 和工具链，已回读确认与实施源码一致。

## 后续交付

下一步冻结贡献声明、typed 输入域和测量／准备 metadata 的具体接口，再接通最终
发布检查和整页组装资产。当前 `PreparationMeasurement` 仍为 Copy 尺寸值，
测量 cache 仅保留既有 observations；新声明证明不能沿这条路径丢弃。
`graph/presentation.rs` 的候选收集、最终检查和安装顺序须与这些测量依赖一起核对。
源／注册变化、真实 inline、边框资格、服务家族留存和匹配端到端成本继续按矩阵验收。
