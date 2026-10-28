# Third-party reference sources

本目录保存用于行为核验的上游源码快照。它们不是 Novadraw 自有代码，不参与 Rust
workspace 构建，也不构成 Novadraw 公共 API。

## Eclipse Draw2D

| 项目 | 内容 |
|---|---|
| 目录 | `org.eclipse.draw2d.source_3.10.100.201606061308/` |
| Bundle | `org.eclipse.draw2d.source` |
| 版本 | `3.10.100.201606061308` |
| 上游 | [Eclipse Draw2D source archive](https://download.eclipse.org/oomph/archive/simrel/gef.aggrcon/index/org.eclipse.draw2d.source_3.10.100.201606061308.html) |
| 许可证 | Eclipse Public License 1.0 |
| 原始声明 | [`about.html`](org.eclipse.draw2d.source_3.10.100.201606061308/about.html) |

该快照只用于核对 Draw2D 的行为语义、默认值和边界条件。Novadraw 对上游行为的采用、
调整和验证映射，以
[Draw2D parity ledger](../../engine/novadraw/doc/parity/draw2d/api-coverage.md)
为准。

新增第三方材料时，必须同时记录来源、精确版本、许可证和本地修改情况，并保留上游
版权声明。
