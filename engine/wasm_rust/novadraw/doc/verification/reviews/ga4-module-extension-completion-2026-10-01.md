# GA-4 模块与扩展表面完成记录

类型：`verification`

日期：2026-10-01

状态：`complete`

实现基线：`c95272f`

规范：
[Runtime 所有权与模块协作边界](../../design/architecture/runtime-ownership-and-module-boundaries.md)。

## 1. 完成范围

GA-4 保持六个公开 package、Runtime 单一提交权威和 Viewer 单一投影 owner，不新增
共享可变服务。内部职责已拆分为：

- FigureTree topology、query、layout/measurement、presentation；
- Runtime component update、connection service、frame/resource、frame submission；
- Viewer containment/connection projection。

`FigureEditor` 保留通用节点操作与 typed component update。Label、Clickable、
Image、PointList、ScalablePolygon、TextFlow、Border 和专用图形通过受检 capability
editor 更新，不公开 downcast 或任意 mutation closure。

## 2. 外部扩展证据

`ga4.extension-consumers` 使用公开 package API 覆盖两个独立消费者：

1. 外部 Figure、Layout、文本布局与 callback deferred component update；
2. 外部 Editor compound visual，在模型 refresh 中更新 Part 自有私有组件，并拒绝
   非本 Part visual。

执行结果：

```text
d4_component_update: 6 passed
d4_constrained_measurement: 2 passed
m10_text_extension_contract: 2 passed
ga4_external_visual_component: 2 passed
```

消费者没有依赖 `pub(crate)`、测试 helper 或扩张 Core 枚举。

## 3. 投影与职责迁移验证

- presentation capability 搬迁前后实现逐字比对，仅四处相对模块路径变化；
- Viewer projection 搬迁前后实现逐字比对，仅五个父模块调用入口改为
  `pub(super)`；
- connection route/decoration 原语仍属于 connection service，没有混入 presentation；
- `g2.viewer-projection`：19 passed；
- `g5.1.connection-projection`：19 passed，底层 connection contract 15 passed；
- `cargo xtask check --quick`：PASS。

## 4. 性能复核

在同一 Apple M1 Pro、Rust 1.94.1、release profile 和同一 `Cargo.lock` 下，对
GA-4 前基线 `d320149` 与实现基线 `c95272f` 各运行三轮完整 `r8-perf`。14 个场景的
确定性工作量全部一致。

三轮 p50 median-of-medians 的最大回退为 4.6%，低于既有 15% 调查阈值；宽树、深树
10,000、routing、全量更新等主要场景持平或改善。个别 p95 存在本地调度尾噪声，
最大为深树 validation 10,000 的 17.0%，但该场景 p50 为 -1.8%，且节点访问结果不变，
没有形成一致的工作量或中位数回退。

原始报告位于本地：

```text
target/verification/reports/ga4-before-repeat-{1,2,3}.json
target/verification/reports/ga4-module-repeat-{1,2,3}.json
```

本复核只证明 GA-4 没有可归因的 headless CPU 回退，不更新 GA-2 的总体性能结论。
真实 present、input-to-present 和 WebGPU 浏览器预算仍未验证。

## 5. 结论

GA-4 的所有权契约、内部职责、外部扩展通路、Viewer 投影和性能回归检查已闭合。
GA-2 与 GA-3 的未完成项保持原状态，不因本阶段完成而升级。
