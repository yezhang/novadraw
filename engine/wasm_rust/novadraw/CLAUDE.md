# Novadraw 图形工具包

## 项目概述

使用 Rust + WebGPU 技术栈实现的高性能绘图引擎工具包，参考 eclipse draw2d/GEF 架构设计，目标是扩展为通用图形框架。

### 技术栈

- **渲染后端**: vello (WebGPU)
- **窗口/事件**: winit
- **文本渲染**: cosmic-text
- **构建工具**: cargo

### 模块结构

```text
novadraw-core/     - 核心数据类型
novadraw-math/     - 数学运算
novadraw-geometry/ - 几何计算
novadraw-render/   - 渲染抽象
novadraw-scene/    - 场景图、Figure
apps/editor/       - 编辑器示例
```

## 核心禁止事项

- **递归深度限制**：树遍历允许使用递归，深度上限 10,000 层；性能专项开始前不得重新引入迭代渲染主线
- **禁止临时方案**：问题必须从根因解决
- **禁止全局状态**：不使用 Singleton
- **禁止热路径日志**：渲染循环中不打印日志
- **禁止 magic numbers**：业务代码中不使用硬编码数字
- **迭代渲染归档**：`render_iterative.rs`、`use_iterative_render` 和 I 键切换已从主线移除；历史 POC 归档在 git tag `archive/render-iterative-poc-20260617`
- **保护渲染主循环**：`novadraw-scene/src/graph/render_recursive.rs` 是当前 Draw2D 渲染主流程承载点，通常不应修改主循环逻辑；除非已对标 draw2d 证明当前主流程与 draw2d 不符，否则问题应优先定位到 Figure 协议、坐标转换、NdCanvas 命令、Vello 后端或调用路径
- **禁止提前恢复迭代渲染**：递归渲染在 M1-M10 核心契约完备前，不得把迭代渲染重新接入代码主线；未来仅能作为性能专项 delta 从归档 tag 恢复并重新设计
- **通用机制下沉引擎层**：事件分发、坐标转换、target/source Figure 事件点适配、通用上下文等机制必须位于引擎 crate（如 `novadraw-scene`），`apps/*` 只做平台输入适配与示例编排

## 提交前检查

```bash
cargo fmt --check && cargo check && cargo clippy -- -D warnings && cargo test
```

## Git 提交规范

- 提交信息摘要必须使用中文。
- 提交应按主题拆分，保持原子化，避免把无关改动混入同一提交。

## Skills 与文档

### 可用 Skills

- `/analyzing-gef-code` - 分析 draw2d/GEF 源码
- `/analyzing-xilem-code` - 参考 xilem GUI 框架
- `/analyzing-swt-code` - 分析 SWT GC 底层 API
- `/novadraw-impl` - 项目实现指导
- `/code-review` - 代码审查
- `/simplify` - 代码简化审查

### 详细文档

完整文档见 [doc/00-index.md](doc/00-index.md)。

| 场景 | 文档 |
|------|------|
| Figure 开发 | [doc/02-figure/](doc/02-figure/) |
| 渲染管线 | [doc/03-rendering/](doc/03-rendering/) |
| 架构设计 | [doc/01-architecture/](doc/01-architecture/) |
| 坐标系 | [doc/04-coordinates/](doc/04-coordinates/) |
| 路线图（产品视图 + Demo 矩阵） | [doc/06-roadmap/](doc/06-roadmap/) |

### Milestone 与路线图

- **Draw2D API 语义覆盖账本**：`doc/01-architecture/draw2d_api_coverage.md`（API family、语义契约、Novadraw 合理变体、M1-M10 映射）
- **产品交付清单**：`doc/06-roadmap/product-deliverables.md`（5 图元 / 6 布局 / 6 边框 等策略层清单）
- **Demo + 验证矩阵**：`doc/06-roadmap/demo-matrix.md`（每个 milestone 配套 demo + 截图断言策略 + GEF 层探索附录）

