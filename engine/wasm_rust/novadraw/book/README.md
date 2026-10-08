# Novadraw Books

本目录包含三本面向不同读者的书：

| 书籍 | 读者 | 目标 | 入口 |
|---|---|---|---|
| 《Novadraw 应用开发与扩展指南》 | 应用开发者、外部扩展作者 | 使用公开 API 构建应用并扩展能力 | [`src/README.md`](src/README.md) |
| 《深入理解 Novadraw》 | 高级用户、架构设计者、贡献者 | 理解稳定模型、不变量与算法 | [`internals/src/README.md`](internals/src/README.md) |
| 《Novadraw 贡献者指南》 | Novadraw 贡献者 | 在当前仓库中安全实现和验证修改 | [`contributor/src/README.md`](contributor/src/README.md) |

分界规则：

- 不修改 Novadraw 仓库源码即可完成的工作，属于应用开发与扩展指南；
- 不依赖当前源码结构的状态模型、不变量和算法推导，属于深入理解书；
- 需要修改 Core、Editor、Inspector、后端或平台内部协议的工作，属于贡献者指南；
- `doc/` 中的 ADR、规范性设计、语义账本和验证清单仍是唯一事实源，三本书只负责
  面向各自读者组织教学内容。

构建入口：

```bash
./scripts/build_book.sh
```

脚本会分别生成用户书、深入理解书和贡献者书。
