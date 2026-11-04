# 第三方类型与依赖规范

类型：`normative-design`

能力归属：Core 公共合同；backend／platform 各自管理实现依赖与 interop。
规范效力：沿用原编号 022（2026-09-29 已接受）中类型隔离与依赖检查合同；
Vello 拆包选择以 [ADR-023](../../adr/adr-023-crate-consolidation-and-extension-boundaries.md)为准。

## 公共类型边界

backend-neutral 公共签名不暴露 Kurbo、Vello、Winit 或内部数学库类型。
使用 Novadraw 几何值、SurfaceInfo 和自有领域错误。
后端构造通过明确的窗口句柄能力／interop 接入；平台 crate 的自身适配接口可以使用
平台类型，但不能将其反向带进 Core、Render IR、Scene 或 Editor。
host 和 wasm rustdoc 扫描负责 alias、重导出及条件编译签名；源码搜索不能替代它。

## 算法归属

| 用途 | owner | 输入输出 |
|---|---|---|
| 通用几何算法 | Core geometry 私有实现 | Novadraw 几何值与系数 |
| arc normalization／curve bounds | Core render 私有算法 | PathOp 与 Novadraw 几何值 |
| Vello path／stroke／clip／affine lowering | Vello backend | backend 私有 Vello／Kurbo 值 |

RenderCommand 与 PathOp 不持有 Kurbo 值。backend 不反向定义通用几何语义。
按能力分配算法能避免把全部第三方使用集中到 backend，也增加转换和边界检查成本。

## 依赖与失败检查

既有接受基线是直接 Kurbo 0.13.1、Resvg 0.48.1，依赖图收敛到同一 Kurbo 版本。
升级时检查完整 workspace feature／target dependency graph。
无法统一时先记录依赖链、影响和退出条件，再调整门禁；不能静默接受新主版本。
版本数字是依赖基线，不是永久公共 API。

`facade.dependencies` 检查 Core 不启用具体平台 renderer；
`api.third-party-types` 检查公开签名；`dependencies.kurbo` 检查版本集合。
初始化失败使用 Novadraw 自有错误，不把 Vello error 作为中立契约。
产物体积只在用户明确要求比较时采证，遵循 [项目规则](../../../CLAUDE.md#包体积与性能要求)；
原稿中的自动体积门禁不再适用。

原批准与依赖收口记录见
[原稿 022](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-022-third-party-type-and-render-dependency-boundary.md.txt)。
