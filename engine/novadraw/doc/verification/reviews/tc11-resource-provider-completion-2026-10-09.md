# TC-11 ResourceProvider 完成记录

类型：`implementation-verification`

日期：2026-10-09

状态：`complete`

## 交付范围

TC-11 按 ADR-030 把资源 I/O、解码与 Runtime 发布拆为三个独立阶段：

```text
platform/application provider
  -> bytes
  -> Core decode
  -> Runtime request-token completion
```

完成内容：

- 删除 `ImageData::decode_file` 与 `Runtime::complete_image_file`；
- 删除 `ImageDecodeError` 中的 I/O 和 Runtime update 占位错误；
- 新增 `ImageCompletionError::{Decode, Resource}`；
- 新增 `ResourceRequestToken` 与 begin/complete/fail 请求 API；
- replacement、duplicate、foreign 和 direct-mutation 后的迟到完成返回
  `ResourceError::StaleRequest` 或 namespace 错误；
- Runtime `register_image/register_font` 直接返回 `Result`，删除 panic/try 双入口；
- Native package 提供 `FileResourceProvider`；
- Web package 在 `resource` feature 下提供 `FetchResourceProvider`；
- Core source gate 禁止资源文件 I/O 回流。

## 所有权与错误

Runtime 不持有 provider。provider 只读取 bytes，应用负责调度：

- Native 文件失败保留 `std::io::Error`；
- Web 区分 fetch rejection、HTTP status、response 类型与 body 读取失败；
- 图片解码失败保留 `ImageDecodeError`；
- resource identity、kind、revision 和 stale request 保留 `ResourceError`。

provider 失败可以通过 `fail_resource_request` 写入稳定诊断，但原始平台错误仍由应用
持有，不压缩为 Core 字符串错误。

## 自动验证

统一 suite：`core.tc11-resource-provider`。

覆盖：

- Core request replacement、direct completion invalidation、decode/resource 错误分层；
- Native provider 读取真实 SVG fixture 并完成 Runtime 发布；
- Native missing file 保留 `io::ErrorKind::NotFound`；
- Web fetch provider wasm32 feature 编译；
- Core render/runtime source I/O 静态门禁。

已通过：

- `cargo xtask verify core.tc11-resource-provider`；
- `cargo xtask verify core.tc12-render-failure`；
- `cargo test -p novadraw --test d4_component_update`；
- `cargo test -p novadraw --test p2_text_graphics_contract`；
- `cargo check --workspace`；
- Core、Native provider 与 Web resource feature 定向 Clippy；
- `cargo xtask docs`；
- `cargo xtask check --quick`；
- full gate 中 fmt、Facade、第三方类型、API snapshot、hot-path、Core I/O、
  Kurbo、Native/Web Vello 与 workspace Clippy 全部通过。

`cargo xtask check --full` 最终仅在 `workspace.test` 阶段被下述既有 Builder
`Result` 迁移问题阻断。

## 已知独立阻断

workspace `cfg(test)` 仍受此前 Builder `Result` 迁移未完成影响，本次 full 报告 405 个
编译错误。`m10_label_contract` 另受此前 root facade 收窄后的旧导入阻断。
这些问题不属于 TC-11；
TC-11 定向测试、source workspace 编译和 Native/Web provider 验证不依赖该旧调用面。

## 关系

- [ADR-030](../../adr/adr-030-resource-provider-and-request-token-boundary.md)
- [资源生命周期](../../design/architecture/resource-lifecycle.md)
- [临时概念设计审计](temporary-concept-design-audit-2026-10-08.md)
- [公共 API 复审](framework-public-api-follow-up-audit-2026-10-07.md)
