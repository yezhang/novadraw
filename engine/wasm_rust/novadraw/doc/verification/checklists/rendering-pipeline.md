# 渲染管线开发清单

类型：`verification`

## 概述

本项目渲染管线包含三个核心环节，每个环节都可能出错。合理的开发顺序是 **"从 IR 向两端扩展"**，每个环节独立验证。

```
┌─────────────────────────────────────────────────────────────┐
│                      渲染管线架构                            │
├─────────────────────────────────────────────────────────────┤
│                                                             │
│   ┌─────────────┐    ┌─────────────┐    ┌─────────────┐    │
│   │  场景图渲染  │ →  │ RenderCommand │ →  │  渲染后端   │    │
│   │  (逻辑层)   │    │  (IR 中间层) │    │  (Vello)   │    │
│   └─────────────┘    └─────────────┘    └─────────────┘    │
│         ↓                  ↓                  ↓             │
│    变换累积错误      命令格式错误       渲染解析错误          │
│                                                             │
└─────────────────────────────────────────────────────────────┘
```

## 推荐开发顺序

### 步骤 1: 定义并验证 RenderCommand (IR 层)

**目标**: 验证渲染命令格式定义正确

```rust
// novadraw-render/src/context.rs 单元测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fill_rect_command() {
        let mut ctx = RenderContext::new();
        ctx.set_fill_color(Color::RED);
        ctx.fill_rect(10.0, 20.0, 100.0, 50.0);

        assert_eq!(ctx.commands.len(), 1);
        let cmd = &ctx.commands[0];
        match &cmd.kind {
            RenderCommandKind::FillRect { rect, color } => {
                assert_eq!(rect[0], DVec2::new(10.0, 20.0));
                assert_eq!(rect[1], DVec2::new(110.0, 70.0));
                assert_eq!(color, Some(Color::RED));
            }
            _ => panic!("Expected FillRect"),
        }
    }

    #[test]
    fn test_transform_accumulation() {
        let mut ctx = RenderContext::new();
        ctx.push_transform(Transform::from_translation(100.0, 200.0));
        ctx.push_transform(Transform::from_translation(50.0, 50.0));
        ctx.fill_rect(0.0, 0.0, 10.0, 10.0);

        let cmd = &ctx.commands[0];
        let trans = cmd.transform.translation();
        assert_eq!(trans.x(), 150.0);  // 100 + 50
        assert_eq!(trans.y(), 250.0);  // 200 + 50
    }
}
```

**验证要点**:
- [ ] 命令结构体定义正确
- [ ] 变换累积公式正确
- [ ] 状态管理 (save/restore) 正确

### 步骤 2: Mock 渲染后端

**目标**: 验证命令解析逻辑，不依赖 GPU

```rust
// 测试用 Mock 渲染器
struct MockRenderer {
    rects: Vec<(Rect, Color)>,
    lines: Vec<(Point, Point, Color, f64)>,
}

impl MockRenderer {
    fn new() -> Self {
        Self { rects: Vec::new(), lines: Vec::new() }
    }

    fn render(&mut self, commands: &[RenderCommand]) {
        for cmd in commands {
            match &cmd.kind {
                RenderCommandKind::FillRect { rect, color } => {
                    let r = Rect::new(
                        rect[0].x, rect[0].y,
                        rect[1].x - rect[0].x,
                        rect[1].y - rect[0].y
                    );
                    self.rects.push((r, color.unwrap_or(Color::BLACK)));
                }
                RenderCommandKind::Line { p1, p2, color, width, .. } => {
                    self.lines.push((*p1, *p2, *color, *width));
                }
                // ... 其他命令
            }
        }
    }
}
```

**验证要点**:
- [ ] 命令类型覆盖完整
- [ ] 参数解析正确
- [ ] 坐标转换正确

### 步骤 3: 场景图渲染逻辑验证

**目标**: 验证场景图遍历和变换累积

```rust
// novadraw-scene/src/graph/mod.rs 单元测试
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parent_child_transform() {
        // 场景: parent(100,100) -> child(30,30)
        // 期望: child 实际位置 (130, 130)
        let mut scene = FigureGraph::new();
        let parent = Rectangle::new(0.0, 0.0, 100.0, 100.0);
        let parent_id = scene.set_contents(Box::new(parent));

        let child = Rectangle::new(30.0, 30.0, 50.0, 50.0);
        let _child_id = scene.add_child_to(parent_id, Box::new(child));

        let gc = scene.render();
        assert_eq!(gc.commands.len(), 2);

        // parent 命令
        let parent_cmd = &gc.commands[0];
        let parent_trans = parent_cmd.transform.translation();
        assert_eq!(parent_trans.x(), 0.0);
        assert_eq!(parent_trans.y(), 0.0);

        // child 命令
        let child_cmd = &gc.commands[1];
        let child_trans = child_cmd.transform.translation();
        assert_eq!(child_trans.x(), 30.0);  // 相对坐标
        assert_eq!(child_trans.y(), 30.0);
    }

    #[test]
    fn test_render_order_z_order() {
        // 后添加的在上面（Z-order）
        let mut scene = FigureGraph::new();
        let contents = scene.set_contents(Box::new(Rectangle::new(0.0, 0.0, 100.0, 100.0)));

        let rect1 = Rectangle::new(0.0, 0.0, 100.0, 100.0);
        let _id1 = scene.add_child_to(contents, Box::new(rect1));

        let rect2 = Rectangle::new(50.0, 50.0, 50.0, 50.0);
        let _id2 = scene.add_child_to(contents, Box::new(rect2));

        let gc = scene.render();

        // rect1 先渲染（在下层）
        // rect2 后渲染（在上层）
        assert_eq!(gc.commands.len(), 2);
    }
}
```

