# D4.2 资源因果与 Backend Session 验证

类型：`verification-record`

日期：2026-09-09

## 结论

D4.2 已完成。资源增量保持真实操作顺序，同一 Runtime 可以通过 Ready snapshot 无损
重建单个 backend session，普通 full redraw 不承担 cache 恢复职责。

## 实现证据

- `ResourceDelta` 只保存 ordered `ResourceOp::{Upsert,Remove}`。
- ResourceRegistry Ready/Failed/Removed 转换按提交顺序 append op。
- Registry 可冻结按 ResourceId 稳定排序的最新 Ready snapshot。
- `ResourceSync::{Delta,Snapshot}` 区分增量日志与 session 基线。
- `BackendSessionId` 组合 Runtime namespace 与单调 generation。
- Runtime 初始 session 和 reset 后首帧强制 Snapshot、Full damage 与完整 commands。
- delta Retry 恢复到 newer pending ops 之前；snapshot Retry 从当前 Registry 重建。
- completion 同时校验 session/frame；旧 session completion 不确认当前 in-flight。
- Vello 在跨 Runtime/新 generation 时清理 resource 与 retained cache；每次 Snapshot
  都替换 resource cache。
- Native editor 在 renderer 重建时调用 Runtime session reset。

## 定向验证

- Ready -> Failed -> Ready 产生 Upsert -> Remove -> Upsert。
- ordered Vello resource ops 最终保留最新 Ready revision。
- 空 Snapshot 仍保留 cache replacement 语义。
- partially-applied cache 被新 Snapshot 完整替换。
- snapshot in-flight 后的新 mutation 在 Presented 后作为 Delta 提交。
- snapshot Retry 只包含 Registry 当前 Ready revision。
- session reset 后旧 completion 被拒绝。
- 两个 Runtime 的 BackendSessionId namespace 不同。
- surface resize 产生 Full damage，但资源同步保持空 Delta。

## 自动门禁

- `cargo fmt --all -- --check`：通过
- `cargo check --workspace`：通过
- `cargo clippy --workspace -- -D warnings`：通过
- `cargo test --workspace`：通过
- `cargo check -p web-validation --target wasm32-unknown-unknown`：通过
- `cargo run -p update-app -- --verify`：六项通过

验证期间因磁盘空间不足清理了本仓库可再生成的 `target/` 构建产物；未删除源码、
文档或用户数据。
