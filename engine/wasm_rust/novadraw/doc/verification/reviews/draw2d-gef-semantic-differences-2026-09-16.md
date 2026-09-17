# Draw2D / GEF 语义差异报告

类型：`verification`

## 结论与时间边界

Novadraw 已建立 Draw2D/GEF 的主要结构与编辑闭环，但不能认定为核心语义全面等价。
主要问题集中在布局坐标与测量、组合组件、失败恢复、对象生命周期，以及开放的编辑扩展。
Rust 的所有权、强类型和事务边界总体方向合理；部分公开契约的实现和证据尚未闭合。

本报告基于 2026-09-16 开始时的工作区快照，涵盖六组审计和 73 行汇总映射。
源码清单为158个crate Rust文件、75,788行，包含测试；这不是逐行穷尽审计声明。
只对标 `org.eclipse.draw2d`、`org.eclipse.gef` 源码及同版本官方指南。

2026-09-17 恢复汇总时，工作区已迁移证据目录并开始独立整改。因此：

- 原始分组提出17条P1候选；最终跨组复核保留 **16条P1、1条P2待验证风险**。
  Native离窗项因缺实际平台事件序列证据降为P2/6；下表保留原位置及最终评级。
- [整改状态页](draw2d-gef-semantic-remediation-2026-09-16.md) 最新已记录 **5项关闭、
  12项待关闭**：Web构建、release listener scope、event target、pointer exit、
  Policy target已关闭。这是后续整改记录，本次未重新执行其修复验证。
- “待关闭”表示整改页尚无关闭记录，不等于已对正在变化的当前源码重新证明缺陷。
- 原始反例日志不能代表修复后的行为。其行号针对原快照；查询现代码应优先按方法名定位。

详见[固定基线](../reference/draw2d-gef-semantic-baseline-2026-09-16.md)、
[语义映射](draw2d-gef-semantic-mapping-2026-09-16.md)、
[实施方案](draw2d-gef-migration-plan-2026-09-16.md)。

## 原快照问题清单

编号 F01–F17 为本报告交叉引用，不创建新的 roadmap milestone。
“执行”指本次原快照实际复现；“静态”指完整函数与具体输入推导，未运行该反例。
P1 表示需优先修复，不代表所有平台、所有输入都会触发。未发现需要定为 P0 的证据。

