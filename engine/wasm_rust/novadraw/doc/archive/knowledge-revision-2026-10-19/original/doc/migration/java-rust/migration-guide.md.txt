# Java to Rust 迁移指南

类型：`migration-guide`

> Draw2D 类型体系迁移实践

## 概述

本文档描述如何将 Java 的继承体系迁移到 Rust 的 Trait 系统。以 Eclipse Draw2D 为例，展示从 IFigure → Figure → Shape → 具体图形 的完整迁移路径。

---

## Draw2D 原始类层次

```text
IFigure (接口)
    │
    └── Figure (实现类，约2000行)
            │
            ├── Shape (继承Figure，添加抽象方法)
            │       │
            │       ├── RectangleFigure
            │       ├── Ellipse
            │       ├── Polyline (AbstractPointListShape)
            │       └── Polygon
            │
            └── 其他容器 (FlowPage, ScrollPane...)
```

---

## 迁移步骤

### 步骤 1：识别接口（Interface）

**Java 特征**：

```java
public interface IFigure {
    void paint(Graphics graphics);
    Rectangle getBounds();
    void setBounds(Rectangle rect);
    // ... 100+ 方法
}
```

**Rust 迁移**：

```rust
/// 核心 Figure Trait
pub trait Figure {
    fn paint(&self, graphics: &mut dyn Graphics);
    fn bounds(&self) -> Rectangle;
    fn set_bounds(&mut self, rect: Rectangle);
    // ...
}
```

**原则**：

- Java 接口 → Rust Trait
- 如果方法过多（>20个），考虑**拆分**为多个小 Trait
- IFigure 是覆盖多个协议面的宽接口；Rust 迁移可按能力边界拆分

---

### 步骤 2：识别抽象类（Abstract Class）

**Java 特征**：

```java
public abstract class Shape extends Figure {
    // 部分方法有实现
    public void paint(Graphics g) {
        fillShape(g);      // 抽象
        outlineShape(g);   // 抽象
    }

    protected abstract void fillShape(Graphics g);
    protected abstract void outlineShape(Graphics g);

    // 字段
    private int lineWidth = 1;
}
```

**Rust 迁移**：创建 Trait + 默认实现

```rust
/// Shape 抽象 Trait
pub trait Shape: Figure {
    /// 填充图形（子类实现）
    fn fill_shape(&self, graphics: &mut dyn Graphics);

    /// 描边图形（子类实现）
    fn outline_shape(&self, graphics: &mut dyn Graphics);

    /// 线宽（默认实现）
    fn line_width(&self) -> f64 {
        1.0
    }
}

/// Shape 的 paint 默认实现
impl<T: Shape> Figure for T {
    fn paint(&self, graphics: &mut dyn Graphics) {
        self.fill_shape(graphics);
        self.outline_shape(graphics);
    }
}
```

**原则**：

- 抽象类 → Trait + 默认方法实现
- 抽象方法 → Trait 中的 fn 签名（无默认实现）
- 具体方法 → Trait 中的 fn + 默认实现

---

### 步骤 3：识别具体类（Concrete Class）

**Java 特征**：

```java
public class RectangleFigure extends Shape {
    protected void fillShape(Graphics g) {
        g.fillRectangle(getBounds());
    }

    protected void outlineShape(Graphics g) {
        g.drawRectangle(getBounds());
    }
}
```

**Rust 迁移**：

```rust
/// RectangleFigure 具体实现
pub struct RectangleFigure {
    bounds: Rectangle,
    // ... 其他字段
}

impl Figure for RectangleFigure {
    fn bounds(&self) -> Rectangle { self.bounds }
    fn set_bounds(&mut self, rect: Rectangle) { self.bounds = rect }
}

impl Shape for RectangleFigure {
    fn fill_shape(&self, graphics: &mut dyn Graphics) {
        graphics.fill_rect(self.bounds);
    }

    fn outline_shape(&self, graphics: &mut dyn Graphics) {
        graphics.draw_rect(self.bounds);
    }
}
```

