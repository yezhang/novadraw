# R8 清理、性能与扩展验证执行计划

类型：`migration-guide`

本文细化
[`architecture-refactor-plan.md`](architecture-refactor-plan.md)
中的 R8。目标是删除迁移期兼容名称、建立可复查的性能基线，并验证二维核心与
2.5D/Scene3D 扩展边界。本文不改变 `design/` 中的规范架构。

## 1. 前置条件与不变量

R7 已通过 macOS、Web 和 Headless 三类引擎迁移验证。Windows/Linux 平台资格验证
延期到对应跨平台应用开发或发布阶段，不阻塞 R8。

R8 必须保持：

- `FigureId` 是 Runtime-local 的代际节点身份；
- `FigureNode` 保存通用节点状态与具体 Figure；
- `FigureTree` 只管理 arena、拓扑、顺序和树查询；
- `Runtime` 继续作为交互、更新和 mutation 的事务边界；
- 递归树深度上限保持 10,000；
- 不恢复归档的迭代渲染主线；
- 不把 DisplayList proposal、2.5D 或 Scene3D 塞入当前二维核心协议。

## 2. 当前影响面

R8 启动时盘点结果：

| 旧名称 | Rust 引用数 | 目标名称 |
|---|---:|---|
| `BlockId` | 511 | `FigureId` |
| `FigureBlock` | 12 | `FigureNode` |
| `FigureGraph` | 405 | `FigureTree` |

引用分布在 52 个 Rust 文件中。启动时实现关系为：

```text
BlockId    <- canonical definition
FigureId   <- compatibility alias

FigureNode <- canonical definition
FigureBlock <- compatibility alias

FigureGraph <- canonical definition
FigureTree  <- compatibility alias
```

R8 必须把 canonical 定义统一为 `FigureId / FigureNode / FigureTree`，最终删除三个
旧名称，不保留双向别名。

## 3. 批次与提交边界

### R8.0 迁移前基线

状态：`completed`

工作：

- 记录工具链、平台、构建模式和提交；
- 运行现有 `stress_1024` 并保留 release 数据；
- 建立大树、深树、文本和 viewport 四类可重复测量入口；
- 性能结果写入 `target/` 报告，文档只记录稳定基线和命令。

门禁：

- 测量入口不进入渲染热路径日志；
- 每项至少预热一次并重复测量；
- 后续批次使用相同机器、工具链和 release 配置比较。

提交主题：`验证：建立 R8 迁移前性能基线`

基线记录：

- [`../verification/performance/r8-baseline-2026-09-02.md`](../verification/performance/r8-baseline-2026-09-02.md)

### R8.1 Canonical 定义反转

状态：`completed`

工作：

- SlotMap key 的真实定义改为 `FigureId`；
- 树结构真实定义改为 `FigureTree`；
- 节点真实定义保持 `FigureNode`；
- 引擎 crate 内部字段、签名、渲染引用和 Runtime 先迁移到目标名称；
- 旧名称只在批次结束前作为受控兼容别名存在。

门禁：

- `novadraw-scene` 单元测试与契约测试通过；
- crate public re-export 以目标名称为主；
- 不改变布局、事件、坐标、damage 或 mutation 行为。

提交主题：`重构：确立 FigureTree canonical 类型`

### R8.2 调用方迁移与旧别名删除

状态：`completed`

按所有权边界分三次迁移：

1. `novadraw-scene` 测试与内部模块；
2. `novadraw`、`novadraw-apps` 与应用；
3. parity、verification、roadmap 和示例文档。

每次迁移后运行定向测试。全部调用方完成后删除：

```text
BlockId
FigureBlock
FigureGraph
```

最终门禁：

```bash
rg -n '\b(BlockId|FigureBlock|FigureGraph)\b' \
  novadraw novadraw-* apps doc \
  --glob '*.rs' --glob '*.md'
```

结果必须只包含明确说明旧名称已删除的 R8 历史记录，不得存在 public API、示例或
活跃设计引用。

提交主题按模块拆分，最后一项为：`重构：删除旧 Figure 兼容名称`

