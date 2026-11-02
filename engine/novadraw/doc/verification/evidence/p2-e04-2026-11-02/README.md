# E04 组合验证与成本证据

类型：`documentation-index`

能力归属：Editor。日期：2026-11-02。

本目录保存 E04 的无窗口采样和编译日志；平台输入验收未在本目录完成。
结论见[实施记录](../../reviews/p2-e04-a-implementation-2026-11-02.md)，
后续设计见[校验成本整改提案](../../../design/editor/snapping-validation-cost-proposal.md)。

## 复现

```bash
cargo test -p novadraw-editor --test p2_e03_container_editing_contract
cargo test --release -p novadraw-editor --test p2_e03_container_editing_contract snapping_cost_samples_separate_cold_hot_feedback_and_full_release -- --ignored --nocapture
```

八次预热、十六次正式样本，N 表示目标 sibling 数，移动源数分别为 1 和 64。
模型总节点数另列，不能把 N 解释为整棵树容量。
大多数 sibling 放在捕获范围外，一个目标 sibling 放在当前锚点附近，
测量普通 Geometry 策略命中、ReferenceLineRenderer 和实际计划。
这是明确分布的合成场景，不代表密集候选最坏情况。

| 输出阶段 | 包含 |
|---|---|
| candidate | E03 当前父坐标与完整源几何计算 |
| solve_actual_plan | provider、prepare 或受检复用、query、仲裁、完整 owner 校验、E03 实际计划与实际几何复验 |
| feedback_cleanup | 应用反馈、参考线渲染、依赖与事实重验、Overlay 发布及清理 |
| full_release | 真实 Tool release、必要同步与 targeting、完整计划、清理／最终检查、Command 与模型投影 |

冷计划每次新建 session，热计划保留 session，但每次仍取得当前候选。
这不是关闭吸附或旧实现的成对基线，不据此宣称提速。
线程 allocator 记录请求次数与字节；它不测复制量、驻留内存或 GPU 成本。
真实 Instant 统计包括版本回调；owner 检查次数与策略回调时间分别记录，
不把 callback 时间当作完整阶段时间。
正式样本运行期间共享工作区还有其他编译活动，P95 不能解释为安静机器基线。

10000 sibling 的 full release 若命中结构化默认 validation 预算错误，
输出 `snap_blocked`，保留零正式样本和未验收状态；
该分支只接受对应错误，其他 release 错误仍使采样测试失败。
采样程序完成不等于这档编辑行为通过。

初次采样的来源预算不足及第二次 Runtime validation 失败分别保留原始日志。
源码快照和 SHA-256 用于识别样本，`environment.txt` 记录工具链与机器型号。
workspace Clippy 日志保留既有 Core 告警；本目录不声称 workspace test 通过。
