# Bounds 迁移计划

> **Legacy 文档**
>
> 本文是坐标系统迁移期间的历史快照，表格中的“待实现”状态已经过期。
> 当前坐标契约与进度分别以 `doc/design/coordinates/coordinate-system.md`、
> `doc/parity/draw2d/api-coverage.md` 和
> `doc/roadmap/00-index.md` 为准。

本文档记录将本项目的 Figure bounds 系统迁移到与 Eclipse Draw2D 一致所需的改进项。

## 1. 当前实现 vs g2 完整能力对比

### 1.1 Bounds 相关能力对比

| 能力                    | g2 实现                                  | 本项目实现               | 差距        |
| ----------------------- | ---------------------------------------- | ------------------------ | ----------- |
| **bounds 存储**         | Rectangle (x,y,width,height)             | Rect (x,y,width,height)  | ✅ 已实现   |
| **bounds 含义**         | 绝对坐标（相对于坐标根）                 | 绝对坐标（相对于坐标根） | ✅ 已实现   |
| **坐标传播**            | `primTranslate()` 自动传播到子节点       | ✅ FigureGraph 实现       | ✅ 已实现   |
| **useLocalCoordinates** | 支持切换坐标模式                         | ✅ 已实现                 | ✅ 已实现   |
| **坐标根概念**          | ScalableFreeformLayeredPane, Viewport 等 | ✅ 支持检测               | ⚠️ 部分实现 |
| **translateFromParent** | 递归坐标转换                             | ⏳ 待实现                 | ❌ 未实现   |
| **translateToParent**   | 递归坐标转换                             | ⏳ 待实现                 | ❌ 未实现   |

### 1.2 Figure Trait 能力对比

| 方法                            | g2  | 本项目 | 状态         |
| ------------------------------- | --- | ------ | ------------ |
| `bounds()`                      | ✅  | ✅     | 已实现       |
| `setBounds()`                   | ✅  | ✅     | 已实现       |
| `containsPoint()`               | ✅  | ✅     | 已实现       |
| `hit_test()`                    | ✅  | ✅     | 已实现       |
| `useLocalCoordinates()`         | ✅  | ✅     | 已实现       |
| `translate(int,int)`            | ✅  | ✅     | FigureGraph   |
| `primTranslate(int,int)`        | ✅  | ✅     | FigureGraph   |
| `translateFromParent()`         | ✅  | ⏳     | 待实现       |
| `translateToParent()`           | ✅  | ⏳     | 待实现       |
| `translateToAbsolute()`         | ✅  | ✅     | FigureGraph   |
| `translateToRelative()`         | ✅  | ❌     | 未实现       |
| `isCoordinateSystem()`          | ✅  | ✅     | FigureGraph   |
| `getClientArea()`               | ✅  | ✅     | 部分实现     |
| `getInsets()`                   | ✅  | ✅     | 有定义       |
| `isOpaque()`                    | ✅  | ✅     | 已实现       |
| `fireFigureMoved()`             | ✅  | ❌     | 未实现       |
| `fireCoordinateSystemChanged()` | ✅  | ❌     | 未实现       |

### 1.3 布局相关能力对比

| 能力             | g2  | 本项目 | 状态   |
| ---------------- | --- | ------ | ------ |
| LayoutManager    | ✅  | ✅     | 已实现 |
| setConstraints   | ✅  | ✅     | 已实现 |
| invalidate()     | ✅  | 部分   | 需完善 |
| validate()       | ✅  | 部分   | 需完善 |
| revalidate()     | ✅  | ❌     | 未实现 |
| invalidateTree() | ✅  | ❌     | 未实现 |

### 1.4 命中测试能力对比

