# Eclipse Draw2D 演进历史分析

类型：`reference-analysis`

## 项目起源

- **源码版权起始年份**: 2000 年
- **初始实现贡献者（按源码头）**: IBM Corporation
- **许可证**: Eclipse Public License 2.0
- **最早 Git Commit**: 2002-06-04 (Commit ID: `4da32d966f3986a1313518d9e02489764d11b8dd`)

**证据边界**：当前 Git 历史只能证明该仓库最早可见提交为 2002-06-04；源码版权
年份可追溯到 2000，但仅凭版权头不能断言项目启动日期，也不能证明 2002 年发生过
特定版本控制迁移。

## 核心架构设计

从当前源码可以直接确认以下结构模式：

1. **轻量级组件模型**: Figure 不继承 SWT Widget，独立渲染
2. **组合模式 (Composite Pattern)**: 支持图形嵌套形成树形结构
3. **策略模式 (Strategy Pattern)**: 布局管理器可插拔
4. **观察者模式 (Observer Pattern)**: 属性变更通知机制

## 当前源码中的长期核心能力

### 1. 核心图形系统

```
├── Figure (核心基类)
├── IFigure (接口契约)
├── LightweightSystem (连接 SWT Canvas 与 Draw2d)
└── UpdateManager (管理重绘和更新)
```

**关键类**:
- `Figure`: 所有可视对象的根类，包含边界、布局、子图形管理
- `IFigure`: 定义图形的基本契约
- `LightweightSystem`: SWT 与 Draw2D 之间的桥梁
- `UpdateManager`: 管理图形更新和重绘

### 2. 几何系统

```
├── Rectangle (矩形区域)
├── Point (点坐标)
├── Dimension (尺寸)
├── Insets (边距)
└── Translatable (坐标变换接口)
```

**关键特性**:
- 整数坐标系统 (区别于浮点)
- 支持坐标变换
- 边界计算和包含检测

### 3. 布局管理器 (原始版本)

| 布局器 | 约束类型 | 用途 |
|--------|----------|------|
| **XYLayout** | Rectangle (x, y, width, height) | 绝对定位 |
| **BorderLayout** | Integer (TOP/LEFT/CENTER/RIGHT/BOTTOM) | 五区边框布局 |
| **FlowLayout** | (无约束) | 流式排列 |
| **DelegatingLayout** | Locator | 子图形自定位 |

**核心接口**:
- `LayoutManager`: 定义布局契约
- `AbstractLayout`: 布局基类
- `AbstractConstraintLayout`: 约束布局基类

### 4. 基础图形实现

```
├── RectangleFigure (矩形)
├── Ellipse (椭圆)
├── Polyline (折线)
├── Polygon (多边形)
├── Label (文本标签)
└── ImageFigure (图像)
```

### 5. 连接与路由系统

```
├── Connection (连接线接口)
├── ConnectionAnchor (连接锚点接口)
├── AbstractConnectionAnchor (锚点基类)
├── ChopboxAnchor (边界框锚点)
└── AbstractRouter (路由基类)
```

**关键特性**:
- 锚点用于定义连接的附着点
- 路由器负责计算连接线路径
- 支持正交和折线路由

### 6. 事件处理系统

```
├── EventDispatcher (事件分发)
├── MouseListener/MouseMotionListener (鼠标事件)
├── KeyListener (键盘事件)
├── FigureListener (图形变化通知)
├── CoordinateListener (坐标变换通知)
└── PropertyChangeListener (属性变更通知)
```

### 7. 基础渲染支持

```
├── Graphics (抽象绘制上下文接口)
├── SWTGraphics (基于 SWT GC 的实现)
├── ColorProvider (颜色管理)
└── Font/Color 资源管理
```

## 历史结论的证据要求

当前源码树和一次最早提交查询不足以可靠重建“某几年新增了哪些能力”的时间线。
如需维护逐版本历史，应按 class/path 执行 `git log --follow`，并结合 Eclipse release
notes 后再写入；本文不再把未经逐项验证的年份分段当作事实。

## 设计遗产

当前仓库能直接确认的依赖关系包括：

1. **GEF (Graphical Editing Framework)**: 基于 draw2d 构建可视化编辑器
2. **Zest**: 可视化工具包 (图表、网络图)
3. 其他 Eclipse 项目是否“受 Draw2D 影响”属于仓库外历史判断，不在本次源码审计
   中作无来源断言

其核心思想——**轻量级图形、组合模式、布局分离**——至今仍是图形 UI 设计的最佳实践。

## 相关 Git Commit 信息

| 信息项 | 值 |
|--------|-----|
| 最早 Commit ID | `4da32d966f3986a1313518d9e02489764d11b8dd` |
| 最早 Commit 日期 | 2002-06-04 |
| 当前 HEAD Commit | `4463d9d0ce13c19d10fbe769d29f28b7345a8cba` |
| 项目版权声明 | Copyright (c) 2000, IBM Corporation |

---

*源码审计基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。*
