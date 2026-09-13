# ADR-015: 独立 Editor 框架边界

类型：`architecture-decision`

状态：`accepted`

日期：2026-09-13

## 背景

Draw2D Core 1.0 已完成 Figure、布局、更新、输入、Viewport、Layer、Connection、
文本、基础控件和 Accessibility 的核心语义。下一阶段需要在其上建立 GEF 风格的
图形编辑框架，而不是继续把 selection、业务模型和撤销历史写入 Figure Runtime。

GEF Classic 的核心事实是：

- EditPart 连接应用模型与 Figure；
- Viewer 管理 EditPart 生命周期、选择和 visual/model registry；
- Tool 把输入解释为 Request；
- EditPolicy 根据 Request 贡献 Command 和反馈；
- CommandStack 修改模型并管理 undo/redo；
- 模型通知驱动 EditPart 刷新 Figure。

这些职责不属于 Draw2D Figure 核心。

## 决策

### 1. 使用独立 `novadraw-editor` crate

新框架使用 `novadraw-editor`，而不是 `novadraw-gef`：

- 名称表达产品职责，不宣称 Eclipse GEF API 或二进制兼容；
- GEF Classic 是语义参考，不是待逐类翻译的 Java API；
- 未来若确有兼容层需求，可单独评估 `novadraw-gef-compat`。

`novadraw-editor` 依赖 `novadraw-scene` 和平台无关几何协议，不依赖 winit、DOM、
AppKit 或具体 RenderBackend。

### 2. 模型、控制器和 Figure 身份分离

编辑器同时存在三个身份域：

```text
application ModelId -> EditPartId -> FigureId
```

- ModelId 由应用定义并保持可持久化；
- EditPartId 只在所属 Editor/Viewer 生命周期内有效；
- FigureId 只在所属 Novadraw Runtime 内有效；
- registry 显式保存映射，禁止根据内存地址或对象引用推断身份。

Command 只能保存模型身份和模型数据，不保存活的 EditPartId 或 FigureId。Undo 后可
创建新的 EditPart 和 Figure，不承诺恢复旧运行时身份。

### 3. Editor selection 不进入 Figure Runtime

selection、primary selection 和 EditPart focus 由 Viewer 拥有。它们不写入
`NodeState`、`InteractionState` 或具体 Figure。

选择框、handle 和拖拽反馈使用独立 Figure layer 表达，通过 FigureTree 的规范坐标、
命中和生命周期协议管理。现有 `apps/native/editor-app` 的单选与提交后命令描边只
作为集成测试工具，不升级为框架实现。

### 4. 单 crate 起步

初期在一个 crate 内按逻辑模块组织：

```text
model -> part -> viewer -> domain
request -> policy -> command
input -> tool -> feedback
```

只有模块契约稳定、依赖单向、可独立测试或发布，并存在实际编译隔离价值时，才考虑
拆成 `novadraw-editor-core`、`novadraw-editor-tools` 等 crate。

### 5. 输入仲裁先定义后实现

编辑器必须支持 Figure-native widget 与 Editor Tool 共存。目标顺序是：

```text
normalized input
-> Figure dispatch
-> 若已消费或已 capture，则停止
-> active Tool
-> Request / targeting / feedback
-> CommandStack
```

当前 `Runtime::dispatch_*` 不公开 consumed outcome。G1/G2 必须用嵌入式 Button
场景验证是否需要增加 `DispatchOutcome` 或统一输入入口；在证据出现前不修改
Draw2D Runtime。

### 6. 先建立契约账本和垂直切片

GEF 能力使用独立 G0-G6 编号和语义覆盖账本，不复用 M1-M10。首个毕业切片必须覆盖：

- 模型加载与 EditPart/Figure 构建；
- selection；
- create、move、delete；
- connection create/reconnect；
- undo/redo；
- 保存、加载和重建后结果一致。

## 后果

### 正面

- Draw2D Core 保持通用，不被编辑器业务状态污染；
- 模型、控制器和显示对象的生命周期可以独立验证；
- Native、Web 和 Headless 可以复用同一编辑事务；
- Command history 不依赖活 Figure，可支持模型重建和跨 Runtime 恢复；
- crate 名称不会造成 GEF API 兼容承诺。

### 代价

- 需要维护 EditPart 树与 Figure 树之间的显式映射和一致性；
- 输入消费、Tool capture 与 Figure capture 需要新的仲裁契约；
- Rust 中异构模型、Command 和 EditPolicy 的对象安全边界必须通过真实用例固定；
- 在垂直切片完成前，不应把骨架 API 作为稳定公共接口发布。

## 验证

- G0：文档、语义账本、crate 骨架和 workspace 门禁；
- G1：CommandStack headless 契约；
- G2：模型通知、PartTree、Viewer registry 与生命周期；
- G3-G5：选择、工具、策略、反馈和连接编辑；
- G6：Native/Web/Headless 总验收。

路线图见 [`../07-gef-roadmap/00-index.md`](../07-gef-roadmap/00-index.md)，规范设计见
[`../design/editor/architecture.md`](../design/editor/architecture.md)。
