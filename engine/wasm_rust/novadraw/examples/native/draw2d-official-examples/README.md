# Draw2D Official Examples

该应用按 Eclipse `org.eclipse.draw2d.examples` 插件的源码顺序，运行本目录中逐项移植
且通过对等测试的场景。实现不得引用 `novadraw-example-scenes` 中的近似样例；示例状态
以 [`src/lib.rs`](src/lib.rs) 为代码清单。

运行应用：

```sh
cargo run -p draw2d-official-examples
```

列出全部 25 个上游入口、当前状态和能力缺口：

```sh
cargo run -p draw2d-official-examples -- --list
```

保存指定场景截图：

```sh
cargo run -p draw2d-official-examples -- --screenshot=HelloWorld
```

`complete` 表示已通过对等核对；`in_progress` 表示独立移植已有可运行代码，仍有明确差额；
`not_implemented` 表示尚未完成移植，不能据此推断引擎缺能力；`blocked` 表示已定位到
具体 API 或平台缺口。没有实现的条目不绑定替代场景。

当前为 2 个 complete、9 个 in_progress、1 个 not_implemented 和 13 个 blocked。
完整证据见
[`Draw2D 官方示例对等实现审计`](../../../doc/verification/reviews/draw2d-official-examples-parity-2026-10-16.md)。

```sh
cargo run -p draw2d-official-examples -- --screenshot-all
```
