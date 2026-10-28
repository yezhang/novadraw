# 布局约束 (Layout Constraints) 分析

类型：`reference-analysis`

## 概述

在 draw2d 中，**约束 (Constraint)** 是布局管理器与子图形 (Figure) 之间的"契约"——它定义了每个子图形在布局中的定位规则。不同布局系统的约束语义完全不同。

## 约束关系图

```text
┌─────────────────────────────────────────────────────────────────┐
│                        Parent Figure                              │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │                    Client Area                           │    │
│  │                                                          │    │
│  │   ┌──────────────┐    ┌──────────────┐                  │    │
│  │   │   Child A    │    │   Child B    │                  │    │
│  │   └──────────────┘    └──────────────┘                  │    │
│  │                                                          │    │
│  └─────────────────────────────────────────────────────────┘    │
│                        ▲                ▲                         │
│                        │   Constraint   │                         │
│  ┌─────────────────────┴────────────────┴──────────────────┐    │
│  │                 LayoutManager                             │    │
│  │                                                          │    │
│  │   layout.setConstraint(childA, rectA)                    │    │
│  │   layout.setConstraint(childB, borderLayout.TOP)        │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### 约束协议示意

```text
LayoutManager ←──── 约束类型 ────→ Figure
     │
     ├── XYLayout       ──→ Rectangle(x, y, w, h)     // 精确位置
     ├── BorderLayout   ──→ Integer(TOP/BOTTOM/...)   // 边线位置
     ├── FlowLayout     ──→ (无约束)                   // 流式排列
     ├── GridLayout     ──→ GridData(align, span...)  // 网格属性
     └── DelegatingLayout ──→ Locator(calculatePos)    // 自定位
```

## 约束的本质

```java
// LayoutManager 接口中的核心方法
Object getConstraint(IFigure child);           // 获取子图形的约束
void setConstraint(IFigure child, Object c);   // 设置子图形的约束
```

**关键理解**: 约束是**子图形与布局器之间的私有协议**。相同类型的约束对象在不同布局器中有完全不同的含义。

## 不同布局系统的约束语义对比

### 1. XYLayout - 绝对定位约束

**约束类型**: `Rectangle`

**语义**: 定义子图形的精确位置和大小

```java
// 子图形放置在 (10, 20)，大小 100x50
Rectangle constraint = new Rectangle(10, 20, 100, 50);
layout.setConstraint(childFigure, constraint);
```

| 字段 | 含义 |
|------|------|
| `x` | 相对父图形客户区的 X 坐标 |
| `y` | 相对父图形客户区的 Y 坐标 |
| `width` | 子图形宽度 (-1 表示使用首选宽度) |
| `height` | 子图形高度 (-1 表示使用首选高度) |

**布局示意**:

```text
┌─────────────────────────────────────────┐
│  ┌──────┐                               │
│  │ChildA│  (x=10, y=10, w=100, h=50)    │
│  └──────┘                               │
│           ┌──────────┐                   │
│           │  ChildB  │  (x=30, y=80)     │
│           └──────────┘                   │
│                                         │
│                        ┌────────────┐   │
│                        │   ChildC   │   │
│                        │ (x=200,y=50)│   │
│                        └────────────┘   │
└─────────────────────────────────────────┘
```

---

### 2. BorderLayout - 边线位置约束

**约束类型**: `Integer` (使用预定义常量)

**语义**: 定义子图形占据的边线位置

```java
// 子图形放置在顶部
layout.setConstraint(topFigure, BorderLayout.TOP);