**验证要点**:
- [ ] 父子变换累积正确
- [ ] Z-order 渲染顺序正确
- [ ] 可见性过滤正确
- [ ] 裁剪逻辑正确

### 步骤 4: 集成真实渲染后端

**目标**: 对接 Vello，完成端到端渲染

```rust
// novadraw-render/src/backend/vello/mod.rs
impl RendererTrait for VelloRenderer {
    fn render(&mut self, commands: &[RenderCommand]) {
        // 此时命令格式已验证，只需实现渲染
        self.scene.reset();

        for cmd in commands {
            Self::render_command(&mut self.scene, cmd, self.scale_factor);
        }

        // ... Vello 渲染调用
    }

    fn render_command(scene: &mut vello::Scene, cmd: &RenderCommand, scale: f64) {
        // 矩阵转换：Mat3 → kurbo Affine
        let matrix = cmd.transform.matrix().to_array();
        let affine = vello::kurbo::Affine::new([
            matrix[0][0], matrix[1][0],  // a, b
            matrix[0][1], matrix[1][1],  // c, d
            cmd.transform.translation().x() / scale,
            cmd.transform.translation().y() / scale,
        ]);

        match &cmd.kind {
            RenderCommandKind::FillRect { rect, color } => {
                let kurbo_rect = vello::kurbo::Rect::new(
                    rect[0].x, rect[0].y,
                    rect[1].x, rect[1].y
                );
                let vello_color = color.map(|c| VelloColor::new([
                    c.r as f32, c.g as f32, c.b as f32, c.a as f32
                ])).unwrap_or_default();

                scene.fill(vello::peniko::Fill::NonZero, affine, vello_color, None, &kurbo_rect);
            }
            // ... 其他命令
        }
    }
}
```

**验证要点**:
- [ ] 矩阵转换正确 (Mat3 → Affine)
- [ ] 坐标系转换正确 (逻辑像素 → 设备像素)
- [ ] 颜色格式正确 (Color → VelloColor)
- [ ] 渲染输出正确

## 调试方法

### 方法 1: 打印渲染命令

```rust
// 在 FigureGraph 中添加
pub fn debug_commands(&self) {
    let gc = self.render();
    eprintln!("=== 渲染命令列表 ===");
    for (i, cmd) in gc.commands.iter().enumerate() {
        eprintln!("CMD[{}] {:?}", i, cmd.kind);
        eprintln!("     transform=({:.1},{:.1})",
            cmd.transform.translation().x(),
            cmd.transform.translation().y());
    }
    eprintln!("====================");
}
```

### 方法 2: 打印场景图结构

```rust
pub fn print_tree(&self) {
    self.print_block(self.root, 0);
}

fn print_block(&self, block_id: BlockId, depth: usize) {
    let indent = "  ".repeat(depth);
    if let Some(block) = self.blocks.get(block_id) {
        let bounds = block.figure_bounds();
        eprintln!("{}BlockId({:?}): {:?} bounds=({:.0},{:.0},{:.0},{:.0})",
            indent, block_id,
            if block.is_visible { "V" } else { "H" },
            bounds.x, bounds.y, bounds.width, bounds.height);
        for &child in &block.children {
            self.print_block(child, depth + 1);
        }
    }
}
```

### 方法 3: Mock 渲染器对比

```rust
#[test]
fn test_end_to_end() {
    // 1. 创建场景图
    let mut scene = FigureGraph::new();
    // ... 添加图形

    // 2. 生成命令
    let gc = scene.render();

    // 3. 用 Mock 渲染器解析
    let mut mock = MockRenderer::new();
    mock.render(&gc.commands);

    // 4. 验证期望结果
    assert_eq!(mock.rects.len(), 2);
    assert_eq!(mock.rects[0].0, Rect::new(100.0, 100.0, 100.0, 100.0));
    assert_eq!(mock.rects[1].0, Rect::new(120.0, 130.0, 50.0, 50.0));
}
```

## 常见问题排查

| 现象 | 可能原因 | 排查方法 |
|------|----------|----------|
| 矩形位置偏移 | 变换累积公式错误 | 打印 transform 对比期望值 |
| 渲染为空 | 坐标系转换错误 | 检查 scale_factor 处理 |
| 渲染顺序错误 | 栈遍历顺序错误 | 打印渲染顺序验证 Z-order |
| 颜色错误 | 颜色格式转换问题 | 对比 Color 和 VelloColor |

## 验证清单

- [ ] RenderCommand 单元测试通过
- [ ] Mock 渲染器测试通过
- [ ] 场景图变换累积测试通过
- [ ] 场景图 Z-order 测试通过
- [ ] Vello 后端矩阵转换正确
- [ ] 端到端渲染结果正确
