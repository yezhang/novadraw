# S09-D Editor 扩展错误与 panic 诊断实施记录

类型：`implementation-verification`

能力归属：Editor。本片使用既有 Core Runtime、Figure 和通知协议，不新增 Core 能力。
状态：实现与定向验证已完成，最终门禁待补验；2026-11-02。
目标见 [ADR-057](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-057-service-failure-and-inspector-lifecycle.md.txt)。

## 入口盘点与契约

本片按已接受的 S09-D 合同审阅当前实现。GEF Command／CommandStack 与 EditPolicy
对 Draw2D 的投影依赖沿用既有 G1／G4 契约；保存 Rust 错误原因和 panic 诊断归 Editor，
不需要新的 Draw2D 基础能力。

| 入口 | 原问题 | 本片目标 |
|---|---|---|
| CommandError、PolicyError | 只存文本，source 为空，声明任意错误相等 | 保存共享 owned source，保留文本构造，移除 Eq／PartialEq |
| CommandStack execute／undo／redo／check | panic payload 被丢弃；普通错误无执行位置 | 分类、操作与位置独立保存；保持原 history／fault 规则 |
| CompoundCommand 回滚／恢复失败 | 原拒绝和补偿失败合为文本 | source 保留原拒绝，独立字段保留补偿错误 |
| Policy registry activate／target／contribute | 外层转发正确，但无回调位置 | 同一开放 registry 记录回调位置并保留 source |
| DirectText／Connection 计划 | 已经转发 PolicyError | 为实际 fallible 回调补操作位置 |
| EditPart、Model、Tool → Viewer／Domain | 相邻扩展缺少 owned source；部分已有 source 转发 | 保存 Error 原因，沿用 Viewer／Tool／Domain 的领域分层 |
| Viewer／Tool 捕获、creation／cleanup | 文本或布尔诊断丢 payload 分类 | 文本原样保存，非文本仅记录分类与位置，不格式化任意 Any |

owned source 使用局部 `Rc<dyn Error>`，不要求外部错误 Clone／Send／Sync。
CommandError 的分类和失败操作分别读取；PolicyError 的领域类型表示 policy 拒绝，
失败操作独立于显示文本。CommandStackError 不再因包含任意原因而承诺 Eq／PartialEq。
EditPartError 同样取消相等承诺。

`VisualUpdateContext::update_component` 的关联错误只有 Display 约束，可能不是 Error，
该入口继续显示文本，不伪造可 downcast 的原始 Error。ModelAdapter 已要求 Error + 'static，
其 children／connections 原因可以完整保存。Drop 中的 best-effort deactivation 不增加
可在对象销毁后查询的诊断承诺；panic=abort、析构二次 panic 和任意外部副作用仍不可恢复。

## 验证

公开测试入口为 `crates/novadraw-editor/tests/p2_s09_extension_failure.rs`。
独立 workspace 消费者通过 `consume-editor` 复用同一公开夹具，不依赖私有测试 hook。

| 验收项 | 本片证据 |
|---|---|
| SF-10 | 非 Clone／非 Send 的原始 Error 经 CommandStack／Domain 或 Policy registry／Viewer／Tool／Domain 多层转发后可 downcast；Clone 共享原因 |
| SF-11 | Command 的 14 个回调子场景，以及真实 Policy／Tool 回调，覆盖文本和非文本 payload；非文本格式化器不得被调用 |
| SF-12 | 普通拒绝保持模型、历史与此前 Tool；panic 保留已提交前缀并遵循既有 owner fault；补偿失败独立保存两个原因且不发布历史 |
| SF-13 | Native 新增 8 项公开契约测试、独立消费者四项能力组合的 24 项测试通过；全部关闭时 0 项；Editor-only Wasm 较早阶段通过，最新工作区重验因磁盘空间不足失败 |
| SF-19 | 分类、操作、原始原因分别断言；Editor 16 项 doctest 通过，其中 3 项 compile-fail 检查受影响错误类型不再承诺相等 |

定向回归此前通过 Editor lib 20 项、G1 15 项、G4 20 项、G5 20 项、
E02 10 项，共 85 项；新增夹具本轮复验 8 项通过。两批分开统计，
不把此前 7 项夹具结果与本轮 8 项重复累加。
Rust 1.96.0 的 Editor lib 与新增 integration target 严格 Clippy 本轮通过。
独立消费者四项能力组合的 lib 严格 Clippy 同样通过。
workspace 和两份消费者格式检查、`git diff --check` 本轮通过。

最终门禁未完成：

- `cargo test --workspace` 和全目标消费者在链接阶段报
  `No space left on device`，没有形成全量通过结果；
- 架构脚本在刷新公开 API 文档缓存时同样因空间不足失败，后接的 docs 未执行；
- workspace 严格 Clippy 与 extensions suite 已停止，不能据残缺日志判定通过；
- 最新 Wasm 检查和 release 成本探针重试同样因磁盘空间不足失败；
  成本探针已提供，尚无有效测量，P50／P95 和分配数保持待补验。

失败日志见 [证据目录](../evidence/p2-s09-d-2026-11-02/) 中的 `*-final.log`。
旧日志只表示较早阶段的覆盖，不代表最新工作区完成最终门禁。
恢复足够磁盘空间后需运行 Wasm 消费者、成本探针及 workspace／架构／docs／extensions
门禁，核对工作区同时进行的其他改动，再关闭本片。S09、R07 与 TC-10 保持未关闭。

本片不做产物体积比较。S09-B 的容量 64／4096 成本限制保持，
不能据本片测试将整批最大容量成本要求勾选为完成。
