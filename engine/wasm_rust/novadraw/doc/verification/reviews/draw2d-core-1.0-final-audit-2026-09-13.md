# Draw2D Core 1.0 最终审计

类型：`verification`

日期：2026-09-13

状态：`complete`

代码基线：`f1ade0f`（包含 M10 Web 代码提交 `40635a2`）

## 1. 结论

M1-M10 的契约层、产品层、Native/Web 端到端入口和文档状态均已闭合。P0/P1 API
family 不存在未解释的 `missing`，公开产品 API 未发现静默 no-op 或占位成功。
ADR-014 的身份域、生命周期、外部扩展、资源因果、稳定通知和递归性能均有当前测试
证据。R9.4 capability 消融复查完成。

Draw2D Core 1.0 完成门禁通过。

## 2. 自动门禁

```text
cargo fmt --all -- --check: PASS
cargo check --workspace: PASS
cargo clippy --workspace -- -D warnings: PASS
cargo test --workspace: PASS
./scripts/build_web_validation.sh: PASS
```

关键测试结果：

- `novadraw-scene` library：262 passed；
- M10 reusable shape/border：12 passed；
- M10 label/image：8 passed；
- M10 widget：10 passed；
- M10 tooltip：3 passed；
- M10 accessibility：4 passed；
- M9 connection contract/runtime：12 + 17 passed；
- M8 viewport：24 passed；
- ADR-014 component update、constrained measurement、notification epoch 全部通过；
- 10,000 层 dispose 与 accessibility projection 通过。

## 3. Headless 总审计

```text
update-app --verify: 6/6 PASS
event-app --verify: 4/4 PASS
scroll-pane-demo --verify: 7/7 PASS
```

覆盖 damage、validation、通知、panic recovery、1,024 Figure、submission lifecycle、
capture/focus/key/wheel、坐标根降域、scroll/zoom、pinch anchor、freeform range 与 layer
hit order。

## 4. macOS / Native 视觉与人工证据

当前基线重新生成并正常退出：

| Demo | 场景数 | 结果 |
|---|---:|---|
| Layout | 11 | PASS |
| Event | 4 | PASS |
| Viewport | 4 | PASS |
| Connection | 8 | PASS |
| Text / Image | 6 | PASS |
| Widget / Tooltip | 5 | PASS |

代表性截图复核：

- root viewport 五区布局完整，无空白或重叠；
- shared Manhattan 保持正交、箭头与 lane 分离正确；
- Image Resources 的 PNG、SVG、Pending、Failed 四态完整；
- Bottom edge Tooltip 上翻并 clamp 到 surface；
- 所有截图非空，背景、文字、边框和资源均正常。

`widgets-app` 截图期间 macOS 输出一次 task policy 设置警告，但进程正常退出，五张截图
均生成，未出现 panic、device loss 或渲染失败。

人工证据：

- M1-M8：`m1-m8-manual-acceptance-2026-09-12.md`；
- M9：八场景截图与窗口验收；
- M10.1-M10.5：Shape/Border/Text/Image/Widget/Tooltip/Accessibility 验收；
- M10 Web pointer、keyboard、drag-out、Toggle 与 Tooltip placement 于 2026-09-13
  复核通过；
- macOS live resize 缩小/放大无白色拖影或垂直抖动。

本轮提交只改变共享 M10 Demo 建模与文档，不修改引擎运行时；既有人工契约证据继续
有效，并由当前基线截图和全量测试重新覆盖。

## 5. Web 总审计

最终 Wasm release build 成功。Chrome/Vello WebGPU 当前基线复核：

| 场景 | ready/backend/frame | 结果 |
|---|---|---|
| `viewport/nested-viewports` | `true` / `vello-webgpu` / 1 | PASS |
| `text-image/image-resources` | `true` / `vello-webgpu` / 1 | PASS |
| `widgets/tooltip-boundary-visual` | `true` / `vello-webgpu` / 1 | PASS |

Tooltip DOM 为 `display:block`，bounds 非零，右边界与 Canvas 对齐且整体位于 surface
内。Accessibility projection 包含 heading、label、button 和 Bottom edge 节点。
三个页面 Console 均无消息。Wasm 资源返回 HTTP 200 与 `application/wasm`。

## 6. P0/P1 API 账本

`doc/parity/draw2d/api-coverage.md` 无 `missing` 行。剩余 `partial/deferred` 均有明确
边界：

- clip query/path clip、gradient、image source rectangle、高级 stroke/XOR：
  当前核心 primitive 已验证，高级模式延后到真实产品需求；
- indexed add、removeAll：核心 add/reorder/remove/reparent/dispose 已验证，
  convenience overload 延后；
- arbitrary clipping provider：三种核心 replacement 策略已验证；
- unified Shape mutation：具体 Figure typed mutation 已验证；
- TextFlow fragment/bidi：明确为 P2；
- mirrored coordinates、完整 widget toolkit、打印与高级 router：不属于 Core 1.0。

这些差异均不是已暴露但静默失败的 API。

## 7. 公开 API 真实性

源码扫描未发现产品路径中的 `todo!`、`unimplemented!` 或静默 Unsupported。未支持的
render、image、layout、routing 与 accessibility 能力均返回结构化错误。

Figure trait 上默认 `None` 是可选 capability 查询，不是操作成功；`UpdateListener for
()` 是显式 no-op listener adapter，仅用于无观察行为的身份/生命周期测试，不伪装产品
能力。内部 Manhattan `unreachable!` 对应有限约束下已证明的不变量，不是公开失败路径。

## 8. R9.4 capability 消融

| 候选边界 | 结论 | 独立语义 |
|---|---|---|
| `Bounded` | 保留 | 构造期/独立图元可变几何，与树内 `NodeState` 真源分离 |
| typed listeners | 保留 | payload、回调签名、scope 和 self-removal 语义不同 |
| `NodeState` / `FigureNode` | 保留 | 共享状态视图与拓扑/布局/具体行为职责分离 |
| `ResourceDelta` | 保留 | 有序资源因果、Retry 前缀和 backend session delta |
| Figure optional capabilities | 保留 | 输入、生命周期、Accessibility 等能力可独立缺席 |

删除任一边界都会引入宽 trait 空方法、调用方 enum/type 分派、状态域合并或事务旁路，
不满足消融判据。R9.4 以“证据支持保留”关闭。

## 9. 发布边界

以下能力继续按计划延后，不影响 Core 1.0：

- TextFlow/fragment/bidi 与富文本编辑；
- 完整 widget toolkit 和 repeat scheduler；
- DirectedGraph、ShortestPath、打印与高级 Graphics 模式；
- Windows/Linux 发布资格；
- GEF EditPart、Viewer、Tool、Request、EditPolicy、Command、SelectionProvider 与
  undo/redo command stack。

后续工作必须进入独立 GEF roadmap 或明确的 P2 delta，不再扩张 M1-M10。