> 任何 delta、PR、commit、文档引用 `M{n}` 一律指 YAML M{n}，禁止在其他文档发明独立编号。
> 节点编辑器等 GEF 层能力不在 draw2d 核心 milestone 内，详见 `demo-matrix.md` 附录 A。
> 推进 M1-M10 的 architecture/parity delta 时，必须检查对应 `api_semantics` 的 Draw2D API 语义是否完整；受影响 family ID 记录在 delta 的 `api_semantics` 字段，语义来源见 `draw2d_api_coverage.md`。

## 架构设计原则

### 基本准则

架构设计时遵循以下优先级：

1. **扩展性 > 稳定性 > 性能**：优先保证架构可扩展，其次是稳定可靠，最后才考虑性能
2. **参考 g2 设计理念**：draw2d/GEF 经过 20+ 年生产验证，核心设计决策优先对标 g2
3. **不考虑当前代码状态**：架构讨论独立于实现，代码应追随架构而非反之
4. **接口 vs 实现分离**：核心抽象必须是接口（trait），具体实现可替换
5. **引擎层承载通用机制**：draw2d 风格的事件链路、坐标域切换和 Figure 回调上下文属于引擎语义，不属于应用层便利逻辑

### 架构设计执行规则

**在做理想架构设计时，禁止扫描本项目的 rust 代码。**

- 架构设计应从需求、第一性原理、g2/GEF 参考文档出发
- 可以参考 g2 源码（`/Users/bytedance/Documents/code/GitHub/gef-classic`）和已有分析文档
- 不扫描本项目代码是为了避免"现状偏差"，确保架构设计独立于当前实现

### g2 设计哲学

draw2d/GEF 的核心设计哲学：

| 原则 | 说明 | Novadraw 对应 |
|------|------|---------------|
| **接口内聚** | IFigure 接口保持内聚，避免上帝接口 | dyn Figure trait |
| **状态分离** | Figure 只有渲染状态，FigureBlock 持有运行时状态 | Figure/FigureBlock 分离 |
| **委托优于继承** | 布局、更新等通过组合实现 | LayoutManager trait |
| **两阶段更新** | Validation → Damage Repair 分离 | UpdateManager 两阶段 |
| **ID 引用树** | 树节点通过 ID 而非引用访问 | SlotMap<BlockId, T> |

### 架构审查清单

讨论架构时，必须明确：
- [ ] 扩展点在哪里？（新 Figure 类型、新 LayoutManager、新平台后端）
- [ ] 稳定性：接口是否稳定？会不会频繁 breaking change？
- [ ] 与 g2 的对应关系？为何与 g2 不同或相同？
- [ ] 错误处理：失败模式是什么？

## 交互方式

### 思维原则

运用第一性原理思考，拒绝经验主义和路径盲从。不要假设我完全清楚目标，保持审慎。若目标模糊请停下讨论，若目标清晰但路径非最优，请直接建议更短、更低成本的办法。可以采用苏格拉底提问法和奥卡姆剃刀原理。


### 回答格式

所有回答分为两部分：

- **直接执行**：按要求直接给出任务结果
- **深度交互**：审慎挑战需求动机、XY问题、路径弊端，给出替代方案

### 行为准则

- **重大架构变更**：先讨论方案，确认后再实现
- **Bug 修复**：先理解根因，解释后再修复
- **代码改动**：超过 50 行分步提交
- **新增功能**：先定义接口，再迭代实现
- **性能优化**：提供基准数据佐证

## 调试技巧

- 使用 `--screenshot` 参数保存渲染结果
- 使用 `mcp__MiniMax__understand_image` 分析截图
- `render_iterative.rs` 已归档；除非执行性能专项恢复，否则不要把归档代码纳入当前实现分析
- 背景色 RGB(238, 238, 238)，图形颜色应避免与此重复

## 参考代码

- draw2d/GEF: `/Users/bytedance/Documents/code/GitHub/gef-classic`
- SWT GC: `/Users/bytedance/Documents/code/GitHub/eclipse.platform.swt`
- vello: `/Users/bytedance/Documents/code/GitHub/vello`
- xilem: `/Users/bytedance/Documents/code/GitHub/xilem`
- Zed: `/Users/bytedance/Documents/code/GitHub/zed`
