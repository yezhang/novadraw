# ADR-014 首批实现与 Runtime 重建门禁

类型：`verification`

日期：2026-09-10

状态：`complete`。本文保留原失败证据；2026-09-10 已按确认方案关闭同树重包装入口，
后续 D4.4 已完成扩展错误、约束测量与稳定通知门禁。

## 已实现范围

- 新增私有 RuntimeArena：SlotMap 仅保存 local key，公开 Figure/Anchor/Router handle
  带 namespace；公共 handle 不实现 slotmap::Key。
- Runtime 沿用 FigureTree namespace，资源、ConnectionRuntime、Listener 使用同域。
- 新增 dispose_subtree，释放 arena 节点、UUID、constraint 和运行时引用；将旧 damage
  在删除前投影并保留。
- remove_figure、remove_layer、contents replacement 使用销毁路径；子树失焦事件
  在提取后、对象释放前完成，避免丢失 FocusLost。
- Listener 注册时显式声明 Runtime/Figure scope；dispose 清理 Figure scope，并保留
  Runtime 订阅、共享资源与 Router。
- 捕获主要 frame/input/dispose 生命周期 panic，标记 faulted 并拒绝新帧。
- FigureTree builder 只构建拓扑；Runtime 接管后按 parent-before-children 激活。
  动态 add/reparent 在 topology 与 sidecar 提交后执行 completion；dispose 在提取和
  sidecar 清理后按 descendant-before-parent 停用，并传入 namespaced 只读上下文。
- render_recursive.rs 仅更换存储引用类型一行，没有改变遍历主循环。

本批之后的 mutation panic 路径、稳定查询 epoch、typed component update 与约束测量
已由 D4.4 完成验证，见
[`adr014-d4.4-increment-2026-09-10.md`](adr014-d4.4-increment-2026-09-10.md)。
没有性能收益声明。

## 已复现的原阻塞

证据入口：

- `Runtime::with_text_layout_engine`：沿用 tree namespace，但重建 ResourceRegistry、
  ConnectionRuntime、UpdateManager 并把 backend generation 从初始值开始。
- `Runtime::into_tree`：仅返回 tree，其他 Runtime-owned 状态被丢弃。
- `apps/scenes/src/connection.rs` 多个场景确实使用 Runtime 构建后 into_tree；
  此 API 并非不可达或仅测试路径。

最小序列：

```text
Runtime A 注册 image/listener/router，记录 session
-> A.into_tree() 丢弃 Runtime-owned registries
-> Runtime B = Runtime::new(tree)，沿用 namespace、重置 local counters
-> B 注册相同次序的对象
-> 四类新 ID 全部等于 A 的旧 ID
```

测试：
`runtime::runtime::adr014_tests::rebuilding_runtime_from_tree_does_not_reuse_discarded_registry_identity`

命令：

```bash
cargo test -p novadraw-scene --lib rebuilding_runtime_from_tree_does_not_reuse_discarded_registry_identity -- --nocapture
```

实际：`[true, true, true, true]`；期望：`[false, false, false, false]`，退出码 101。
这证明本批“树 namespace 与各 registry 同域”在现有重包装路径下有身份复活问题。
独立创建两个 Runtime 的隔离通过不覆盖这一场景。

## 已实施的契约修正

构建阶段使用 builder，交付时移交完整 Runtime；运行期禁止
`Runtime -> bare FigureTree -> fresh Runtime` 的有损重包装。

- 删除 `Runtime::into_tree`，仓库内不再存在该调用。
- `SceneFactory`、`SceneEntry`、`SceneSpec::build` 与 Native/Web 场景切换统一交付 Runtime。
- 纯 FigureTree 场景只在 `SceneSpec::new/visual` 内首次包装，不曾拥有 Runtime registry。
- Connection、M10 Runtime Mutation、Freeform 场景直接返回完整 Runtime。
- Freeform 的 view location/zoom 写入改经 Runtime API，不暴露 `tree_mut()`。
- 整体 move 回归验证 resource/listener/router/backend session 身份与状态保持。

单独导出场景应产出模型/描述，用工厂创建新对象与身份；不是携带旧句柄的活树。
不要通过随机换一个 registry namespace 掩盖丢失的资源/路由关系，也不要通过全局
计数器解决局部所有权缺陷。未来若确需拆分交接，应引入完整 owned parts；
当前不提供该能力。

## 测试与门禁结果

测试流程已读取 bits-unit-test-gen Step1-Step6；初始四项缺陷测试先失败，修复后通过。
原先新增 10 项测试覆盖独立 Runtime 隔离、删除释放、contents、owner scope、
共享对象保留、lifecycle panic、damage、reparent 和同树重包装。确认方案后删除
不可再表达的有损 API，用 `moving_runtime_preserves_registry_identity_and_state`
验证完整所有权移动。后续 lifecycle completion 测试覆盖 builder 零回调、
Runtime parent-before-children 激活、reparent 旧/新关系 completion、
dispose descendant-before-parent 停用、10,000 层完整销毁，以及 LayeredPane hook
panic 的 faulted 边界。

- 修订后 scene 258 项单测及全部集成/文档测试通过。
- demo-scenes 6 项测试通过，包含完整 catalog 构建、Freeform range/zoom/hit order。
- `cargo check --workspace`：通过。
- `cargo clippy --workspace -- -D warnings`：通过。
- `cargo check -p web-validation --target wasm32-unknown-unknown`：通过。
- `cargo test --workspace --quiet`：通过。
- `cargo run -p update-app -- --verify`：六项契约验证通过。
- 额外 `cargo clippy --workspace --all-targets -- -D warnings`：失败于原有测试 lint：
  graph 测试 type_complexity、border_layout/flow_layout 的 items_after_test_module、
  repair 测试的 useless_vec；未改动这些旧测试结构。
- `utree flush` 已调用，但工具更新技能目录被 sandbox 拒绝，退出码 1；
  测试与命令日志仍在工作区 target/adr014-*.log。
- `cargo fmt --all -- --check` 与 `git diff --check`：通过。
- 未执行 Native/Web 视觉验收，未创建 Git commit。

ADR-014 的 D4.3-D4.4 Core 1.0 门禁已完成；并发 session handoff activation token
仍属于未来并发交接范围，当前 Host 只承诺已验证的串行 handoff。
