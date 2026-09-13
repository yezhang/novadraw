# G2 EditPart Tree 与 Viewer 投影完成记录

类型：`verification`

日期：2026-09-13

状态：`complete`

## 1. 范围

G2 建立应用模型到 EditPart tree 和 FigureTree 的可重建 containment 投影，不引入
selection、Tool、EditPolicy 或 connection editing。

受影响 API family：

- `model.notification`；
- `part.identity`、`part.tree`、`part.factory`、`part.lifecycle`；
- `part.visual`、`part.refresh`；
- `viewer.contents_root`、`viewer.registry`、`viewer.targeting`。

## 2. 已固定契约

- `EditPartId` 使用 Viewer namespace 与 generational key；
- synthetic root part 不对应业务模型，单 contents part 对应 adapter root；
- `PartTree`、模型 containment 与 FigureTree 保持独立身份；
- factory 创建 behavior，behavior 只通过受限 context 创建和刷新 visual；
- primary Figure、content pane 与全部 owned visual 显式注册；
- 模型快照在 mutation 前验证 duplicate identity 和最大深度；
- refresh 接受同 revision 的有序多事件批次，拒绝 stale、gap 和 root replacement；
- children 按模型顺序复用、重排、新建、移除；跨父级移动退休旧身份后重建；
- remove 和 Viewer drop 按父先于子执行 deactivate；
- Figure 命中结果可以沿 ancestor registry 回溯到 owner part；
- structural 或 extension failure 使 Viewer faulted，后续不再接受 refresh。

## 3. Connection 阶段边界

旧 G2 清单同时要求 connection refresh，但 `connection.part` 在 parity 账本属于 G5，
且目标 visual 必须进入 G3 才建立的 connection layer。为避免临时挂载协议，G2 只
闭合 containment；source/target 双向发现、单 ConnectionPart 去重、layer 挂载与
Runtime binding 在 G5 一次闭合。

## 4. 输入仲裁结论

G2 对 `Runtime` 的输入路径复核确认：

- `EventDispatcher` 内部已获得 handler 的 `bool handled`；
- `Runtime::dispatch_mouse_*` 和 `dispatch_key_*` 对外只返回 `()`；
- pressed 后的 capture 只能间接证明部分 handled 情况，不能表达所有事件的消费结果。

因此 G3 的 Figure/Tool 仲裁需要一个明确的 Draw2D Core P2 delta，为 Runtime 暴露
最小 `DispatchOutcome`；G2 不提前修改输入协议。

## 5. 自动验证

`novadraw-editor/tests/g2_viewer_projection_contract.rs` 共 15 项，覆盖：

- 初始递归投影、root/contents、model/visual registry；
- compound visual content pane 与 ancestor owner 查询；
- namespace 隔离和 foreign/unknown visual；
- reuse/reorder/create/remove/reparent；
- retire 后 Part/Figure 身份失效，重新出现时生成新身份；
- activate/deactivate 生命周期与 Viewer drop；
- duplicate model 在初始构建和 refresh 前置验证；
- 初始快照事件吸收、revision gap、stale replay、同 revision 多事件批次；
- visual 属性刷新。

最终门禁：

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
git diff --check: PASS
```

补充执行的 `cargo clippy --workspace --all-targets -- -D warnings` 暴露 4 项既有
`novadraw-scene` 测试 lint（type complexity、items after test module、useless vec）。
它们不在本次 G2 变更范围内；仓库规定的非 all-targets Clippy 门禁通过。
