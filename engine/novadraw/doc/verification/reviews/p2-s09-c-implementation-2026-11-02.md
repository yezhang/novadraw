# S09-C Connections 注册实施记录

类型：`implementation-verification`

能力归属：Core Connections。Editor、示例和基准仅迁移调用方。
状态：实现、消费者、workspace 与 API 门禁完成；2026-11-02。
目标合同见 [ADR-057 修订 3](../../archive/adr-consolidation-2026-11-03/original/doc/adr/adr-057-service-failure-and-inspector-lifecycle.md.txt)，
执行队列见 [P2 backlog](../../roadmap/p2-delta-backlog.md)。

## 问题与改动

原 `RuntimeConnections::register_anchor/register_router` 直接返回身份，
内部通过 `expect` 把 Runtime fault 转成 panic；同语义的 `try_register_*`
另行返回 `Result`。本片将两组入口各合为一个规范注册方法：

```rust
register_anchor(Box<dyn ConnectionAnchor>) -> Result<AnchorId, ConnectionRuntimeError>
register_router(Box<dyn ConnectionRouter>) -> Result<RouterId, ConnectionRuntimeError>
```

Facade 和 Runtime 私有服务均删除 `try_register_*` 别名。服务先检查 Runtime
健康状态，再向该 Runtime 的 arena 登记对象；已 fault 时返回 `Faulted`，
不登记对象，也不发布身份。更换策略不能恢复原 owner，重新创建 Runtime 才能
建立健康的注册域。`direct_router()` 继续读取已有默认资源。

注册没有 validate 回调，不检查路由约束，不计算端点或路径。测试没有把路由失败
当作注册失败。低层 `ConnectionRuntime` 在服务完成健康检查后仍直接写入
RuntimeArena；通用 SlotMap 的容量、分配和代际限制沿用既有合同，
本片不宣称内存耗尽可恢复。

Editor 的六个注册调用继续用 `?` 传播错误。测试夹具、健康示例和基准显式处理
`Result`。连接注销合同保持：被引用的对象返回 `AnchorInUse`／`RouterInUse`，
已移除或不同 owner 的身份返回 `UnknownAnchor`／`UnknownRouter`；
faulted Runtime 的连接注销返回 `Faulted`。

## 验证依据

| 检查 | 已观察结果 |
|---|---|
| 私有 Core 准入测试 | 1 项通过；fault 下注册被拒绝，实际 registry 与已提交连接前缀不变 |
| 公开契约测试 | 3 项通过；内置和自定义策略共用注册、注册无策略回调、连接准备调用自定义策略、引用保护与移除后身份不复用、真实 observer panic 后拒绝、新 owner 隔离 |
| 独立 workspace 消费者 | Connections 单 feature 3 项通过；Animation／Inspector／Connections 组合 16 项通过；关闭可选调用时测试二进制成功运行，含 0 项用例 |
| 消费者编译与 lint | 单 Connections 及三项组合的 Wasm 编译通过；组合消费者的 Rust 1.96 严格 Clippy 通过 |
| 编译失败测试 | 四项通过，覆盖旧直接 ID 消费与已删除别名 |
| workspace 门禁 | fmt、check、Rust 1.96 严格 Clippy 通过；完整 test 1,782 项通过、0 失败、25 项忽略 |
| API／依赖边界 | Native 与 Wasm rustdoc、第三方类型、依赖和符号快照通过；Runtime 公开方法 28／28 |
| 文档 | `cargo xtask docs` 与 `git diff --check` 通过；启动规则、ADR、队列、验证矩阵与审计状态已回填 |

私有快照辅助函数只在 `cfg(test)` 编译，读取实际 registry，不增加公共测试 hook。
测试先让已有连接完成准备，再注入 owner fault；比较注册前后的 Anchor／Router
身份集合、连接 revision／generation、状态、依赖反向索引、layer owner 表、
路由计数和 Figure 几何，结果均一致。
公开故障通过 observer 的真实 panic 触发，清理 listener 后原 Runtime 仍 faulted。
新 Runtime 接受同种外部策略，并拒绝旧身份。

独立消费者的 Native 测试检查公开行为；Wasm 编译检查健康注册 API。
Wasm 的 panic 故障行为没有运行时验收。`panic=abort`、析构二次 panic 与
任意外部副作用沿用已有边界，不承诺回滚或恢复。

可复跑入口：

```bash
cargo test -p novadraw --lib s09_faulted_connection_registration
cargo test -p novadraw --test p2_s09_connection_registration
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-connections
cargo test --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-animation,consume-inspector,consume-connections
cargo check --manifest-path tests/consumers/service-failure/Cargo.toml --lib --locked --offline --no-default-features --features consume-connections --target wasm32-unknown-unknown
cargo +1.96.0 clippy --workspace -- -D warnings
cargo test --workspace
bash tools/scripts/check_architecture.sh
cargo xtask docs
```

原始日志保存在 [workspace 回归](../evidence/p2-s09-c-2026-11-02/workspace-tests.log)、
[严格 Clippy](../evidence/p2-s09-c-2026-11-02/workspace-clippy.log)
和 [Native／Wasm API 边界](../evidence/p2-s09-c-2026-11-02/architecture.log)。
文档门禁日志见 [docs](../evidence/p2-s09-c-2026-11-02/docs.log)。
workspace 编译与测试使用 Rust 1.99.1；严格 Clippy 与前片保持一致，使用 Rust 1.96.0。

## 成本与覆盖范围

本片复用原 fallible 注册的健康检查和 arena 插入，没有新增生产状态、依赖、
策略回调或树扫描。注册成本仍由 Box 与 arena 插入承担；私有事实快照不进入
生产路径。这是代码边界核对，不是运行耗时或分配次数的实测结论。

本轮未做产物体积分析。按用户 2026-10-30 的执行规则，体积比较只在明确请求时运行。
S09-B 的既有成本证据不扩展为本片的性能结论。

本片对应 SF-09，以及 SF-12／13 的 Connections 前缀、消费者和 API 部分。
回填 `connection.anchor`、`connection.router` 与 API-F09 的注册别名残余。
R07／TC-10 继续部分打开；S09-D 的 Editor owned Error source、panic 诊断和
事务边界尚未实施，S09 整批未关闭。

## 复核

已按技术文档清晰度与好好说话技能自审：区分注册和路由失败，明确 Core 状态归属、
调用方迁移、私有注入与公开生命周期、Native 运行与 Wasm 编译的证据范围。
没有独立 reviewer，本记录不声称独立评审通过。
成稿机械扫描的编号标题、翻案句式、空泛元话语和中文半角标点均为 0 命中；
随后回读了事实、逻辑与措辞，保留所有失败条件和验收限制。
