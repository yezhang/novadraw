# Novadraw 设计入口

类型：`documentation-index`

规范效力由已接受 ADR 和 `normative-design` 专题决定，实现状态由
[路线图](../roadmap/00-index.md)维护。冲突和替代规则见
[知识维护](../knowledge-maintenance.md)。

## 按职责进入

按职责选专题；总体边界不重复具体行为，范围更窄的专题优先。

| 入口 | 内容 |
|---|---|
| [Core 架构](architecture/00-index.md) | 公共 API、所有权、树、Figure 能力、资源与组件 |
| [坐标与变换](coordinates/coordinate-system.md) | 坐标域、变换、命中、事件点及 damage |
| [输入手势](input/scroll-zoom-gesture-contract.md) | Scroll/Zoom 分发、固定 session target 与 fallback |
| [渲染专题](rendering/00-index.md) | 更新、Graphics、文字及图像 |
| [动画专题](animation/00-index.md) | 计划、触发、表现平面、领域集成 |
| [Editor 架构](editor/00-index.md) | 模型、命令、投影、连接编辑及 IME |

## 核心设计

- [公共 API 契约](architecture/public-api-contract.md)；
- [派生状态收敛](architecture/derived-state-convergence.md)；
- [深层布局与表现](architecture/deep-layout-presentation-convergence.md)；
- [Figure Capability](architecture/figure-capability-model.md)；
- [组件更新](architecture/component-update.md)；
- [UpdateManager](rendering/update-manager.md)。

## 非规范提案

- [DisplayList 候选协议及探索门禁](rendering/display-list-protocol.md)：休眠候选，
  wire format、chunk patch 和远程传输尚未成为 Core 契约。

旧 API 综合提案已由公共 API 规范及领域专题承接，原稿从
[历史快照](../archive/knowledge-revision-2026-10-19/README.md)追溯。