| 能力                   | g2  | 本项目 | 状态     |
| ---------------------- | --- | ------ | -------- |
| findFigureAt           | ✅  | ❌     | 未实现   |
| findMouseEventTargetAt | ✅  | ✅     | 已实现   |
| 逆序遍历子节点         | ✅  | ✅     | 已实现   |
| 坐标转换               | ✅  | ⚠️     | 部分实现 |
| 路径追踪               | ✅  | ✅     | 已实现   |
| 剪枝（clientArea）     | ✅  | ❌     | 未实现   |
| TreeSearch 过滤        | ✅  | ❌     | 未实现   |

---

## 2. 执行清单

### 阶段 1：基础坐标系统（短期）

| #   | 任务                                                | 优先级 | 状态         |
| --- | --------------------------------------------------- | ------ | ------------ |
| 1.1 | 修复命中测试坐标转换（当前测试失败）                | P0     | ✅ completed |
| 1.2 | 实现 `translate()` 坐标传播，统一 bounds 为绝对坐标 | P1     | ✅ completed |
| 1.3 | 实现 `setBounds()` 方法和 erase/repaint 语义        | P2     | ✅ completed |

### 阶段 2：坐标转换方法（中期）

| #   | 任务                                                      | 优先级 | 状态         |
| --- | --------------------------------------------------------- | ------ | ------------ |
| 2.1 | 实现 `translateFromParent/translateToParent` 坐标转换方法 | P3     | ⏳ pending   |
| 2.2 | 实现 `translateToAbsolute/translateToRelative` 递归转换   | P4     | ✅ completed |

### 阶段 3：布局失效机制（中期）

| #   | 任务                                                     | 优先级 | 状态       |
| --- | -------------------------------------------------------- | ------ | ---------- |
| 3.1 | 实现 `invalidate/revalidate/invalidateTree` 布局失效机制 | P5     | ⏳ pending |

### 阶段 4：坐标根与本地坐标模式（长期）

| #   | 任务                                                                   | 优先级 | 状态       |
| --- | ---------------------------------------------------------------------- | ------ | ---------- |
| 4.1 | 实现 `isCoordinateSystem()` 方法                                       | P6     | ⏳ pending |
| 4.2 | 实现坐标根容器（FreeformLayer, Viewport, ScalableFreeformLayeredPane） | P6     | ⏳ pending |

### 阶段 5：高级命中测试（长期）

| #   | 任务                                   | 优先级 | 状态       |
| --- | -------------------------------------- | ------ | ---------- |
| 5.1 | 实现 `findFigureAt` 和 TreeSearch 过滤 | P7     | ⏳ pending |
| 5.2 | 实现基于 `getClientArea()` 的剪枝      | P7     | ⏳ pending |

---

## 3. 任务详情

### 1.1 修复命中测试坐标转换（当前测试失败）

**问题**：当前命中测试中嵌套场景测试失败，路径未正确追踪

**根因分析**：

- 子节点 bounds 是相对于父节点的
- 命中测试需要将全局坐标转换为本地坐标
- 当前偏移量计算逻辑有误

**预期行为**：

```
场景：root(100,100) → parent(30,30) → child(-30,-30)
点击点 (120, 120) 应该命中 child

绝对坐标：
- root: (100, 100, 200, 150)
- parent: (130, 130, 100, 80)
- child: (100, 100, 60, 40)  // 130-30=100, 130-30=100
```

**关键代码路径**：

- `novadraw-scene/src/scene/hit_test.rs` - HitTester

---

### 1.2 实现 primTranslate() 坐标传播 ✅ 已完成

**目标**：统一 bounds 含义为绝对坐标，父节点移动时自动传播到子节点

**实现位置**：`FigureGraph::prim_translate(block_id, dx, dy)`

**设计**：