---

### 步骤 4：处理继承层次

#### 情况 A：浅继承（2层）

```text
Figure → RectangleFigure
```

→ 直接实现 Trait

```rust
impl Figure for RectangleFigure { ... }
```

#### 情况 B：中度继承（3层）

```text
Figure → Shape → RectangleFigure
```

→ 父类实现Trait，子类继承

```rust
// 父类提供默认实现
impl<T: Shape> Figure for T { ... }

// 子类只实现自己的部分
impl Shape for RectangleFigure { ... }
```

#### 情况 C：深度继承（4层+）

```text
IFigure → Figure → Shape → AbstractPointListShape → Polyline
```

→ 使用 Trait 继承

```rust
/// 第1层：根基 Trait
pub trait Figure: GeometryHolder {
    fn paint(&self, g: &mut dyn Graphics);
    fn bounds(&self) -> Rectangle;
}

/// 第2层：扩展 Trait
pub trait Shape: Figure {
    fn fill_shape(&self, g: &mut dyn Graphics);
    fn outline_shape(&self, g: &mut dyn Graphics);
}

/// 第3层：更具体 Trait
pub trait PointListShape: Shape {
    fn points(&self) -> &[Point];
}

/// 第4层：具体实现
impl PointListShape for Polyline { ... }
```

---

### 步骤 5：处理默认方法

**Java**：

```java
interface Drawable {
    default void draw() { System.out.println("Default draw"); }
}
class Circle implements Drawable { }  // 继承默认实现
```

**Rust**：

```rust
trait Drawable {
    fn draw(&self) {
        println!("Default draw");  // 默认实现
    }
}

struct Circle;

impl Drawable for Circle { }  // 继承默认实现
```

---

### 步骤 6：处理多继承

**Java**：

```java
interface Drawable { void draw(); }
interface Movable { void move(); }
class Player implements Drawable, Movable { }
```

**Rust**：

```rust
trait Drawable { fn draw(&self); }
trait Movable { fn r#move(&self); }

struct Player;

impl Drawable for Player { fn draw(&self) {} }
impl Movable for Player { fn r#move(&self) {} }

// 使用时约束
fn process<T: Drawable + Movable>(obj: &T) {}
```

---

### 步骤 7：处理字段

**Java**：

```java
public class RectangleFigure extends Shape {
    private Color fillColor = Color.WHITE;
    protected Rectangle bounds = new Rectangle();
}
```

**Rust**：

```rust
pub struct RectangleFigure {
    bounds: Rectangle,
    fill_color: Color,
}

impl Default for RectangleFigure {
    fn default() -> Self {
        Self {
            bounds: Rectangle::new(0.0, 0.0, 0.0, 0.0),
            fill_color: Color::WHITE,
        }
    }
}
```

---

## 实际迁移案例：IFigure 拆分

### 拆分策略

| 原 IFigure 类别 | Rust Trait |
|-----------------|------------|
| 树管理 | `ParentFigure` |
| 几何属性 | `FigureGeometry` |
| 坐标变换 | `CoordinateTransform` |
| 样式属性 | `Stylable` |
| 布局 | `Layoutable` |
| 事件监听 | `EventTarget` |

### 完整示例

