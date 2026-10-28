# 目标一致性审计分组

范围：用户指定全项目；full_file，非增量 diff 审查。先读契约再查实现。
共享边界：AGENTS.md、CLAUDE.md、doc/00-index.md、architecture/overview.md、
ADR-014/017/018/019/020/021/022/023、verification/suites.toml、novadraw/src/lib.rs、
Cargo.toml。归档默认排除；第三方仅 org.eclipse.draw2d/org.eclipse.gef。

## Group 1：性能与更新/渲染

- doc/design/rendering/*、architecture/derived-state-convergence.md、resource-lifecycle.md
- doc/verification/performance/*、adr014-d4.5-performance-2026-09-10.md
- benchmarks/*、novadraw/src/graph/*、runtime/*、render/*、novadraw-backend-vello/*
- scripts/、xtask/ 中性能验证入口

重点：目标是否有可测判据；增量工作量、record/submit、性能证据能证明什么。

## Group 2：跨平台与宿主

- doc/design/architecture/tooltip-accessibility.md、doc/design/input/*
- doc/design/editor/p2-direct-text-edit.md、doc/verification/manual/*、M10/P2 平台审计
- novadraw-platform-{web,winit}/*、novadraw/src/{host,event}/*
- examples/{native,web,support}/*、平台构建与验证脚本

重点：四平台承诺、输入/IME、accessibility、host/backend 边界；待验收不等于缺陷。

## Group 3：扩展协议、模块与 API

- doc/design/architecture/{static-architecture,directory-structure,component-update,figure-lifecycle,connection-routing,text-layout,reusable-shape-border}.md
- doc/design/editor/*、doc/parity/gef/*
- novadraw/src/{figure,layout,connection,runtime,graph}/*、novadraw-editor/*、novadraw-inspector/*
- 对应外部组件/公共 API 契约测试、package manifests

重点：第三方 Figure/Layout/Router/TextLayoutEngine、事务失败边界、私有模块依赖。

## Group 4：文档治理、目标追踪与 Draw2D 分母

- doc/{design,adr,roadmap,parity,strategy}/*、README、book/src/*
- verification/suites.toml、xtask 文档检查与依赖门禁
- gef-classic/org.eclipse.draw2d 源码的能力入口

重点：目标到契约到 suite 的闭环、partial/deferred 的退出条件、设计状态与历史记录边界。

边界重叠有意保留：Runtime、Figure、host、render、suite 在不同组承担交叉契约。
原始工作区 diff 文件：Editor/View 路径归组 3；Native/Web 场景与 DOM 归组 2；
全部文档/manifest 归组 4。性能与 API 跨组问题由最终独立复核汇总。
