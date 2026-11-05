# 长文本滚动与 resize 修订 4 回应

类型：`verification-record`

能力归属：Core Flow／Runtime／Recorder，Vello backend 栅格与覆盖策略，
platform 输入和呈现证据；Editor 不在范围。
日期：2026-11-04。状态：五项建议已在方案中回应，待复核。

后续状态：用户于 2026-11-04 确认“修订4审核通过，开始执行”；
方案回应已接受，当前进入 FR-02 的共享内容基础迁移。以下文档自审为授权前的
交付记录；它不代替代码、成本和各专题的生产启用验收。

用户要求按[修订 3 评审](long-text-scroll-resize-revision-3-review-2026-11-04.md)
修订。本记录只说明文档变更，不授予设计接受、代码实施或生产启用效力。
原评审正文、引文、哈希与历史基线缺失说明保持；本次未复原或覆盖被审旧稿。

## 逐项回应与复核入口

| 建议 | 修订与依据 | 尚需验证 |
|---|---|---|
| SR3-R01 | [栅格组合条件](../../design/rendering/text-scroll-raster-reuse.md#可见-ir-与纹理缓存的组合条件)：稳定内容资产与本帧可见范围分开；按整块区域查询，成功且绑定有效才登记完整块；外部实现同注册模型 | 四种配置及冷块→同块滚动→跨块→回滚的像素、版本、命中与成本；接口未实现 |
| SR3-R02 | [真实输入分母](../../design/architecture/long-text-scroll-resize-optimization-proposal.md#真实输入的缓存资格分母)：分数位移、惯性、反转；事件与提交分母分开，合格／热／冷／回退及失败均报告，不取整几何 | 真实触控板资格率与包含回退的整体尾延迟；本次未采集 |
| SR3-R03 | [共同覆盖依据](../../design/rendering/text-scroll-raster-reuse.md#共同覆盖依据与两层接入)：provider 静态墨迹与 backend policy 仍由原生覆盖专题维护；冷块先验 backend 依据，Core 另需查询／提交通道；像素定位可先进行 | 两层覆盖与未知回退分别验收；backend 结果不关闭 Core 门禁 |
| SR3-R04 | [版本关联](../../design/architecture/long-text-scroll-resize-optimization-proposal.md#合并与取消后的版本关联)：接收、替代、消费、取消、提交和显示关联；最新输入延迟与最早待处理等待分列；主线程按周期时间并集及连续占用分别统计 | 冻结事件关联、预算和平台时钟；实际显示不可得仍记 unavailable |
| SR3-R05 | [切片门禁](../../design/architecture/long-text-scroll-resize-optimization-proposal.md#切片交付与生产启用)：FR-02 交付 FA-G03、启用 FA-G04；其他工作包分别列交付与启用要求，平台发布采证后置且保留未完成 | 适用正确性、匹配成本和留存仍须通过；下一片不强制达到待审核 resize 目标 |

本次只补齐设计衔接，不新增性能数据或像素通过结论。
FR-02 修订 1 的独立复核仍未完成，B1 扩张继续暂停，生产依赖未切换。
执行顺序只在[P2 backlog](../../roadmap/p2-delta-backlog.md#当前推进顺序)维护。

## 文档自审

按 technical-writing-clarity 核对 owner、前提、失败行为、统计分母和门禁，
再按好好说话检查中文，确认没有改变原指标、授权与证据限制。
这属于作者自审；五项回应不能记为独立复核关闭。

`cargo xtask docs` 检查 486 篇 Markdown 通过，设计检查为零错误；
`git diff --check` 通过。本次未扫描 Rust 实现或运行性能／像素测试，
文档门禁不证明候选协议已实现或性能达标。