```rust
// ==================== 根基 Trait ====================

/// 几何属性 Trait
pub trait FigureGeometry {
    fn bounds(&self) -> Rectangle;
    fn set_bounds(&mut self, rect: Rectangle);
    fn client_area(&self) -> Rectangle;
}

// ==================== 扩展 Trait ====================

/// 渲染 Trait
pub trait FigureRender: FigureGeometry {
    fn paint(&self, graphics: &mut dyn Graphics);
    fn is_opaque(&self) -> bool;
}

/// 容器 Trait（树结构）
pub trait FigureContainer: FigureGeometry {
    fn children(&self) -> &[FigureId];
    fn add_child(&mut self, child: FigureId);
    fn remove_child(&mut self, child: FigureId);
    fn parent(&self) -> Option<FigureId>;
    fn set_parent(&mut self, parent: Option<FigureId>);
}

/// 样式 Trait
pub trait Stylable {
    fn background_color(&self) -> Option<Color>;
    fn set_background_color(&mut self, color: Option<Color>);
    fn border(&self) -> Option<&dyn Border>;
    fn set_border(&mut self, border: Option<Box<dyn Border>>);
}

// ==================== 默认实现 ====================

/// 为所有实现 Shape 的类型提供 FigureGeometry 默认实现
impl<T: Shape> FigureGeometry for T {
    fn bounds(&self) -> Rectangle { self.get_bounds() }
    fn set_bounds(&mut self, rect: Rectangle) { self.set_bounds(rect); }
    fn client_area(&self) -> Rectangle {
        let b = self.bounds();
        let insets = self.insets();
        Rectangle::new(
            b.x + insets.left,
            b.y + insets.top,
            b.width - insets.left - insets.right,
            b.height - insets.top - insets.bottom,
        )
    }
}

// ==================== 具体实现 ====================

pub struct RectangleFigure {
    bounds: Rectangle,
    background_color: Option<Color>,
    border: Option<Box<dyn Border>>,
    // ...
}

impl FigureGeometry for RectangleFigure { ... }
impl FigureRender for RectangleFigure { ... }
impl Shape for RectangleFigure { ... }
impl Stylable for RectangleFigure { ... }
```

---

## 多态调用支持

Java 多态调用通过以下方式实现：

1. 方法覆盖（子类重写父类方法）
2. 接口实现
3. 父类引用指向子类对象（upcasting）

Rust 等价实现有两种模式：

### 1. 静态分发（Static Dispatch）

**Java 方式**：

```java
void drawFigure(RectangleFigure f) {
    f.paint(g);  // 编译时确定具体类型
}
```

**Rust 静态分发**：

```rust
fn draw_figure<T: Figure>(f: &T) {
    f.paint(&mut graphics);  // 编译时确定，无运行时开销
}
```

**特点**：

- 零运行时开销
- 编译期多态
- 适用于已知具体类型

---

### 2. 动态分发（Dynamic Dispatch）

**Java 方式**：

```java
void drawFigure(IFigure f) {
    f.paint(g);  // 运行时多态
}

// 调用
IFigure f = new RectangleFigure();
f.paint(g);  // 调用 RectangleFigure 的 paint
```

**Rust 动态分发**：

```rust
fn draw_figure(f: &dyn Figure) {
    f.paint(&mut graphics);  // 运行时分派
}

// 调用
let f: Box<dyn Figure> = Box::new(RectangleFigure::new());
f.paint(&mut graphics);  // 调用 RectangleFigure 的 paint
```

---

### 3. Trait 对象实现多态

#### 3.1 堆上分配（Box<dyn Trait>）

```rust
/// 存储多种图形类型
pub struct Scene {
    figures: Vec<Box<dyn Figure>>,
}

impl Scene {
    pub fn add(&mut self, figure: Box<dyn Figure>) {
        self.figures.push(figure);
    }

    pub fn render_all(&self, g: &mut dyn Graphics) {
        for figure in &self.figures {
            figure.paint(g);  // 动态分发
        }
    }
}
```

#### 3.2 引用传递（&dyn Trait）

```rust
/// 接受任意 Figure
pub fn paint_all(figures: &[&dyn Figure], g: &mut dyn Graphics) {
    for f in figures {
        f.paint(g);
    }
}

// 使用
let rect = RectangleFigure::new();
let ellipse = EllipseFigure::new();
paint_all(&[&rect, &ellipse], &mut g);
```

---

### 4. 继承层次的多态

#### 4.1 Trait 继承链

