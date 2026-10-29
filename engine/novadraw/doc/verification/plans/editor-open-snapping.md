# Editor 开放吸附验证矩阵

类型：`verification-plan`

能力归属：Editor（`novadraw-editor`），验收目标为 GEF 开放吸附行为。
Core 基础协议的使用作为依赖验证；平台输入与应用消费者证据分别归属，
不将 Core 回归通过或应用网格实现计为 Editor 吸附完成。

状态：草案，待评审；2026-10-29。对应
[ADR-058](../../adr/adr-058-open-editor-snapping.md)与
[专题](../../design/editor/open-snapping.md)，没有实施结果。

| ID | 场景 | 必须观察 |
|---|---|---|
| SN-01 | 外部 typed 策略与三个内置策略 | 相同注册、候选、仲裁、反馈与错误通道 |
| SN-02 | 优先级、精确平局、空候选、耦合候选 | 排序确定；每自由度一次修正；耦合整体接受／跳过 |
| SN-03 | 负坐标、网格中点、非法间隔／阈值 | 确定取整；受检参数；不靠 clamp 隐藏非法值 |
| SN-04 | 混合父多选和祖孙选择 | 唯一锚点；全批同一 surface 平移；源子树排除 |
| SN-05 | 嵌套 content pane、zoom／scroll／DPI 1／2 | surface 阈值稳定，目标父约束正确 |
| SN-06 | 无 pointer move 的 auto-expose／zoom／resize | 当前版本重算；release 不使用旧反馈 |
| SN-07 | 目标删除、模型或策略配置变化 | 旧候选失效；失败可观察；无伪命中 |
| SN-08 | resize 固定边、比例和最小尺寸 | 候选遵守自由度，拒绝不改变模型 |
| SN-09 | create／drop、XY→Ordered 换父 | 共用坐标；不误查移出资格；有序策略显式接受／拒绝 |
| SN-10 | 预览、释放、undo／redo | 同一受检结果；一次 Command；完整约束恢复 |
| SN-11 | cancel／capture lost／tool switch 与清理失败 | 无残留参考线；清理拒绝先于 Command 构造 |
| SN-12 | 外部错误、非有限输出、panic | source 保留；整次候选失败；panic 遵循 Viewer fault |
| SN-13 | guide attachment 和网格显示／启用 | 临时反馈不写业务模型；显示独立于启用 |
| SN-14 | N=64／1024／10000，单选／64 项批次 | 记录重建、K、坐标查询、分配、复制、P50／P95 |
| SN-15 | Native／Wasm 可达产物三组对照 | 原始／压缩字节；未消费能力可裁剪；资源列表 |

公开独立消费者证明正常生命周期；故障注入不增加公共测试后门。
Native／Web 必须通过真实 capture、DPI、cancel 和无 pointer move 版本变化，
无窗口 Tool 测试不能替代平台输入验收。
定向 suite 名在实施时使用真实 xtask 入口，不预造 suite ID。
最终 workspace、API、文档和性能证据完成后再提升 `snap.guides` 的采用状态。