| ID | 原快照缺陷与触发结果 | 原位置（Novadraw根相对路径） | 证据 / 置信度 |
|---|---|---|---|
| F01 | 交互父节点下无handler子节点：父漏收Entered，离开子节点时反而新增Entered | runtime/event/mod.rs:377–410（scene） | 执行；10/10；G1；后续已关闭 |
| F02 | scope写入藏在debug_assert中，release下删除owner后订阅仍存活 | runtime/runtime.rs:2485–2488（scene；八类入口） | 静态；10/10；G1补充；后续已关闭 |
| F03 | press→离窗→release被丢弃→重新进入，capture残留导致继续Dragged | novadraw-apps/src/app.rs:488–495 | P2/6，待验证风险；未实测平台序列；后续整改页已关闭 |
| F04 | freeform可见溢出child移动，旧damage被父visual bounds裁空，partial retained可留残影 | runtime/update/repair.rs:116–119（scene） | 静态；10/10；G2-F1 |
| F05 | 100×100容器四边inset10，Stack child从surface(20,20)开始而非(10,10) | graph/mod.rs:4162–4164（scene） | 静态；10/10；G2-F2 |
| F06 | 200高Border中South要求150，Center预留50而South实际只放100，产生空带 | layout/border_layout.rs:279–287（scene） | 静态；10/10；G2-F3 |
| F07 | 删除Ready图片后ImageFigure仍发旧资源引用，Vello整帧持续Retry | graph/mod.rs:3061–3063（scene） | 静态；10/10；G3-F01 |
| F08 | CompoundBorder组合TitleBar时，标题测量与绘制快照不传播，文字和占位消失 | figure/border/compound_border.rs:92–98（scene） | 静态；10/10；G3-F02 |
| F09 | 非空Label与Ready图标，North/South文字相对图标方向相反 | figure/label.rs:581–589（scene） | 静态；10/10；G3-F03 |
| F10 | 首次退化route被Locator拒绝后，观测依赖未保存；移动owner也不再自动恢复 | connection/runtime.rs:650–658（scene） | 静态；10/10；G4-F1 |
| F11 | 同组A→B/B→A两条Fan连接，序号与法向双翻转，折线完全重合 | connection/router.rs:428–436（scene） | 数值代入；10/10；G4-F2 |
| F12 | Connection换到平移父容器后absolute bendpoint未转换或拒绝，端点不动而折点跳跃 | runtime/runtime.rs:1638–1643（scene） | 静态；10/10；G4-F3 |
| F13 | root已activate、child创建失败；contents未提交，Viewer Drop漏deactivate root | novadraw-editor/src/viewer/mod.rs:2691–2695 | 执行；10/10；G5-F01 |
| F14 | E条边顺序不变的属性refresh仍逐边复制/扫描全layer，产生Θ(E²)工作量 | novadraw-editor/src/viewer/mod.rs:2174–2181 | 调用链计数；10/10；G5-F02；无耗时基准 |
| F15 | refresh_visuals改几何后panic，外层捕获时Viewer仍未fault，通知已drain | novadraw-editor/src/viewer/mod.rs:1768–1779 | 执行；10/10；G5-F03 |
| F16 | Policy::target返回另一有效Part，命令与反馈仍走source policy/host | novadraw-editor/src/viewer/mod.rs:1351–1355 | 执行；10/10；G6-F1；后续已关闭 |
| F17 | Web wheel中Scroll/Zoom分支新增DispatchOutcome，调用match未弃值，E0308 | apps/web/web-validation/src/lib.rs:950–953 | web.build退出101；10/10；后续已关闭 |