// 子图形放置在中心
layout.setConstraint(centerFigure, BorderLayout.CENTER);
```

| 约束值 | 语义 |
|--------|------|
| `TOP` | 占据顶部边缘，宽度填满，高度由首选高度决定 |
| `BOTTOM` | 占据底部边缘，宽度填满，高度由首选高度决定 |
| `LEFT` | 占据左侧边缘，高度填满（扣除TOP/BOTTOM），宽度由首选宽度决定 |
| `RIGHT` | 占据右侧边缘，高度填满（扣除TOP/BOTTOM），宽度由首选宽度决定 |
| `CENTER` | 占据剩余中间区域 |

**重要特性**: 每个位置只能有一个子图形。后设置的会覆盖先设置的。

**布局示意**:

```text
┌─────────────────────────────────────────┐
│                  TOP                      │
│  ═══════════════════════════════════════ │
│                                          │
│  LEFT  │         CENTER          │ RIGHT │
│        │                         │       │
│        │   (剩余空间)            │       │
│        │                         │       │
│  ═══════════════════════════════════════ │
│                BOTTOM                     │
└─────────────────────────────────────────┘
```

---

### 3. FlowLayout - 无约束/隐式约束

**约束类型**: `Object` (通常不使用或内部使用)

**语义**: FlowLayout 不依赖外部约束，子图形的排列完全由布局器的配置决定

```java
// FlowLayout 不使用约束
// 子图形按添加顺序排列，自动换行
FlowLayout layout = new FlowLayout();
parent.setLayoutManager(layout);
parent.add(child1);  // 添加到第一行
parent.add(child2);  // 同排或换行
```

**布局示意**:

```text
┌─────────────────────────────────────────┐
│  ┌────┐ ┌────┐ ┌────┐ ┌────┐         │
│  │ A  │ │ B  │ │ C  │ │ D  │  ──→    │  ← 自动换行
│  └────┘ └────┘ └────┘ └────┘         │
│                                         │
│  ┌────┐ ┌────┐ ┌────┐                │
│  │ E  │ │ F  │ │ G  │                │
│  └────┘ └────┘ └────┘                │
└─────────────────────────────────────────┘
```

**控制参数** (布局器级别，非子图形级别):

| 参数 | 作用 |
|------|------|
| `majorAlignment` | 行的对齐方式 (左/中/右) |
| `minorAlignment` | 行内元素的对齐方式 (顶/中/底) |
| `majorSpacing` | 行间距 |
| `minorSpacing` | 元素间距 |

---

### 4. GridLayout - 单元格约束

**约束类型**: `GridData`

**语义**: 定义子图形在网格中的位置、大小和对齐方式

```java
// 创建 GridData 约束
GridData data = new GridData();
data.horizontalAlignment = SWT.FILL;
data.verticalAlignment = SWT.FILL;
data.grabExcessHorizontalSpace = true;
data.grabExcessVerticalSpace = true;

layout.setConstraint(childFigure, data);
```

**GridData 核心字段**:

| 字段 | 类型 | 语义 |
|------|------|------|
| `horizontalAlignment` | int | 水平对齐 (BEGINNING/CENTER/END/FILL) |
| `verticalAlignment` | int | 垂直对齐 (BEGINNING/CENTER/END/FILL) |
| `horizontalSpan` | int | 水平跨列数 |
| `verticalSpan` | int | 垂直跨行数 |
| `widthHint` | int | 宽度提示 (最小宽度) |
| `heightHint` | int | 高度提示 (最小高度) |
| `grabExcessHorizontalSpace` | boolean | 是否占用额外水平空间 |
| `grabExcessVerticalSpace` | boolean | 是否占用额外垂直空间 |
| `horizontalIndent` | int | 水平缩进 |

---

### 5. DelegatingLayout - 定位器约束

**约束类型**: `Locator`

**语义**: 将定位责任委托给子图形自己的定位器

```java
// 使用 Locator 约束
Locator locator = new ConnectionLocator(connection) {
    @Override
    protected int getGap() {
        return 5;
    }
};

layout.setConstraint(labelFigure, locator);
```

| 约束类型 | 语义 |
|----------|------|
| `Locator` | 负责计算子图形的位置和大小 |

**特点**: `DelegatingLayout.layout(container)` 遍历约束，将每个 child 的定位委托给
对应 `Locator.relocate(child)`；决定位置的是 Locator，不是 child 自身。

---

## 约束语义总结表

| 布局器 | 约束类型 | 核心语义 | 使用场景 |
|--------|----------|----------|----------|
| XYLayout | Rectangle | 绝对位置+大小 | 精确控制、图形编辑器 |
| BorderLayout | Integer | 边线位置 | 经典边框布局 |
| FlowLayout | (无) | 流式顺序 | 标签、按钮组 |
| GridLayout | GridData | 网格单元格属性 | 复杂表单、对齐布局 |
| DelegatingLayout | Locator | 自定位 | 连接线标签、动态定位 |

## 设计启示

1. **约束是布局协议**: 每种布局定义自己的约束类型，构成与子图形的私有协议

2. **约束不跨布局兼容**: Rectangle 在 XYLayout 中是位置，在其他布局中可能无效

3. **约束与布局器绑定**: 使用约束前必须确认父图形使用的是哪种布局器

4. **约束支持多态**: 通过 Object 类型支持不同类型的约束，运行时检查具体类型

5. **LayoutManager 实现可以有状态**: `AbstractLayout` 缓存 preferred size，
   `AbstractConstraintLayout` 和 `GridLayout` 保存 child 到 constraint 的映射。
   `LayoutManager` 是策略接口，不是“无状态实现”的约束。

## 源码基线

eclipse/gef-classic commit `4463d9d0c`（2026-01-01）：
`LayoutManager.java`、`AbstractLayout.java`、`AbstractConstraintLayout.java`、
`XYLayout.java`、`BorderLayout.java`、`FlowLayout.java`、`GridLayout.java`、
`DelegatingLayout.java`。