```rust
// FigureGraph 中实现
pub fn prim_translate(&mut self, block_id: BlockId, dx: f64, dy: f64) {
    // 使用显式栈迭代实现，避免递归栈溢出
    let mut stack = vec![block_id];

    while let Some(id) = stack.pop() {
        if let Some(block) = self.blocks.get_mut(id) {
            // 修改当前节点的 bounds (x, y)
            if let Some(rect) = block.figure.as_rectangle_mut() {
                rect.bounds.x += dx;
                rect.bounds.y += dy;
            }

            // 检查是否使用本地坐标模式
            if block.figure.use_local_coordinates() {
                // 本地坐标模式：不传播
                continue;
            }

            // 默认模式：将所有子节点加入栈进行平移
            for &child_id in &block.children {
                stack.push(child_id);
            }
        }
    }
}
```

**已实现方法**：

- `prim_translate()` - 原始平移，传播到子节点
- `translate_to_absolute()` - 获取节点的绝对位置
- `is_coordinate_system()` - 检查节点是否是坐标根

**影响范围**：

- 子节点 bounds 始终保持绝对坐标（相对于坐标根）
- 使用显式栈避免递归栈溢出

---

### 1.3 实现 setBounds() 方法 ✅ 已完成

**目标**：实现完整的 bounds 设置语义

**实现位置**：
- `FigureBlock::set_bounds()` - `novadraw-scene/src/scene/mod.rs:158-164`
- `FigureGraph::set_bounds()` - `novadraw-scene/src/scene/mod.rs:698-731`

**最小实现方案**：
- 计算位置偏移 (dx, dy)
- 使用栈迭代调用 `prim_translate` 传播偏移到所有子节点
- 更新自身的宽高

**未实现（留待后续迭代）**：
- `erase()` - 擦除旧位置（需渲染系统集成）
- `repaint()` - 重绘新位置（需渲染系统集成）
- `fireFigureMoved()` - 事件通知（需事件系统）
- `fireCoordinateSystemChanged()` - 坐标根变化通知
- `invalidate()` / `revalidate()` - 布局失效机制

**测试覆盖**：
- `test_scene_set_bounds_basic` - 基本功能测试
- `test_scene_set_bounds_position_only` - 仅位置变化测试
- `test_scene_set_bounds_nested_propagation` - 嵌套传播测试
- `test_scene_set_bounds_size_only` - 仅尺寸变化测试

---

## 4. 优先级说明

| 优先级 | 含义       | 适用场景                      |
| ------ | ---------- | ----------------------------- |
| P0     | 最高优先级 | 修复测试失败，影响核心功能    |
| P1     | 高优先级   | 基础 API 缺失，影响系统一致性 |
| P2     | 中高优先级 | 常用 API，影响用户体验        |
| P3     | 中优先级   | 完整 API，支持高级功能        |
| P4     | 中优先级   | 完整 API，支持高级功能        |
| P5     | 中低优先级 | 布局系统完整                  |
| P6     | 低优先级   | 高级特性，支持缩放/滚动       |
| P7     | 低优先级   | 高级特性，支持过滤            |

---


---

## 5. 依赖关系

```
1.1 修复命中测试        ✅ 已完成
    │
    ▼
1.2 primTranslate       ✅ 已完成
    │
    ▼
1.3 setBounds()         ✅ 已完成
    │
    ▼
2.1 translateFromParent ⏳ pending
    │
    ▼
2.2 translateToAbsolute ✅ 已完成
    │
    ▼
3.1 布局失效机制        ⏳ pending
    │
    ▼
4.1 isCoordinateSystem  ⏳ pending
    │
    ▼
4.2 坐标根容器          ⏳ pending
    │
    ▼
5.1 高级命中测试        ⏳ pending
```

---

## 6. 验收标准

每个任务完成后需满足：

1. **单元测试通过** - 所有相关测试 100% 通过
2. **编译无警告** - `cargo clippy` 无警告
3. **API 文档完整** - 所有 pub 方法有文档注释
4. **行为与 g2 一致** - 关键行为与 Eclipse Draw2D 相同

---

_创建时间：2024-01-15_
_基于 Eclipse Draw2D 源码分析_
