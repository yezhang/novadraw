# 文字阶段协议与默认接入验证状态

类型：`implementation-verification`

更新：2026-11-05。能力归属：Core Text／Flow、默认上游文字适配器；
backend、platform、Editor 无新增能力。
本页归并 ADR-059 评审和 B1 子片记录，不新增授权或修改预算合同。

## 当前状态

2026-11-05 用户授权迁移到未打补丁的 Parley 0.12.0，旧三份依赖源码、
35 份补丁和专用消费者／重放工具已退役。最新验证见
[上游迁移记录](parley-upstream-migration-2026-11-05.md)。
下表 B 各子片是退役前证据，不再代表生产依赖或可执行入口；
Core A 的通用阶段协议与外部引擎预算合同保留。
默认 Parley 尚未实现严格内部预算和留存容量登记，C／D 仍未完成。

ADR-059 修订 2 于 2026-11-02 复核通过、由用户接受并批准实施；
资产先查询、新增工作再准入，原始 source 和复合失败规则已明确。
合同见[ADR-059](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-059-staged-text-width-reuse.md.txt)，
旧依赖范围见[分析准入方案](../../design/architecture/text-analysis-admission-proposal.md)。

| 阶段 | 已有验证／剩余边界 |
|---|---|
| A | 统一阶段协议、实例资产登记、预算拒绝、淘汰和外部消费者已验证；[记录](text-stages-a-implementation-2026-11-02.md) |
| B 存储／操作 | 补丁已退役；历史模板复制、对齐及候选断行准入证据见[存储](text-stages-b-storage-patch-2026-11-02.md)、[操作](text-stages-b-operations-2026-11-02.md)、[断行](text-stages-b-line-breaking-2026-11-02.md) |
| B1 分析／shaping | 已实施下表所列受限范围，扩张于 2026-11-03 按用户指示暂停；完整 mark、复杂 shaping 和其他未覆盖路径未完成 |
| B2～B4 | Fontique family 匹配及私有 collection 的注册内存字体查询准入已验证；Parley 样式分段、边界分析和字体栈内部准入已通过定向测试；完整编排、导出计费与资产适配未完成，见[第二批采用记录](draw2d-text-followup-2026-11-05.md#parley-分析前段的准入子片) |
| C／D | Flow 阶段接入、完整输出和性能验收未实施，生产缓存未默认启用 |

## B1 退役前已验证范围

三十份补丁精确重放。以下是已实施子片的范围，不宣称任意字体和脚本均已准入。

| 工作组 | 已覆盖内容 | 明确保留的边界 |
|---|---|---|
| 输入／实例 | buffer、规范化坐标、条件选择、无 avar／avar1 的用户坐标 | avar2 等未覆盖路径仍拒绝 |
| 冷缓存／计划 | 目录、cmap、mark digest、lookup／子表、空 AAT 表头、ShapePlan、GSUB／GPOS 计划、私有与通用语言、脚本规划回调 | 非空 AAT、完整复杂计划和实际 shaping 不由表头或规划通过推定 |
| GSUB | 受限单／多字形替换、连字、上下文匹配和 spill／backup | 各片保留 lookup flags、格式和叶操作条件，不视为通用 GSUB 完成 |
| GPOS／方向 | 受限 SinglePos／PairPos、ASCII RTL、原生方向反转和镜像查询 | mark 属性／定位、其他复杂定位未完成 |
| 字符与规范化 | 已映射 Unicode、GDEF 属性、一般 scalar、singleton、空格／U+2011 fallback、分解后缺字 | 原始 singleton 证明和公共输入限制仍适用 |
| 共享内核 | 分解／排序／重组、dotted-circle 输出／同步、grapheme 合并和反向分组 | 内部内核通过不表示公共组合输入已开放或 Flow 已接入 |

每个相关操作在访问、增长或复制前准入；拒绝保持已有 buffer 恢复合同，
不通过普通入口重做来绕过粘滞预算。具体 fixture、费用和拒绝点保存在原子片快照。

## 退役前验证分母与性能限制

最终 grapheme 子片的二十二矩阵、四配置共 10,786,752 份完整观察一致：
原版 44 份配置核对后复用，生产 44 份重新执行。
Native／portable 各 218 项内部测试、102 项消费者测试通过；
Core 长文本布局、绘制、共享 glyph 和阶段协议定向回归 32 项通过。
不把不同子片的累计矩阵或重复测试数量相加。

768,000 条成本样本、七份采样前 Clippy 收据和十九份原始采样文件核对完成，
十一项边界检查通过；库 Clippy 新增诊断为零，既有诊断保留。
Wasm 只构建，未执行测试。普通路径个别配置 P95 比值达到 3.409，
外部负载未受控，不能证明没有性能回退。
这些结果不关闭 16 ms 更新／4 ms 查询或默认配置性能门禁。

约 2 ms 的[隔离 reflow](text-width-reuse-feasibility-2026-11-02.md)
没有包含生产 Flow 全链路。FR-01 已完成局部输入复用验证，
下一步优先 FR-02 贡献组装合同；恢复 B1 需先明确端到端收益与新顺序。
唯一执行状态见[P2 backlog](../../roadmap/p2-delta-backlog.md#当前推进顺序)，
真实负载结果见[文字优化验证](../performance/text-optimization-validation.md)。

## 原始依据

三十份 B1 子片和五份 ADR-059 评审／回应退出当前正文，原始报告、数字、
fixture、命令和限制保存在[维护前快照](../../archive/knowledge-maintenance-2026-11-03.md)。
文本原始日志和成本样本仍在 `target/verification/text-*/`，不纳入 Git。
