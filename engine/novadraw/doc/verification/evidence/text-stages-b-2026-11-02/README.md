# 文本阶段 B 本地产物

类型：`documentation-index`

能力归属：Core 文字引擎及其第三方文本依赖。

日志、NDJSON／JSONL 样本、收据、构建记录和生成快照已迁至
`target/verification/text-stages-b-2026-11-02/`，不纳入 Git。
该路径相对仓库根目录，克隆仓库后不会自带这些文件。
原始字节及冻结收据中的历史路径保持不变。
验证脚本的 `--output` 应使用此路径或其他被 Git 忽略的本地目录。
生产依赖补丁继续保存在 `dependencies/`。

方法、结果与限制见[阶段 B 操作验证](../../reviews/text-stages-b-operations-2026-11-02.md)
及[grapheme 内核验证](../../reviews/text-stages-b1-grapheme-kernel-2026-11-03.md)；
运行命令见[消费者说明](../../../../tests/consumers/harfrust-admission/README.md)。