完成记录：

- `9c18073`：迁移聚合 crate、应用公共库与应用调用方；
- `98213f3`：迁移 parity、verification 与参考文档中的活跃术语；
- `a845736`：迁移 `novadraw-scene` 契约测试；
- `262dd90`：删除三个兼容名称，并将渲染只读视图改名为
  `FigureTreeRenderRef`；
- workspace fmt、check、Clippy `-D warnings` 与全量测试通过。

### R8.3 Context、坐标与失效文档清理

状态：`completed`

工作：

- 删除已经没有调用方的旧 context 和旧坐标模式；
- 核对 `api-coverage.md` 中受影响 family；
- 当前设计文档统一使用 target 名称；
- 仅有历史价值的内容移动到 `archive/`。

门禁：

- 坐标往返、coordinate root、hit-test 和 target-domain 测试通过；
- `doc/00-index.md` 不链接失效文档；
- 不通过修改规范文档来掩盖实现偏差。

完成记录：

- `b9fb878`：删除 `SceneUpdateManager` alias 和一次性旧 bounds 迁移入口，修正
  parent-local 坐标映射，并归档旧执行清单与 bounds 迁移计划；
- `afcc7f5`：将 Figure callback context 收敛为具体 `EventContext<'a>`，删除
  `NovadrawContext` / `SceneNovadrawContext` 双层旧 API；
- workspace fmt、check、Clippy `-D warnings` 与全量测试通过。

### R8.4 性能复测

状态：`completed`

使用 R8.0 的相同命令和环境复测：

- 1,024 节点更新事务；
- 大树绘制与命中；
- 10,000 层深树边界；
- 文本场景；
- viewport/scroll/zoom 场景。

若结果明显退化，先定位原因，不以扩大阈值通过门禁。性能结论必须给出前后数据，
不能只给主观判断。

结果记录：

- [`../verification/performance/r8-baseline-2026-09-02.md`](../verification/performance/r8-baseline-2026-09-02.md)
- command 数与 R8.0 基线完全一致；
- 三次追加复测的 median-of-medians 均未超过 15% 回归阈值。

### R8.5 扩展边界验证

状态：`in_progress`

工作：

- 确认 DisplayList 仍为 proposal，不新增协议实现；
- 为 `ProjectiveComposition` 增加最小 capability/unsupported 测试；
- Scene3D 只验证独立扩展接口和二维嵌入边界；
- 不实现伪 3D，不把 `Point/Rectangle` 提升为模糊的通用 3D 类型。

门禁：

- backend 不支持 projective 时返回明确结果；
- 二维 layout bounds、hit-test 和 damage 语义保持不变；
- Scene3D 类型不反向依赖 FigureTree 内部可变状态。

## 4. 每批统一验证

```bash
cargo fmt --all -- --check
cargo check --workspace
cargo clippy --workspace -- -D warnings
cargo test --workspace
```

涉及 Web feature 或 backend 时追加：

```bash
cargo clippy -p web-validation \
  --target wasm32-unknown-unknown \
  -- -D warnings
./scripts/build_web_validation.sh
```

涉及窗口可见行为时，执行对应
[`../verification/manual/`](../verification/manual/)
手工流程。

## 5. 停止条件

出现以下任一情况，停止当前批次并回到契约审查：

- 需要修改 `render_recursive.rs` 主循环才能完成命名清理；
- rename 导致坐标、事件或 damage 行为变化；
- 需要恢复迭代渲染才能达到性能目标；
- DisplayList 或 Scene3D 开始反向定义二维核心；
- 性能回归无法用测量数据解释。

## 6. 完成判据

R8 只有在以下条件全部满足后才能标记为 `approved`：

- 三个旧 public 名称已删除；
- 活跃代码与文档使用 `FigureId / FigureNode / FigureTree`；
- context、坐标与失效文档清理完成；
- 性能复测不低于 R8.0 已记录基线；
- DisplayList 保持 proposal；
- ProjectiveComposition 与 Scene3D 扩展边界测试通过；
- 自动门禁和受影响手工验证均通过。
