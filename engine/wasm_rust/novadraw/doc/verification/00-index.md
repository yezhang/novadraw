# 验证与审计

类型：`verification`

本目录保存人读的验证方法和历史结论，不定义架构或里程碑状态。

| 目录 | 内容 | 入口 |
|---|---|---|
| `reference/` | 外部参考源码基线与纠错依据 | [索引](reference/00-index.md) |
| `reviews/` | 阶段审计和实现验证报告 | [索引](reviews/00-index.md) |
| `manual/` | 窗口、交互和视觉验收步骤 | [索引](manual/00-index.md) |
| `plans/` | 尚未进入实现的验证矩阵与完成门禁 | [索引](plans/00-index.md) |
| `performance/` | 可重复的性能基线 | [GA-2 方法](performance/ga2-methodology.md)、[Novadraw CPU 基线](performance/ga2-novadraw-cpu-2026-10-01.md)、[R8 历史基线](performance/r8-baseline-2026-09-02.md) |
| `checklists/` | 开发与验证检查清单 | [渲染管线](checklists/rendering-pipeline.md) |

## 自动化入口

可执行命令、suite 到 milestone 的映射和证据路径统一定义在
[`../../verification/suites.toml`](../../verification/suites.toml)，schema 和证据
保留规则见 [`../../verification/README.md`](../../verification/README.md)。

```bash
cargo xtask list
cargo xtask docs
cargo xtask check --quick
cargo xtask check --full
cargo xtask verify <suite-id>
cargo xtask manual <suite-id>
```

新增或修改验证入口时先更新 manifest。Review 只引用稳定 suite ID 和执行结果，
不得复制一套会漂移的命令定义。

## 证据边界

- `reviews/` 保存可阅读的结论，不冒充当前设计 SSOT。
- `manual/` 只补充窗口、GPU、浏览器和输入设备体验。
- `target/verification/` 保存可重建的本地运行产物，不提交 Git。
- [`../../verification/evidence/`](../../verification/evidence/README.md) 保存需要随审计
  提交的日志、JSON、探针源码和分组报告。
- 编译后的探针和其他二进制生成物不得进入 `doc/` 或 `verification/evidence/`。

发现不一致时，先回到 `design/`、ADR 或 parity ledger 确定合理契约，再调整实现和
验证；历史报告不覆盖后续设计修订。