```rust
pub trait Figure: GeometryHolder {
    fn paint(&self, g: &mut dyn Graphics);
}

pub trait Shape: Figure {
    fn fill_shape(&self, g: &mut dyn Graphics);
    fn outline_shape(&self, g: &mut dyn Graphics);
}

// 静态分发：已知具体类型
fn draw_shape<T: Shape>(s: &T, g: &mut dyn Graphics) {
    s.paint(g);  // 调用 T 的 paint
}

// 动态分发：未知具体类型
fn draw_figure(f: &dyn Shape, g: &mut dyn Graphics) {
    f.paint(g);  // 运行时确定
}
```

#### 4.2 Blanket Implementation

Java 中可以通过继承自动获得方法：

```java
abstract class Shape extends Figure {
    public void paint(Graphics g) {
        fillShape(g);
        outlineShape(g);
    }
}
```

Rust 等价：

```rust
/// 为所有实现 Shape 的类型提供 Figure 默认实现
impl<T: Shape> Figure for T {
    fn paint(&self, g: &mut dyn Graphics) {
        self.fill_shape(g);
        self.outline_shape(g);
    }
}
```

---

### 5. 覆盖父类方法

#### 5.1 Java 覆盖

```java
class RectangleFigure extends Shape {
    @Override
    public void paint(Graphics g) {
        // 自定义实现
        g.fillRectangle(getBounds());
    }
}
```

#### 5.2 Rust 覆盖

```rust
impl Shape for RectangleFigure {
    fn fill_shape(&self, g: &mut dyn Graphics) {
        g.fill_rect(self.bounds());
    }

    fn outline_shape(&self, g: &mut dyn Graphics) {
        g.draw_rect(self.bounds());
    }
}

/// 自定义 paint 覆盖（可选）
impl Figure for RectangleFigure {
    fn paint(&self, g: &mut dyn Graphics) {
        // 自定义实现
        self.fill_shape(g);
    }
}
```

---

### 6. 泛型约束实现多态

#### 6.1 单一约束

```rust
fn render<T: Figure>(figure: &T, g: &mut dyn Graphics) {
    figure.paint(g);
}
```

#### 6.2 多重约束

```java
// Java：实现多个接口
class Player implements Drawable, Movable {
    @Override public void draw() { }
    @Override public void move() { }
}

void process(Player p) { }  // 接受实现了 Drawable 和 Movable 的类型
```

```rust
// Rust：多个 trait 约束
trait Drawable { fn draw(&self); }
trait Movable { fn r#move(&self); }

fn process<T: Drawable + Movable>(obj: &T) {
    obj.draw();
    obj.r#move();
}
```

#### 6.3 where 子句

```rust
fn process<T>(obj: &T)
where
    T: Drawable + Movable,
{
    obj.draw();
}
```

---

### 7. 运行时类型识别

#### 7.1 Java instanceof

```java
void handle(IFigure f) {
    if (f instanceof RectangleFigure) {
        RectangleFigure r = (RectangleFigure) f;
        r.setCornerRadius(10);
    }
}
```

#### 7.2 Rust downcast

```rust
use std::any::Any;

trait Figure: Any {
    fn as_any(&self) -> &dyn Any;
}

impl<T: Figure> Figure for T {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

fn handle(f: &dyn Figure) {
    if let Some(rect) = f.as_any().downcast_ref::<RectangleFigure>() {
        rect.set_corner_radius(10.0);
    }
}
```

---

### 8. 多态容器

#### 8.1 Java List

```java
List<Shape> shapes = Arrays.asList(
    new RectangleFigure(),
    new EllipseFigure()
);

for (Shape s : shapes) {
    s.paint(g);  // 多态调用
}
```

#### 8.2 Rust Vec<Box<dyn Trait>>

```rust
let shapes: Vec<Box<dyn Shape>> = vec![
    Box::new(RectangleFigure::new()),
    Box::new(EllipseFigure::new()),
];

for shape in &shapes {
    shape.paint(&mut g);  // 多态调用
}
```

#### 8.3 引用数组

