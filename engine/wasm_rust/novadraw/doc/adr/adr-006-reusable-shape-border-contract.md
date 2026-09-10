# ADR-006: Reusable Shape 与 Border 产品化边界

类型：`architecture-decision`

## 状态

已通过

## 背景

M1-M5 已验证 Graphics、Figure、paint、box model、layout、validation 和 damage 的
基础主链路，但 Ellipse、RoundedRectangle、Polyline、Polygon、Triangle 和多数
Border 仍属于 deferred product surface。

若把 M10.1 定义为“补全 Graphics、Shape 与 Border”，容易重新打开已经稳定的
Graphics 基础契约，并把 clipPath、shear、gradient、XOR 等与 reusable Figure
交付无直接关系的能力混入当前批次。

Draw2D 源码同时表明：

- Shape 是 fill-before-outline 的绘制模板；
- bounded Shape 与 point-list Shape 使用不同的 bounds 来源；
- Border 同时影响 client area、preferred size 和最终绘制；
- CompoundBorder 有明确的嵌套指标和绘制顺序；
- TitleBarBorder 依赖字体与文字测量，不能独立于 Text/Label 完成；
- mutable shared Border 和按引用暴露 PointList 依赖调用方手工触发失效。

Novadraw 需要保留这些行为语义，同时维持 Runtime 原子事务、NodeState 单一几何真值
和 FigureStyle 继承模型。

## 决策

接受：

1. M10.1 重命名为“Reusable Shape 与 Border 产品化收口”，不重新设计 M1 Graphics；
2. Graphics 状态栈、坐标、裁剪和基础 command 在 M10.1 冻结；只有现有 path/command
   无法正确表达产品 Figure 时，才增加经契约证明所需的最小 primitive；
3. Shape 保持轻量绘制辅助协议，只定义 fill-before-outline 及 Shape 专属 stroke
   参数，不拥有 bounds、inherited style、Runtime、FigureTree 或 backend；
4. foreground、background 和 alpha 继续以 FigureStyle/ResolvedStyle 为唯一通用
   样式真值；M10.1 不预设宽的统一 Shape setter；
5. Rectangle、Ellipse 和 RoundedRectangle 使用 NodeState border box；
   Triangle 从 client box 派生顶点；
6. Polyline/Polygon 以 PointList 为路径真值，由 Runtime 在同一事务内提交局部 points、
   派生 NodeState bounds、damage 和通知；不暴露可绕过事务的可变 PointList；
7. reusable Figure 提供精确 shape hit-test；Triangle 不复制 Draw2D 的矩形命中退化；
8. Border 是不可变、可复用的只读策略，不持有 owner、Runtime、FigureTree 或全局状态；
   修改 Border 配置通过构造新值并执行 Runtime replacement；
9. Border 分别声明 insets、preferred size 和 ring opacity；effective opacity 还必须
   结合实际颜色与 ResolvedStyle alpha；
10. CompoundBorder 保留 Draw2D 的 insets 相加、preferred size union、outer-before-
    inner 和 Graphics state 隔离语义；
11. EtchedBorder 与 BevelBorder 由普通 line command 表达，不增加 Graphics primitive；
12. TitleBarBorder 移至 M10.2，与字体测量、Label 和 text cache 同批交付；
13. clipPath、shear、gradient、XOR、打印和高级 stroke 不进入 M10.1；
14. 若实施发现必须修改已冻结的 Graphics 契约，应停止并提交独立 architecture
    delta，经批准后再继续。

完整契约见
`doc/design/architecture/reusable-shape-border.md`。

2026-09-10 补充（ADR-014）：内置 typed setter 不构成封闭 Figure 类型清单。
自定义 Shape/组件内容采用 prepared update，经统一 revision/facts/damage 协议提交，
不要求 Runtime 为每个外部类型新增分支。不可变 Border replacement 主线不变。

## 后果

### 正面

- M1 Graphics 稳定边界不会因 M10 产品补齐而被无条件重开；
- bounded Shape 与 point-list Shape 的几何真值和更新责任明确；
- point mutation、bounds、damage 和通知保持原子；
- Border 可安全复用，不依赖隐式共享可变状态；
- Border 指标可以被 Layout 统一消费；
- Text 相关 Border 不会在字体测量契约之前形成临时实现；
- 新 Figure 和新 Border 不需要修改渲染主循环或 Vello backend 类型分派。

### 负面

- 相比 Draw2D 直接修改 Shape/Border 对象，运行期修改需要 Runtime typed operation；
- point-list Figure 需要额外的 parent-domain 到 local-domain 规范化；
- Border replacement 会产生新值，而不是原地修改共享实例；
- 精确 hit-test 和 stroke-aware visual bounds 增加几何测试成本；
- TitleBarBorder 延后到 M10.2，M10.1 不能单独交付全部六类 Border。

## 不采用的方案

### 在 M10.1 一次性补齐全部 Graphics API

不采用。大量 API 与当前产品 Figure 无关，会扩大 backend 和测试面，并重新打开已经
稳定的 M1 契约。

### 复制 Draw2D 的 mutable shared Border

不采用。Border 原地变化无法可靠通知所有 owner，会破坏 Runtime 的 validation 和
damage 原子性。

### 暴露可变 PointList

不采用。调用方可能忘记通知 Figure，导致 bounds、paint、hit-test 和 damage 使用
不同版本的几何。

### 将 Shape 通用颜色继续复制到具体 Figure

不采用。foreground、background 和 alpha 已由 FigureStyle/ResolvedStyle 统一管理，
重复状态会形成两条优先级不明确的样式路径。

### 在 M10.1 实现 TitleBarBorder

不采用。TitleBarBorder 的 insets、preferred size 和 paint 都依赖字体与文字测量，
先实现会制造临时测量协议。

### 为每个 Shape 增加 backend 专用命令

不采用。现有 path/command 能表达的 Figure 应在 scene 层组合命令，backend 不识别
Figure 类型。

## 参考

- `doc/reference/draw2d/figure/reusable-shape-border.md`
- `doc/design/architecture/reusable-shape-border.md`
- `doc/design/architecture/figure-style.md`
- `doc/design/architecture/static-architecture.md`
- `doc/design/rendering/update-manager.md`
- `doc/parity/draw2d/api-coverage.md`
- Eclipse GEF Classic commit `4463d9d0ce13c19d10fbe769d29f28b7345a8cba`

## 日期

2026-09-07