上表“scene”统一指 `novadraw-scene/src/`。G1–G6原报告链接集中于
[映射表证据索引](draw2d-gef-semantic-mapping-2026-09-16.md#阅读方法与证据)；
包含每项期望、实测/推导、直接调用方、官方来源、修复方向及缺失用例。
最终跨组核对见
[cross-group-review.md](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/cross-group-review.md)。

### 运行反例

[事件探针](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/event-probe.log)
证明 geometric hover 与实际事件目标不同会产生错误enter序列。
[Editor探针](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/editor-probe.log)
记录：

```text
initial_failure: activated=[1] deactivated=[]
policy_redirect: requested_target_model=3 actual_command_hosts=[2]
projection_panic: caught=true viewer_faulted=false
```

Editor探针的panic是故意触发并被捕获，退出码0表示成功证明原缺陷，
不表示正确实现。修复后应使用正向断言，不能把这些反向断言作为长期验收标准。
源码和链接库hash在同目录JSON中。两次探针的scene rlib hash不同，各自证据按各自
元数据解释；不能声称它们来自同一个原子构建快照。初始清理探针记录的是停用回调遗漏，
没有实际注册外部listener；Policy探针证明错误host路由，没有执行错误模型命令。

### 不重复报告已落地的连接重构

当前原快照已有 prepare/validate/atomic commit 主链、Runtime Locator binding、
可替换Anchor descriptor、命名Router及增量关系索引。历史“完全没有这些机制”的
结论不成立。F10是已有事务失败分支漏依赖，F14是增量索引之外仍有全局扫描；
两者都需要沿实际调用链修正。

## 非缺陷类差异与完整性缺口

| 类别 | 具体差异 | 对迁移目标的影响 |
|---|---|---|
| 合理迁移 | namespaced generational ID、值几何、Runtime唯一写入口 | 限制别名与悬空引用，保留可观察行为；持久ModelId仍独立 |
| 合理增强 | buffered LayoutOutput、路由预检、Command补偿和fault | 优于直接共享可变对象；不能扩大为任意外部副作用自动回滚 |
| 合理迁移 | Parley布局、后端无关Glyph IR、资源revision/session | 去除SWT耦合；GPU像素等价必须另验 |
| 已声明收窄 | 同viewport chain限定、remove即dispose、stable journal | 当前受限版本可用，但不是Java完整语义；需保留明确替代契约 |
| 开放扩展缺口 | Tool/Tracker注入、外部Request、SelectionPolicy、RootLayerFactory | 框架扩展仍要求改中心代码，不能给出完整GEF扩展性结论 |
| 上下文不足 | LayoutSnapshot缺visibility/盒模型信息；VisualUpdateContext只便于改primary | trait存在但外部布局和compound visual无法完成等价功能 |
| 组合机制不足 | owner-scoped BorderSnapshot特化TitleBar；custom scalable写入口限内置 | 正常内置组合已出现F08；不能靠继续增加downcast分支解决扩展性 |
| 布局算法未决 | XY/Freeform固定轴hints、Grid equal/grab/压缩、Toolbar minimum、Viewport tracks | 没有明确延期依据，应按方法级语义补齐，不称为Rust合理变体 |
| 输入收窄 | mouse modifier不足、拖动中modifier不更新、Editor keyboard在demo分支 | 约束键、Figure widget与Tool共存尚不完整 |
| 静默支持不一致 | LineStyle、部分arc公开入口与Vello实际lowering不一致 | 后置能力应明确拒绝或不开放，不能接受后静默忽略 |
| 已后置 | 旋转装饰、完整EndpointLocator、路径裁剪、高级stroke、TextFlow、clipboard/direct edit/snap | 不计已交付；逐family保留说明及后续验收 |
| 交付未闭合 | G5检查点C、G6保存加载和三平台编辑等价 | 自动回放通过不足以提升人工或跨平台状态 |

## 数据结构、复杂度与维护性

轻量树、containment与connection关系分离、canonical connection snapshot、typed
constraint符合Rust所有权。去除Java静态临时对象和隐式引用关系是正向迁移。
核心算法不必为了Java类对应关系而全部trait化；真实可替换策略则必须允许外部注入。

已有复杂度保证应沿整条链评估：模型快照可为O(V+E)，但逐child reorder可为
O(Σd²)，连接逐条排序/resolve可为O(E²)；Locator逐连接扫描全部绑定可为O(KL)，
共享router成员查找和lane分配也可能超线性。报告仅给出源码推导，没有测量性能收益。
不建议借此恢复已归档迭代渲染；先消除重复全局工作并测操作计数。

## 验证结果与限制

| 证据 | 原快照结果 | 能证明 / 不能证明 |
|---|---|---|
| workspace.quality | fmt/check/clippy/tests日志通过 | 已有断言通过；不是全部语义组合或wasm32编译通过 |
| verify.update、verify.event、verify.scroll-pane | 全部退出0 | 现有headless应用检查通过 |
| replay.editor-g3、g4、g5.2、g5.3、g5.4、g5.5 | 六项退出0 | 既有编辑事务回放；不替代窗口人工验收 |
| web.build | 退出101，E0308 | 原快照Web构建失败；后续整改页已记录修复通过 |
| event/editor公共API探针 | 构建、执行成功 | 原快照四个缺陷具运行证据 |
| 当前GPU截图/浏览器/Native视觉 | 未执行 | 不宣称视觉等价或检查点C通过 |
| 性能benchmark/极端树深/外部完整工具 | 未执行 | 复杂度和API分析不能冒充这些验证 |

日志汇总见
[additional-checks.json](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/additional-checks.json)、
[workspace-quality.log](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/workspace-quality.log)。
最终文档检查与恢复时源码变化另见
[delivery-checks.json](../../../verification/evidence/draw2d-gef-semantic-audit-2026-09-16/delivery-checks.json)。

本次交付只新增审计文档及证据，不修改运行时或测试，不替并行整改作关闭承诺。
原始分组报告与报告中的“verified”引用是审计时账本事实，不应覆盖当前整改状态。
