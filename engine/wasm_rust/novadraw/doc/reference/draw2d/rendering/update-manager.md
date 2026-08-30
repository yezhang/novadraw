# g2 完整渲染管线分析

类型：`reference-analysis`

> 本文档聚焦 Eclipse Draw2d 的渲染管线设计，作为 novadraw 架构演进的参考基准。
> 除非明确说明，否则内容仅描述 g2 本身的机制，不涉及 novadraw 的实现差异。

## 目录

1. [1. 整体架构](#1-整体架构)
2. [2. LightweightSystem](#2-lightweight-system)
3. [3. UpdateManager](#3-update-manager)
4. [4. Figure.paint 调用链](#4-figurepaint-调用链)
5. [5. Graphics 状态管理](#5-graphics-状态管理)
6. [6. EventDispatcher](#6-event-dispatcher)
7. [7. 完整管线图](#7-完整管线图)
8. [8. 关键设计原则](#8-关键设计原则)

---

## 1. 整体架构

Draw2d 的渲染管线由三个核心组件构成：

```text
SWT Canvas (GC)
       │
       ▼
LightweightSystem        ← 平台桥接层：SWT 事件 → Draw2d
       │
       ▼
UpdateManager           ← 更新编排层：延迟批处理 + 两阶段更新
       │
       ▼
Figure Tree             ← 渲染层：paint() 遍历
       │
       ▼
GraphicsSource          ← 渲染输出：双缓冲或直接 GC
```

### 1.1 组件职责

| 组件 | 类 | 职责 |
|------|----|------|
| 平台桥接层 | `LightweightSystem` | 持有 root Figure + UpdateManager，注册 SWT 事件监听 |
| 更新编排层 | `DeferredUpdateManager` | 延迟批处理、两阶段更新（validation + repaint）、脏区域合并 |
| 渲染层 | `Figure` (paint) | 递归遍历树，绘制每个 Figure |
| 渲染输出 | `GraphicsSource` | 双缓冲（BufferedGraphicsSource）或直接 GC（NativeGraphicsSource） |

---

## 2. LightweightSystem

### 2.1 职责概述

`LightweightSystem` 是 SWT Canvas 到 Draw2d 的桥接层，核心职责：

1. **持有根引用**：`root` (RootFigure) + `contents` (用户 Figure)
2. **持有 UpdateManager**：默认 `new DeferredUpdateManager()`，可替换
3. **注册 SWT 事件监听**：Paint/Control/Mouse/Key → 事件分发
4. **平台资源管理**：创建/销毁 GraphicsSource

### 2.2 初始化流程

```java
// LightweightSystem.java

// 构造时自动初始化
public LightweightSystem() {
    init();
}

protected void init() {
    setRootPaneFigure(createRootFigure()); // 创建 RootFigure
}

protected void setRootPaneFigure(RootFigure root) {
    getUpdateManager().setRoot(root);     // ← 关键：UM 持有 root 引用
    this.root = root;
}

public void setControl(Canvas c) {
    canvas = c;
    // 根据是否双缓冲选择 GraphicsSource
    if ((c.getStyle() & SWT.DOUBLE_BUFFERED) != 0) {
        getUpdateManager().setGraphicsSource(new NativeGraphicsSource(canvas));
    } else {
        getUpdateManager().setGraphicsSource(new BufferedGraphicsSource(canvas));
    }
    getEventDispatcher().setControl(c);
    addListeners();                       // ← 注册所有 SWT 监听器
}
```

### 2.3 事件监听注册

```java
// LightweightSystem.java 第 94 行

protected void addListeners() {
    EventHandler handler = createEventHandler();
    // 辅助功能
    canvas.getAccessible().addAccessibleListener(handler);
    canvas.getAccessible().addAccessibleControlListener(handler);
    // 鼠标
    canvas.addMouseListener(handler);
    canvas.addMouseMoveListener(handler);
    canvas.addMouseTrackListener(handler);
    // 键盘
    canvas.addKeyListener(handler);
    canvas.addTraverseListener(handler);
    // 焦点
    canvas.addFocusListener(handler);
    // 生命周期
    canvas.addDisposeListener(handler);
    canvas.addListener(SWT.MouseWheel, handler);
    // 重绘
    canvas.addControlListener(new ControlAdapter() {
        public void controlResized(ControlEvent e) {
            LightweightSystem.this.controlResized();
        }
    });
    canvas.addListener(SWT.Paint, e -> paint(e.gc));  // ← 渲染触发入口
}
```

### 2.4 控制大小变化

```java
// LightweightSystem.java 第 119 行

protected void controlResized() {
    if (ignoreResize > 0) {
        return;
    }
    Rectangle r = new Rectangle(canvas.getClientArea());
    r.setLocation(0, 0);
    root.setBounds(r);                      // ← 同步 root Figure 尺寸
    root.revalidate();                     // ← 触发布局验证
    getUpdateManager().performUpdate();    // ← 强制同步更新
}
```

### 2.5 渲染触发入口

```java
// LightweightSystem.java 第 204 行

public void paint(GC gc) {
    getUpdateManager().paint(gc);          // ← 委托给 UpdateManager
}
```

### 2.6 依赖注入点

`LightweightSystem` 提供两个可替换的注入点：

```java
// 替换 UpdateManager（如测试 mock）
public void setUpdateManager(UpdateManager um) {
    manager = um;
    manager.setRoot(root);
}

// 替换 GraphicsSource（双缓冲策略）
public void setGraphicsSource(GraphicsSource gs) {
    manager.setGraphicsSource(gs);
}
```

---

## 3. UpdateManager

### 3.1 类层次

```text
UpdateManager (抽象基类)
├── DeferredUpdateManager  ← Draw2D 默认具体实现
└── SubordinateUpdateManager (deprecated, abstract) ← 嵌套 update manager 基类
```

`UpdateManager` 是公开抽象扩展点，不能从仓库内置类列表推出
`DeferredUpdateManager` 是“唯一可能实现”。

### 3.2 UpdateManager 抽象基类

```java
// UpdateManager.java

public abstract class UpdateManager {

    private final List<UpdateListener> listeners = new CopyOnWriteArrayList<>();
    private boolean disposed;

    // 核心抽象方法
    public abstract void addDirtyRegion(IFigure figure, int x, int y, int w, int h);
    public abstract void addInvalidFigure(IFigure figure);
    public abstract void performUpdate();
    public abstract void performUpdate(Rectangle exposed);  // 带暴露矩形
    public abstract void setGraphicsSource(GraphicsSource gs);
    public abstract void setRoot(IFigure figure);

    // 工具方法
    protected void firePainting(Rectangle damage, Map<IFigure, Rectangle> dirtyRegions);
    protected void fireValidating();
    public void performValidation() { performUpdate(); }  // 默认实现

    // 模板方法
    protected void paint(GC gc) {
        performUpdate(new Rectangle(gc.getClipping()));    // GC 裁剪区域作为暴露矩形
    }

    // 回调机制
    public void runWithUpdate(Runnable run) {}  // 更新完成后执行
}
```

### 3.3 DeferredUpdateManager 数据结构

```java
// DeferredUpdateManager.java

public class DeferredUpdateManager extends UpdateManager {

    private Rectangle damage;                            // 合并后的全局脏区域
    private Map<IFigure, Rectangle> dirtyRegions = new HashMap<>();  // figure → 脏区域
    private GraphicsSource graphicsSource;                // 渲染输出
    private final List<IFigure> invalidFigures = new ArrayList<>();  // 待验证 figure
    private IFigure root;                               // 根 Figure（UpdateManagerSource）
    private boolean updateQueued;                       // 防重复入队
    private boolean updating;                            // 防重入
    private boolean validating;                          // 防重入
    private List<Runnable> afterUpdate = new ArrayList<>();  // 更新后回调链
    private int refreshRate = -1;                      // 节流刷新间隔（ms）
}
```

### 3.4 双缓冲 swap

`repairDamage` 中使用 **双缓冲 swap** 避免在遍历中修改数据结构：

```java
// DeferredUpdateManager.java 第 272 行

protected void repairDamage() {
    // 立即创建新的空 map 接收下一轮脏区域
    Map<IFigure, Rectangle> oldRegions = dirtyRegions;
    dirtyRegions = new HashMap<>();

    oldRegions.forEach((figure, contribution) -> {
        // 验证过程中可能新增脏区域 → 进入新的 dirtyRegions map
        contribution.intersect(figure.getBounds());
        IFigure walker = figure.getParent();
        while (!contribution.isEmpty() && walker != null) {
            walker.translateToParent(contribution);
            contribution.intersect(walker.getBounds());
            walker = walker.getParent();
        }
        if (damage == null) {
            damage = new Rectangle(contribution);
        } else {
            damage.union(contribution);
        }
    });

    if (!oldRegions.isEmpty()) {
        firePainting(damage, oldRegions);  // 通知监听器
    }

    if (damage != null && !damage.isEmpty()) {
        Graphics graphics = getGraphics(damage);
        if (graphics != null) {
            root.paint(graphics);
            releaseGraphics(graphics);
        }
    }
    damage = null;
}
```

设计要点：

- `oldRegions` 被遍历的同时，`dirtyRegions` 已是新的空 map
- 验证过程中产生的脏区域会写入新 map，不干扰当前遍历
- `oldRegions.isEmpty()` 检查确保只在有脏区域时 firePainting

### 3.5 performUpdate 完整流程

```java
// DeferredUpdateManager.java 第 172 行

public synchronized void performUpdate() {
    if (isDisposed() || updating) {
        return;
    }
    updating = true;
    try {
        performValidation();               // Phase 1
        updateQueued = false;
        repairDamage();                   // Phase 2
        if (!afterUpdate.isEmpty()) {
            List<Runnable> chain = afterUpdate;
            afterUpdate = new ArrayList<>();  // swap
            chain.forEach(Runnable::run);     // 运行回调
            if (!afterUpdate.isEmpty()) {
                queueWork();                    // 回调中可能追加新回调
            }
        }
    } finally {
        updating = false;
    }
}
```

### 3.6 performValidation 原地清空

```java
// DeferredUpdateManager.java 第 198 行

public synchronized void performValidation() {
    if (invalidFigures.isEmpty() || validating) {
        return;
    }
    try {
        validating = true;
        fireValidating();
        for (int i = 0; i < invalidFigures.size(); i++) {
            IFigure fig = invalidFigures.get(i);
            invalidFigures.set(i, null);    // ← 原地置空
            fig.validate();                 // validate 过程可能新增 invalid figure
        }
    } finally {
        invalidFigures.clear();             // ← 遍历完后清空
        validating = false;
    }
}
```

设计要点：原地置空避免在遍历中追加新 figure 时漏掉或越界。

### 3.7 延迟批处理机制

```java
// DeferredUpdateManager.java 第 233 行

protected void queueWork() {
    if (!updateQueued) {
        sendUpdateRequest();
        updateQueued = true;
    }
}

protected void sendUpdateRequest() {
    Display display = Display.getCurrent();
    if (refreshRate <= 0) {
        display.asyncExec(new UpdateRequest());      // 立即异步
    } else {
        display.timerExec(refreshRate, new UpdateRequest());  // 节流定时
    }
}

protected class UpdateRequest implements Runnable {
    public void run() {
        performUpdate();
    }
}
```

### 3.8 performUpdate(Rectangle exposed)

```java
// DeferredUpdateManager.java 第 224 行

public synchronized void performUpdate(Rectangle exposed) {
    addDirtyRegion(root, exposed);   // 暴露区域当作 root 上的脏区域
    performUpdate();
}
```

用途：SWT 的 `gc.getClipping()` 返回裁剪区域，直接作为 root 的脏区域，复用 `repairDamage` 逻辑。

### 3.9 paint(GC) 模板方法

```java
// DeferredUpdateManager.java 第 138 行

protected void paint(GC gc) {
    if (!validating) {
        SWTGraphics graphics = new SWTGraphics(gc);
        if (!updating) {
            // 非更新期间收到 paint：通知监听器
            Rectangle rect = graphics.getClip(new Rectangle());
            HashMap<IFigure, Rectangle> map = new HashMap<>();
            map.put(root, rect);
            firePainting(rect, map);
        }
        performValidation();
        root.paint(graphics);
        graphics.dispose();
    } else {
        // validation 期间收到 paint：转为脏区域等下一轮
        addDirtyRegion(root, new Rectangle(gc.getClipping()));
    }
}
```

### 3.10 runWithUpdate 回调链

```java
// DeferredUpdateManager.java 第 315 行

public synchronized void runWithUpdate(Runnable runnable) {
    afterUpdate.add(runnable);
    if (!updating) {
        queueWork();
    }
}
```

---

## 4. Figure.paint 调用链

### 4.1 paint 完整流程

```java
// Figure.java 第 1250 行

public void paint(Graphics graphics) {
    // 1. 设置本地颜色/字体（如果存在）
    if (getLocalBackgroundColor() != null)
        graphics.setBackgroundColor(getLocalBackgroundColor());
    if (getLocalForegroundColor() != null)
        graphics.setForegroundColor(getLocalForegroundColor());
    if (getLocalFont() != null)
        graphics.setFont(getLocalFont());

    // 2. pushState：保存当前 Graphics 状态
    graphics.pushState();
    try {
        // 3. paintFigure：绘制自身主体
        paintFigure(graphics);

        // 4. restoreState：恢复颜色设置（只恢复颜色，不恢复变换/裁剪）
        graphics.restoreState();

        // 5. paintClientArea：绘制子节点
        paintClientArea(graphics);

        // 6. paintBorder：绘制边框
        paintBorder(graphics);
    } finally {
        // 7. popState：恢复完整状态（变换 + 裁剪 + 颜色）
        graphics.popState();
    }
}
```

### 4.2 paintFigure 实现

```java
// Figure.java

protected void paintFigure(Graphics graphics) {
    if (isOpaque()) {
        // 填充背景
        graphics.fillRectangle(getBounds());
    }
    // 子类可在此处绘制自身内容
}
```

### 4.3 paintClientArea 实现

```java
// Figure.java

protected void paintClientArea(Graphics graphics) {
    if (useLocalCoordinates()) {
        // 使用局部坐标：平移到 client area 原点
        Insets insets = getInsets();
        graphics.translate(getLocation().x + insets.left, getLocation().y + insets.top);
    }
    // clip 到 client area
    graphics.clipRect(getClientArea());
    graphics.pushState();
    try {
        paintChildren(graphics);    // 绘制子节点
    } finally {
        graphics.popState();
    }
}
```

### 4.4 paintChildren 实现

```java
// Figure.java

protected void paintChildren(Graphics graphics) {
    for (IFigure child : getChildren()) {
        if (!child.isVisible()) continue;
        // clip 到子节点 bounds
        graphics.clipRect(child.getBounds());
        // 递归绘制子节点
        child.paint(graphics);
    }
}
```

### 4.5 调用层次示意

```text
root.paint(graphics)
  │
  ├── pushState()
  ├── paintFigure()             ← 背景填充
  ├── restoreState()
  ├── paintClientArea()
  │     ├── translate(bounds)    ← 移到子区域坐标
  │     ├── clipRect(client)
  │     ├── pushState()
  │     └── paintChildren()
  │           ├── clipRect(child1.bounds)
  │           └── child1.paint(graphics)   ← 递归
  │                 ├── pushState()
  │                 ├── paintFigure()
  │                 ├── restoreState()
  │                 ├── paintClientArea()
  │                 │     └── paintChildren() ...  ← 递归
  │                 └── paintBorder()
  │                 └── popState()
  │
  ├── paintBorder()             ← 边框绘制
  └── popState()
```

---

## 5. Graphics 状态管理

### 5.1 State 结构

```java
// SWTGraphics.java

static class State extends LazyState {
    float[] affineMatrix;       // 2D 仿射变换矩阵
    int alpha;                  // 透明度 0-255
    Pattern bgPattern;          // 背景图案
    Pattern fgPattern;          // 前景图案
    int dx, dy;                // 平移量（整数）
    int graphicHints;           // 位掩码：AA/XOR/interpolation/fillRule
    Clipping relativeClip;     // 相对裁剪区域
}
```

### 5.2 三状态模型

`SWTGraphics` 内部维护三个状态对象：

| 状态 | 字段 | 含义 |
|------|------|------|
| `currentState` | `State` | Graphics 的当前逻辑状态 |
| `appliedState` | `LazyState` | 已应用到底层 GC 的状态（延迟应用） |
| `stack[]` | `List<State>` | 状态栈，支持 push/pop |

### 5.3 pushState / popState / restoreState 语义

```java
// pushState: 复制当前状态到栈顶
public void pushState() {
    currentState.dx = translateX;
    currentState.dy = translateY;
    if (stack.size() > stackPointer) {
        s = stack.get(stackPointer);
        s.copyFrom(currentState);       // 复用已有对象
    } else {
        stack.add(currentState.clone());  // 克隆新对象
    }
    stackPointer++;
}

// restoreState: 恢复状态，不改变 stackPointer
protected void restoreState(State s) {
    setAffineMatrix(s.affineMatrix);
    currentState.relativeClip = s.relativeClip;
    setBackgroundColor(s.bgColor);
    setForegroundColor(s.fgColor);
    setAlpha(s.alpha);
    setLineAttributes(s.lineAttributes);
    setFont(s.font);
    setGraphicHints(s.graphicHints);
    translateX = currentState.dx = s.dx;
    translateY = currentState.dy = s.dy;
}

// popState: 恢复 + stackPointer--
public void popState() {
    stackPointer--;
    restoreState(stack.get(stackPointer));
}
```

**语义区分**：

- `restoreState()`: 从栈顶下方读取状态，恢复到 currentState，**不改变 stackPointer**
- `popState()`: 恢复 + `stackPointer--`，成对操作

### 5.4 clipRect 裁剪机制

```java
// clipRect: 与当前裁剪区域求交集
public void clipRect(Rectangle rect) {
    currentState.relativeClip.intersect(rect.x, rect.y, rect.right(), rect.bottom());
    appliedState.relativeClip = null;  // 标记需要重新应用
}
```

### 5.5 translate 平移

```java
// translate: 更新平移量
public void translate(int dx, int dy) {
    if (currentState.relativeClip != null) {
        currentState.relativeClip.translate(dx, dy);  // 更新裁剪矩形坐标
        appliedState.relativeClip = null;
    }
    translateX += dx;
    translateY += dy;
}
```

---

## 6. EventDispatcher

`EventDispatcher` 负责将 SWT 事件路由到对应的 Figure，是 Draw2d 的**输入处理核心**。

### 6.1 类层次

```text
EventDispatcher (抽象基类)
└── SWTEventDispatcher     ← SWT 平台实现
```

### 6.2 鼠标目标确定（receive）

```java
// SWTEventDispatcher.java

private void receive(MouseEvent me) {
    currentEvent = null;
    updateFigureUnderCursor(me);
    if (captured) {
        if (mouseTarget != null) {
            currentEvent = new MouseEvent(this, mouseTarget, me);
        }
    } else {
        IFigure f = root.findMouseEventTargetAt(me.x, me.y);
        if (f == mouseTarget) {
            if (mouseTarget != null) {
                currentEvent = new MouseEvent(this, mouseTarget, me);
            }
            return;
        }
        if (mouseTarget != null) {
            currentEvent = new MouseEvent(this, mouseTarget, me);
            mouseTarget.handleMouseExited(currentEvent);
        }
        setMouseTarget(f);
        if (mouseTarget != null) {
            currentEvent = new MouseEvent(this, mouseTarget, me);
            mouseTarget.handleMouseEntered(currentEvent);
        }
    }
}
```

### 6.3 鼠标事件分发

```java
// SWTEventDispatcher.java

// mouseDown
public void dispatchMousePressed(MouseEvent e) {
    receive(e);
    if (mouseTarget != null) {
        mouseTarget.handleMousePressed(currentEvent);
        if (currentEvent.isConsumed()) {
            setCapture(mouseTarget);   // handler 消费 press 后才建立捕获
        }
    }
}

// mouseUp
public void dispatchMouseReleased(MouseEvent e) {
    receive(e);
    if (mouseTarget != null) {
        mouseTarget.handleMouseReleased(currentEvent);
    }
    releaseCapture();
    receive(e);                    // 释放后重新计算 hover/mouse target
}

// mouseMove
public void dispatchMouseMoved(MouseEvent e) {
    receive(e);
    if (mouseTarget != null) {
        if ((e.stateMask & SWT.BUTTON_MASK) != 0) {
            mouseTarget.handleMouseDragged(currentEvent);
        } else {
            mouseTarget.handleMouseMoved(currentEvent);
        }
    }
}
```

### 6.4 鼠标捕获（Capture）

- **设置捕获**：`setCapture(figure)` 后，`captured = true`，所有鼠标事件绕过 `findFigureAt`，直接发给捕获目标
- **释放捕获**：`releaseCapture()` 后恢复正常分发
- **典型场景**：拖拽操作中按下鼠标时捕获，释放时解除

### 6.5 焦点管理

```java
// SWTEventDispatcher.java

protected void setFocus(IFigure fig) {
    FocusEvent fe = new FocusEvent(focusOwner, fig);
    IFigure oldOwner = focusOwner;
    focusOwner = fig;
    if (oldOwner != null) {
        oldOwner.handleFocusLost(fe);   // 旧焦点 figure 收到 lost
    }
    if (fig != null) {
        fig.handleFocusGained(fe);      // 新焦点 figure 收到 gained
    }
}
```

Tab 遍历由 `FocusTraverseManager` 管理，实现 `getNextFocusableFigure` / `getPreviousFocusableFigure`。

### 6.6 键盘事件分发

```java
// keyboard shortcuts → focusOwner
public void dispatchKeyPressed(KeyEvent e) {
    if (focusOwner != null) {
        focusOwner.handleKeyPressed(new KeyEvent(this, focusOwner, e));
    }
}

public void dispatchKeyReleased(KeyEvent e) {
    if (focusOwner != null) {
        focusOwner.handleKeyReleased(new KeyEvent(this, focusOwner, e));
    }
}

// Tab traversal
public void dispatchKeyTraversed(TraverseEvent e) {
    IFigure next = getFocusTraverseManager()
        .getNextFocusableFigure(root, focusOwner);
    if (next != null) {
        e.doit = false;
        setFocus(next);
    }
}
```

### 6.7 鼠标滚轮

```java
public void dispatchMouseWheelScrolled(Event e) {
    MouseEvent me = new MouseEvent(e);
    receive(me);
    if (mouseTarget != null) {
        mouseTarget.handleMouseWheelScrolled(currentEvent);
    }
}
```

### 6.8 事件流程图

```text
SWT Canvas 事件
       │
       ▼
LightweightSystem.EventHandler.handleEvent()
       │
       ▼
SWTEventDispatcher.dispatch*(event)
       │
       ├─ MouseDown  ───► receive() → findMouseEventTargetAt()
       │                      │
       │                      ├── target.handleMousePressed()
       │                      └── 若事件被消费则 setCapture(target)
       │
       ├─ MouseUp    ───► target.handleMouseReleased()
       │                      │
       │                      └── releaseCapture()
       │
       ├─ MouseMove  ───► receive() → findMouseEventTargetAt()
       │                      └── 有按键则 dragged，否则 moved
       │
       ├─ MouseEnter/Exit
       │                      └── target.handleMouseEntered/Exited()
       │
       ├─ KeyDown    ───► focusOwner.handleKeyPressed()
       │
       ├─ KeyUp      ───► focusOwner.handleKeyReleased()
       │
       ├─ Traverse   ───► FocusTraverseManager → setFocus(next)
       │
       ├─ FocusGain  ───► oldOwner.handleFocusLost()
       │                      └── newOwner.handleFocusGained()
       │
       ├─ FocusLost   ───► focusOwner.handleFocusLost()
       │
       ├─ MouseWheel ───► mouseTarget.handleMouseWheelScrolled()
       │
       └─ Dispose    ───► updateManager.dispose()
```

---

## 7. 完整管线图

```text
┌────────────────────────────────────────────────────────────────────────────────┐
│                              SWT Canvas                                         │
│                                                                                │
│  SWT.Paint Event ──── canvas.addListener(SWT.Paint, paint)                  │
│  resize Event   ──── canvas.addControlListener → controlResized()           │
│  MouseEvent     ──── canvas.addMouseListener → EventHandler                  │
│  KeyEvent       ──── canvas.addKeyListener → EventHandler                   │
└────────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                           LightweightSystem                                   │
│                                                                                │
│  paint(GC gc)         ────► getUpdateManager().paint(gc)                    │
│  controlResized()     ────► root.setBounds() + root.revalidate()            │
│                           + getUpdateManager().performUpdate()                 │
│  setControl(Canvas c) ────► 选择 BufferedGraphicsSource 或 NativeGS         │
│  setUpdateManager(um) ────► um.setRoot(root)                                │
│  RootFigure.getUpdateManager() ──► LightweightSystem.this.manager           │
└────────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                         UpdateManager (抽象基类)                               │
│                                                                                │
│  paint(GC)              ────► performUpdate(new Rectangle(gc.getClipping()))    │
│  performUpdate()        ────► performValidation() + repairDamage()           │
│  performUpdate(rect)    ────► addDirtyRegion(root, rect) + performUpdate()   │
│  performValidation()    ────► fig.validate() for each invalid figure           │
│  repairDamage()         ────► dirty region merge + root.paint(graphics)      │
│  fireValidating()       ────► listeners.notifyValidating()                    │
│  firePainting()         ────► listeners.notifyPainting(damage, regions)       │
│  runWithUpdate(r)       ────► afterUpdate.add(r) + queueWork()             │
└────────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                       DeferredUpdateManager                                    │
│                                                                                │
│  addDirtyRegion()      ────► dirtyRegions.put(figure, rect.union())          │
│                           + queueWork()                                        │
│  addInvalidFigure()   ────► invalidFigures.add(f) (去重)                      │
│                           + queueWork()                                        │
│  queueWork()          ────► sendUpdateRequest()                               │
│                           └── Display.asyncExec(UpdateRequest)                 │
│                              或 Display.timerExec(refreshRate, UpdateRequest)  │
│                                                                                │
│  performValidation()  ────► 原地清空 invalidFigures                           │
│                           + fig.validate()                                     │
│  repairDamage()       ────► oldRegions/dirtyRegions swap                      │
│                          脏区域向父级传播 + 交集裁剪                            │
│                           Graphics = graphicsSource.getGraphics(damage)         │
│                           root.paint(graphics)                                │
│                           releaseGraphics(graphics)                            │
└────────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                           Figure Tree                                          │
│                                                                                │
│  root.paint(graphics)                                                          │
│    │                                                                          │
│    ├── pushState()                                                             │
│    ├── paintFigure()         ← 背景填充（若 opaque）                          │
│    ├── restoreState()        ← 仅恢复颜色，不恢复变换                          │
│    ├── paintClientArea()                                                      │
│    │     ├── translate(bounds)                                                  │
│    │     ├── clipRect(client)                                                 │
│    │     ├── pushState()                                                      │
│    │     └── paintChildren()                                                  │
│    │           └── for each child:                                            │
│    │                 ├── clipRect(child.bounds)                               │
│    │                 └── child.paint(graphics)  ← 递归                        │
│    │                                                                           │
│    ├── paintBorder()         ← 边框绘制                                        │
│    └── popState()                                                            │
└────────────────────────────────────────────────────────────────────────────────┘
                                      │
                                      ▼
┌────────────────────────────────────────────────────────────────────────────────┐
│                         GraphicsSource                                         │
│                                                                                │
│  BufferedGraphicsSource:                                                       │
│    getGraphics(damage)  ────► 创建离屏 Image + Image GC                      │
│                              返回 SWTGraphics(imageGC)                          │
│    flushGraphics(damage) ────► controlGC.drawImage(imageBuffer)               │
│                              + dispose image/imageGC/controlGC                  │
│                                                                                │
│  NativeGraphicsSource:                                                         │
│    getGraphics(damage)  ────► 返回 SWTGraphics(controlGC)                      │
│    flushGraphics(damage) ────► NOP（GC 已是直接目标）                         │
└────────────────────────────────────────────────────────────────────────────────┘

┌────────────────────────────────────────────────────────────────────────────────┐
│                       EventDispatcher (输入处理)                                │
│                                                                                │
│  Canvas 事件 ────► LightweightSystem.EventHandler                             │
│                        │                                                       │
│                        ▼                                                       │
│                   SWTEventDispatcher                                            │
│                        │                                                       │
│                        ├── dispatchMousePressed()                              │
│                        │     receive() → findMouseEventTargetAt()               │
│                        │     target.handleMousePressed() → consumed 后 capture  │
│                        │                                                       │
│                        ├── dispatchMouseReleased()                              │
│                        │     target.handleMouseReleased()                       │
│                        │     releaseCapture()                                  │
│                        │                                                       │
│                        ├── dispatchMouseMoved()                                │
│                        │     receive() → findMouseEventTargetAt()               │
│                        │     target.handleMouseDragged/Moved()                  │
│                        │                                                       │
│                        ├── dispatchMouseEntered/Exited()                        │
│                        │     target.handleMouseEntered/Exited()                │
│                        │                                                       │
│                        ├── dispatchKeyPressed()                                 │
│                        │     focusOwner.handleKeyPressed()                      │
│                        │                                                       │
│                        ├── dispatchKeyTraversed()                               │
│                        │     FocusTraverseManager → setFocus(next)             │
│                        │                                                       │
│                        ├── dispatchFocusGained/Lost()                          │
│                        │     oldOwner.handleFocusLost()                        │
│                        │     newOwner.handleFocusGained()                      │
│                        │                                                       │
│                        └── dispatchMouseWheelScrolled()                        │
│                              target.handleMouseWheelScrolled()                  │
└────────────────────────────────────────────────────────────────────────────────┘
```

---

## 8. 关键设计原则

### 8.1 更新与绘制分离

Validation（布局）总是在 Repaint（重绘）之前执行，且合并批量处理。

```text
performUpdate()
  ├── performValidation()  ─── 先执行所有 figure.validate()
  │     可能新增 invalid figures（追加到队列尾部）
  │     可能新增 dirty regions（写入新的 dirtyRegions map）
  │
  └── repairDamage()        ─── 再重绘合并后的脏区域
```

### 8.2 脏区域父级传播

`repairDamage` 中每个脏区域沿父链向上传播，每层取交集：

```text
子节点脏区域 (x=10, y=10, w=50, h=50)
      │
      ▼ intersect(figure.bounds)          → 裁剪到自身边界
      ▼ translateToParent()                → 转换到父坐标
      ▼ intersect(parent.bounds)          → 裁剪到父边界
      ▼ ... (继续向上直到 root)
      ▼ union()                           → 合并到总脏区域
```

保证子节点不会在其祖先 bounds 之外绘制。

### 8.3 双缓冲 swap

脏区域和失效块队列在遍历时使用 swap 模式，避免读写冲突：

```text
repairDamage() 中:
  oldRegions = dirtyRegions
  dirtyRegions = new HashMap<>()    ← 立即创建新 map
  // 遍历 oldRegions 的同时，新脏区域写入新 map

performValidation() 中:
  for (i = 0; i < size; i++) {
      invalidFigures.set(i, null)   ← 原地置空
      fig.validate()                 ← validate 可能新增 invalid figures
  }
  invalidFigures.clear()             ← 遍历后清空
```

### 8.4 延迟批处理

`queueWork()` 通过 `Display.asyncExec` 将更新延迟到下一个 UI 线程空闲时刻，自然合并同一事件循环内的多次修改。

### 8.5 防重入

`updating` / `validating` 标志防止 `performUpdate` 和 `performValidation` 重入：

```java
if (isDisposed() || updating) return;  // guard
validating = true;                      // 保护 performValidation 重入
```

### 8.6 三状态 Graphics

`pushState` / `restoreState` / `popState` 三操作分离颜色恢复和完整状态恢复：

```text
paint() 中:
  pushState()        ──── 保存完整状态（变换 + 裁剪 + 颜色）
  paintFigure()       ──── 绘制自身（可能修改颜色）
  restoreState()      ──── 恢复颜色设置（保留变换）
  paintClientArea()   ──── 绘制子节点（在子坐标中）
  paintBorder()       ──── 绘制边框
  popState()          ──── 恢复完整状态（变换 + 裁剪 + 颜色）
```

### 8.7 事件分发策略

- **鼠标按下时设置捕获**：后续所有鼠标事件绕过 `findFigureAt`，直到释放
- **焦点所有者处理键盘**：只有持有焦点的 Figure 收到键盘事件
- **Tab 遍历通过 FocusTraverseManager**：支持自定义焦点顺序

---

## 参考资料

| 文件 | 路径 |
|------|------|
| LightweightSystem | `org.eclipse.draw2d/src/org/eclipse/draw2d/LightweightSystem.java` |
| UpdateManager | `org.eclipse.draw2d/src/org/eclipse/draw2d/UpdateManager.java` |
| DeferredUpdateManager | `org.eclipse.draw2d/src/org/eclipse/draw2d/DeferredUpdateManager.java` |
| SubordinateUpdateManager | `org.eclipse.draw2d/src/org/eclipse/draw2d/SubordinateUpdateManager.java` |
| Figure | `org.eclipse.draw2d/src/org/eclipse/draw2d/Figure.java` |
| SWTGraphics | `org.eclipse.draw2d/src/org/eclipse/draw2d/SWTGraphics.java` |
| GraphicsSource | `org.eclipse.draw2d/src/org/eclipse/draw2d/GraphicsSource.java` |
| BufferedGraphicsSource | `org.eclipse.draw2d/src/org/eclipse/draw2d/BufferedGraphicsSource.java` |
| SWTEventDispatcher | `org.eclipse.draw2d/src/org/eclipse/draw2d/SWTEventDispatcher.java` |

源码基线：eclipse/gef-classic commit `4463d9d0c`（2026-01-01）。
