# ADR-059 A：统一文字阶段协议与实例资产登记

类型：`implementation-verification`

日期：2026-11-02。能力归属：Core（`novadraw`）Text。
Editor、backend、platform 无新增能力。
`api_semantics`：`text.flow`、`text.interaction`。

A 的统一阶段协议、独立消费者与定向边界验证已完成。
普通 Parley 和 Outline 引擎仍使用完整布局；Flow 尚未选择阶段调用。
本文验证协议、失败与留存计费，不代表 Parley 宽度复用或性能验收完成。
授权与后续顺序见 [ADR-059 修订 2](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-059-staged-text-width-reuse.md.txt)。

## 实现入口与行为

公开 API 统一放在 `text::shaping`。`TextLayoutEngine::staged_layout` 缺省返回
`None`；外部引擎可以提供相同的 `StagedTextLayoutEngine`。
`MeasureContext::layout_styled_in_session` 使用调用者提供的共享预算和诊断，
由 Core 捕获引擎身份、revision、registry 身份和 typed key。
普通 `layout_styled` 调用保持原入口。

`TextAnalysisRequest` 只从受检的 `StyledTextRequest` 构造，只移除宽度。
所有样式、inline、方向、alignment 和 wrap 均参与精确比较。
会话上下文不能由扩展自行填写绑定；registry 的登记、查询与 typed 读取
均检查该绑定和完整分析输入。

调用先查询有效资产，再在 miss 后询问新增费用下界。
缓存恰好占满字节额度时，已有资产仍进入 reflow，不触发费用询问或 analyze。
零配置额度、显式关闭及缺失能力走完整入口。
下界未知或观测过期不会被解释为不支持；成功缺失只允许一次完整回退，
回退继续使用原会话预算。

每次费用询问、analyze、reflow 的成功、缺失和错误返回均接受绑定与预算检查。
`TextError::Stage` 保存 `TextStageFailure`，主错误按预算、绑定、扩展、Core 校验
的优先级选择，各伴随错误独立保留。
原始扩展错误及其 source 由共享对象持有；`TextError` 的 `Send + Sync` 边界保持，
扩展错误需满足相同约束。错误克隆保持同一对象身份，独立捕获的错误不因文字
相同而相等。

主要源码：

- [阶段协议、会话调用器与复合失败](../../../crates/novadraw/src/text/staged.rs)
- [typed registry、lease 与账本](../../../crates/novadraw/src/text/staged/registry.rs)
- [受检请求投影与存储描述](../../../crates/novadraw/src/text/styled.rs)
- [测量会话入口](../../../crates/novadraw/src/text/system.rs)
- [独立消费者](../../../tests/consumers/text-stages/README.md)

## 留存字节与操作单位

registry 创建者显式提供条目和留存字节额度。索引预留容量、账本、entry、
请求副本、payload 容器、Rc 对齐和计数器、共享 backing 描述与登记表均计字节。
共享 backing 只在相同身份、相同描述下去重。

登记先完成受检计算、会话扣费和 fallible 存储预留，再提交条目与 backing 引用。
容量不足返回未准入；描述冲突、溢出和存储预留失败返回结构化错误。
失败前发生的 work／allocation 不退回。

淘汰只释放缓存引用。旧 lease 继续占条目与字节额度，registry 外壳销毁后，
lease 仍持有账本。最后一个 entry 释放才扣除其 payload 和唯一 backing。
逻辑删除后仍存在的索引、backing 表容量继续收费。
登记使用独立的请求副本，会话 context 的克隆不能延长该副本的留存。

work／allocation 继续使用既有逻辑单位：访问元素计 work，新集合容量项计
allocation，allocation 同时计 work。Core 对请求复制、比较和登记扫描在操作前
扣费；生产者使用 `TextStageContext::charge` 对自己的分析、复制、断行和导出
提供步骤计划。留存字节不直接传入旧 allocation 单位。

生产者负责 payload 的完整容量与共享引用申报。
Core 检查结构、算术、身份、去重和额度，不独立证明任意外部 payload 的真实内存。
账本也不包含 allocator 管理开销、shaping 瞬时峰值或整个 Runtime 的 RSS。
本片没有修改默认 Parley 的存储或注册字体集合。

## 验证范围与证据

独立消费者实现自己的 typed payload、key 和阶段引擎，通过公开 API 调用，
无需 Core 识别消费者类型。该测试引擎使用简单的中性几何，仅用于验证协议；
它不能作为默认文字引擎的 Unicode、glyph 或交互等价证明。

19 项消费者测试覆盖不同宽度、完整回退、零额度、满额有效命中、外国 lease、
错误 typed key、revision 与内容失效、旧 lease 钉住额度、registry 销毁后留存、
共享 backing 去重及描述冲突、输出请求校验、预算失败和计划缺失。
其中一项执行 90 组阶段／绑定变化／返回值／预算组合，
核对主错误、伴随字段、原始 source 和禁止完整回退。

私有事务测试另覆盖存储预留失败、过期下界、容量不足、payload 对齐、
context 克隆与请求释放。存储预留失败使用只在测试编译中存在的注入点，
位置在临时存储准备后、账本提交前；该测试不声称模拟了真实 allocator 的全部
故障方式。

验证使用 Rust 1.96.0，Native 为 macOS arm64，Wasm 为
`wasm32-unknown-unknown`。中间构建放在 `/tmp/novadraw-text-stage-*`。
原始结果归档到 [A 证据目录](../evidence/text-stages-a-2026-11-02/README.md)。

| 检查 | 结果 |
|---|---|
| Core `--lib` 回归 | 445 通过，1 忽略；新增最后一项事务测试另行定向验证 |
| 独立消费者 Native | 默认及关闭默认 feature 各 19 通过 |
| 独立消费者 Clippy | 通过 |
| 私有事务测试 | 5 通过 |
| 既有长文本／styled／外部 Flow／glyph／underline 契约 | 28 通过，1 忽略 |
| Core `--lib` Clippy，`-D warnings` | 通过 |
| 独立消费者 Wasm | 默认及关闭默认 feature 均构建通过 |
| Native／Wasm 公开 API、依赖与签名边界 | 通过；Runtime 28／28，Kurbo 单版本 0.13.1 |
| 文档与差异检查 | 491 份当前 Markdown 校验通过；差异检查通过 |
| 格式 | 本片文件通过；workspace 检查发现并行 E04 吸附文件未格式化 |

Core 单元测试存在既有 dead-code 与 must-use 警告；本片的正式 Core library
Clippy 已通过 `-D warnings`。本报告不把警告或定向测试当作全 workspace 最终验收。
workspace 格式差异仅出现在正在并行开发的 Editor 吸附源码与测试，本片未改动
这些文件；该状态继续留给整批最终门禁。

## 后续边界

本片未取得新的宽度复用性能数据，16 ms／4 ms 目标保持待验收。
`text.flow` 和 `text.interaction` 的已验证语义保持；新的阶段路径全输出等价归 C，
本片不新增编辑功能，也不改既有语义账本状态。

B 仍需解决 Parley 0.7.0 的完整私有 Vec 容量及共享字体 backing 计费接口，
并为 clone、断行、对齐、导出取得操作前计划。
取得经评审的依赖接口后才能进入默认适配；C 的受控 Flow 接入与 D 的匹配性能、
默认启用及限额决策继续按批准的顺序推进。

本报告已按技术文档结构和中文表达做自审，检查了 owner、验证范围、错误主体、
计费责任和“实现／验收”的区分；未把自审记作独立评审。
