# Crate 边界与发布结构审计

类型：`architecture-review`

状态：`proposal`

日期：2026-09-29

## 1. 审计范围

本审计依据当前规范架构、ADR 和 Cargo manifests 判断 crate 边界，不扫描 Rust 实现，
避免由现状目录反推长期架构。

主要依据：

- `doc/design/architecture/overview.md`
- `doc/design/architecture/static-architecture.md`
- `doc/design/architecture/directory-structure.md`
- ADR-003、ADR-014 至 ADR-022
- workspace 与各 package 的 `Cargo.toml`
- `cargo metadata --no-deps` 和 feature dependency tree

## 2. 先纠正判断维度

Cargo crate 不是部署单位。应区分四种边界：

| 边界 | 主要职责 |
|---|---|
| Rust module | 所有权、可见性和内部职责 |
| Cargo crate/package | 编译、依赖、feature、发布和 semver |
| facade | 用户入口与 API 导航 |
| binary/cdylib/wasm bundle | 部署制品 |

不能仅按“桌面部署”或“Web 部署”切 crate。只有当一组代码具有独立依赖、可选安装、
稳定接口、独立测试或发布价值时，才应成为 crate。平台无关代码也不必按每个逻辑层
拆 crate；module 足以表达大部分内部边界。

## 3. 当前结构审计

当前主依赖链为：

```text
novadraw-core + novadraw-geometry
              |
              v
       novadraw-render
              |
              v
        novadraw-scene
          /         \
         v           v
novadraw-editor  novadraw-inspector
         \           /
          \         /
           novadraw facade
```

实际 `novadraw` 默认依赖 `novadraw-editor`。因此“无 backend”不等于“只安装 Core”。

### 3.1 逐 crate 结论

| 当前 crate | 审计结论 | 处置建议 |
|---|---|---|
| `novadraw` | 仅作 facade，反向聚合全部引擎包且默认带 Editor | 改为真正的平台无关主引擎 crate |
| `novadraw-core` | 只承载少量基础值，名称大于职责 | 合并进 `novadraw` |
| `novadraw-geometry` | 无独立版本或外部复用证据，所有核心层共同依赖 | 合并为 `novadraw::geometry` |
| `novadraw-render` | 混合 Render IR、文本/图像服务和具体 Vello backend | 协议合并进 `novadraw`；Vello 提取为 backend crate |
| `novadraw-scene` | 实际承载 Figure、Layout、Tree、Runtime，是当前真正 Core | 合并进 `novadraw`，保留内部模块边界 |
| `novadraw-editor` | 独立、平台无关、依赖方向正确 | 保留独立 crate |
| `novadraw-inspector` | 可选诊断能力，具有独立进程和未来 wire protocol | 保留独立 crate |
| `novadraw-math` | 当前无 workspace consumer，3D 能力尚未进入主线 | 从主 workspace 移除；真实 3D 开始时再恢复 |
| `novadraw-apps` | 混合可复用 Winit adapter 与 demo shell | 提取平台 adapter；demo shell 留在 `apps/` |
| `novadraw-demo-scenes` | 测试和演示 fixture，不是框架 API | 保留 workspace 私有并设置 `publish = false` |

## 4. 推荐目标

### 4.1 用户可安装的主框架

```text
novadraw
├── geometry
├── graphics / render protocol
├── figure / layout / container / connection
├── tree
├── runtime / event / update / resources
└── host contracts

novadraw-editor
└── model / part / viewer / tool / request / policy / command / feedback
```

`novadraw` 应是 Core，而不是只做转发的 facade。`novadraw-editor` 是可选的图形编辑框架，
依赖 `novadraw`；`novadraw` 不反向依赖或重导出 Editor。普通绘图用户只安装
`novadraw`，编辑器用户再安装 `novadraw-editor`。

### 4.2 可选集成包

```text
novadraw-inspector
novadraw-backend-vello
novadraw-platform-winit
novadraw-platform-web
```

- `novadraw-inspector`：只读诊断模型，继续独立；
- `novadraw-backend-vello`：只实现 `RenderBackend` 和 backend-local resource/cache；
- `novadraw-platform-winit`：窗口、输入、IME、cursor、redraw 和 accessibility 适配；
- `novadraw-platform-web`：DOM/pointer/wheel/IME、canvas surface 和浏览器调度适配。

Winit 和 Web 不是渲染后端。它们是平台适配器；Vello 才是渲染后端。应用 composition
root 选择一组 platform + backend：

```text
Native app = novadraw + novadraw-platform-winit + novadraw-backend-vello
Web app    = novadraw + novadraw-platform-web   + novadraw-backend-vello
Headless   = novadraw + application test backend/host
```

### 4.3 Workspace 私有包

`apps/*`、demo scenes、benchmarks 和 `xtask` 是验证或部署制品，不作为框架 crate 发布。
所有这类 package 应显式设置 `publish = false`。

## 5. 关键问题结论

### 当前是不是整理 crate 的合适时机

