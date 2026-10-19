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

K00 源码清单位于 `source-contracts.tsv`，普通测试核对它与 catalog、全量方案的对应关系：

```sh
cargo test -p draw2d-official-examples --test source_contracts
```

使用本地上游检出校验全部 main、helper、资源和 graph fixture：

```sh
DRAW2D_REFERENCE_ROOT=/Users/bytedance/Documents/code/GitHub/gef-classic cargo test -p draw2d-official-examples --test source_contracts upstream_main_helpers_and_resources_match_the_inventory -- --ignored
```

`source-events.tsv` 记录全部 25 入口及清单中 49 个源码文件的 listener、callback、
SWT import 和宿主调用行；它是词法证据，不是事件运行顺序或全 feature 证明。
更新命令：

```sh
python3 verification/oracles/draw2d/capture_source_events.py --reference-root /Users/bytedance/Documents/code/GitHub/gef-classic --output examples/native/draw2d-official-examples/source-events.tsv
```

[Grid 事件 oracle](../../../verification/oracles/draw2d/README.md) 已记录原生 SWT
注册回调的实际结果。源码清单通过不等于结构/视觉/全交互验收；K00 仍需补齐各入口
运行证据和人工核对后的全 feature manifest。