```rust
let rect = RectangleFigure::new();
let ellipse = EllipseFigure::new();

let figures: Vec<&dyn Shape> = vec![&rect, &ellipse];

for f in &figures {
    f.paint(&mut g);
}
```

---

### 9. 完整示例：Draw2D 多态渲染

#### Java 版本

```java
public void paint(Graphics g) {
    List<IFigure> children = getChildren();
    for (IFigure child : children) {
        child.paint(g);  // 多态调用
    }
}
```

#### Rust 版本（动态分发）

```rust
pub struct Figure {
    children: Vec<Box<dyn Figure>>,
}

impl Figure {
    pub fn paint(&self, g: &mut dyn Graphics) {
        for child in &self.children {
            child.paint(g);  // 动态分发
        }
    }
}
```

#### Rust 版本（静态分发）

```rust
/// 泛型版本，零运行时开销
pub fn paint_all<T: Figure>(figures: &[T], g: &mut dyn Graphics) {
    for f in figures {
        f.paint(g);  // 静态分发
    }
}
```

---

### 10. 多态选择指南

| 场景 | 推荐方式 | 原因 |
|------|----------|------|
| 存储异构类型 | `Vec<Box<dyn Trait>>` | 运行时多态 |
| 函数参数类型已知 | 泛型 `T: Trait` | 零开销 |
| 回调/接口 | `&dyn Trait` | 灵活 |
| 性能关键 | 泛型 + 内联 | 静态分发 |
| 不确定类型数 | Trait Object | 运行时决定 |

---

### 11. 与 Java 对比总结

| Java | Rust | 多态类型 |
|------|------|----------|
| `IFigure f = new Rectangle()` | `let f: Box<dyn Figure>` | 动态 |
| `void f(Rectangle r)` | `fn f<T: Figure>(r: &T)` | 静态 |
| `f instanceof Rectangle` | `f.downcast_ref::<Rectangle>()` | 类型识别 |
| `List<IFigure>` | `Vec<Box<dyn Figure>>` | 容器 |

---

### 决策流程图

```text
Java 类型
    │
    ├── 只有方法签名（无字段）
    │     │
    │     └──→ Trait（无关联类型）
    │
    ├── 有方法 + 有抽象方法（无实现）
    │     │
    │     └──→ Trait（抽象方法签名 + 默认实现）
    │
    ├── 有具体方法实现 + 有字段
    │     │
    │     ├── 简单 → Trait + Default impl
    │     │
    │     └── 复杂 → 拆分为：
    │           1. Trait（接口定义）
    │           2. Struct（数据）
    │           3. Impl（实现）
    │
    └── 具体类（有父类）
          │
          ├── 2层继承 → impl Trait for Struct
          │
          ├── 3层继承 → Trait + 父Trait impl + 子Trait impl
          │
          └── 4层+继承 → Trait 继承链
```

---

## 经验法则

| Java 模式 | Rust 等价 | 注意事项 |
|-----------|-----------|----------|
| `interface` | `trait` | 方法签名 |
| `abstract class` | `trait` + 默认 impl | 抽象方法无默认 |
| `extends` | `trait` 继承 | `trait A: B + C` |
| `implements` | `impl Trait for Type` | - |
| 多继承 | `trait + trait` | 用 `+` 连接 |
| 字段 | `struct` 字段 | 无 private，用 `pub(crate)` |
| 静态方法 | `impl Type` 关联函数 | `Type::method()` |
| `default` 方法 | trait 默认 impl | 直接写函数体 |
| `@Override` | 自动覆盖 | Rust 无此关键字 |

---

## 总结

迁移核心思路：

1. **接口 → Trait**：Java interface 直接转 Rust trait
2. **抽象类 → Trait + Default**：抽象方法留签名，具体方法给默认
3. **继承链 → Trait 继承**：多层继承用 `trait A: B` 表示
4. **宽接口 → 能力拆分**：按稳定语义把大接口拆成多个小 trait
5. **具体类 → Struct + Impl**：数据存 struct，行为放 impl