是，但应只做一次有目标的 0.1 breaking migration：

- Core M1-M10 与 Editor G0-G5 已完成，行为契约已稳定；
- ADR-017 至 ADR-022 已收口公共 API、facade、feature 和第三方类型边界；
- workspace 调用方集中，尚未形成 1.0 兼容负担；
- 当前 facade、core、geometry、scene 的职责错位已经产生真实安装与维护成本。

不应继续按长期设计中的每个逻辑层拆成独立 crate。Figure、Layout、Tree、Runtime
之间共享事务和 crate-private primitive，拆开会迫使内部细节公开并增加 facade 转发。

### 是否只保留 Core 和 Editor 两个 crate

作为“主要框架产品”可以；作为“workspace 全部 crate”不可以。

- Core 应是 `novadraw`；
- Editor 应是独立 `novadraw-editor`；
- Inspector、backend 和 platform adapter 是正当的可选包；
- app、benchmark 和工具是部署/验证 package，不属于公开框架集合。

### Editor 是否需要改名

不建议改名。`novadraw-editor` 在 Novadraw 命名空间内已经表达“基于 Novadraw 的编辑
框架”，ADR-015 也明确拒绝了暗示 GEF 兼容的 `novadraw-gef`。更长的
`novadraw-editor-framework` 只增加包名和导入噪声。

应通过 package description、README 标题和 rustdoc 首页统一写明
“GEF-style graphical editor framework”，产品级编辑器应用使用自己的产品名。

### Inspector 是否应独立

应保持独立。它是非必需诊断能力，依赖稳定只读协议，拥有不同的发布节奏和进程边界，
未来还可能产生独立 wire protocol。现在不拆 `novadraw-inspector-protocol`；只有 IPC
协议实现并需要跨进程/版本兼容时再拆。

### Geometry 和 Math 是否并入 Core

- Geometry 应并入 `novadraw`。它是二维引擎的领域词汇，不是独立产品。
- Math 不应并入 Core。当前无消费者，且 3D 不属于二维核心；应移出主 workspace，
  在真实 Scene3D 工作开始后作为明确能力重新引入。

## 6. 对 ADR-022 的修订建议

ADR-022 的“第二个生产 backend 出现后再拆 Vello crate”适合单独讨论 backend 时采用。
如果本次执行整体 crate 收口，则应在同一迁移中提取 Vello，避免先把 Vello 从
`novadraw-render` 移入 `novadraw`，之后再搬第二次。

此处拆分依据不是假设中的第二 backend，而是已经存在的：

- 稳定 `RenderBackend` / `RenderSubmission` 边界；
- 明确的 heavy dependency 和 target-specific dependency 集；
- native、web、headless 独立验证；
- 用户不安装 GPU backend 的真实需求；
- Core 合并带来的唯一迁移窗口。

该变化需要新 ADR 替代 ADR-021 的“facade 聚合 Editor”和 ADR-022 的暂缓拆分条款。

## 7. 迁移顺序

1. 新增 crate consolidation ADR，冻结目标依赖图和 public path。
2. 提取 `novadraw-backend-vello`，保持当前 RenderBackend 契约不变。
3. 从 `apps/support` 提取 `novadraw-platform-winit`；从 Web app 提取通用 Web adapter。
4. 将 Core、Geometry、Render protocol、Scene/Runtime 合并进 `novadraw` 内部模块。
5. 改为 `novadraw-editor -> novadraw`，删除 `novadraw -> novadraw-editor`。
6. 改为 `novadraw-inspector -> novadraw`，保持 Editor adapter 在宿主侧。
7. 从 workspace 移除 `novadraw-math`；将 apps、fixtures、benchmarks 标记为不发布。
8. 删除旧 crate，不保留 0.1 阶段的兼容转发壳。

每一步独立提交，但只在完整迁移闭合后对外发布，避免中间依赖图成为可消费状态。

## 8. 验收门禁

- `cargo tree -p novadraw` 不包含 Editor、Inspector、Winit、WebSys 或 Vello；
- `novadraw-editor` 和 `novadraw-inspector` 仅单向依赖 `novadraw`；
- platform crates 不拥有 FigureTree、Runtime 或 RenderBackend 语义；
- backend crate 不依赖 platform adapter 或应用 crate；
- Native、Web、Headless 三种 composition root 使用同一 `novadraw`；
- 公共 API 第三方类型、Kurbo 单版本、二进制体积门禁继续通过；
- Core、Editor、Inspector、Native、Web 与 full workspace suite 全部通过。

## 9. 审计结论

推荐批准本次 crate consolidation。目标不是“把逻辑层都拆开”，而是：

1. 把平台无关引擎收成一个真正的 `novadraw`；
2. 把 Editor 保持为一个可选框架；
3. 把 Inspector、backend、platform adapter 保持为真正独立的可选能力；
4. 删除无消费者的 Math；
5. 让 app/package/deployment 三种边界重新一致。
