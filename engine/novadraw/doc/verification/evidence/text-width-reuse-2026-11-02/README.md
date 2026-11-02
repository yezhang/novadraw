# 宽度复用隔离实验

能力归属：Core 默认文字引擎适配候选；本目录只验证 Parley 0.7.0。
不修改 Core 生产 API，也不代表 TextFlow、Runtime 或 GPU 已通过验收。

`src/main.rs` 使用仓库的四种内置字体，比较新鲜 shaping 与不可变模板克隆后重新断行。
同一请求两条路径交替执行，每负载每路径 8 次预热、96 次正式样本。
正式宽度循环为 640、220、960、640；极窄宽度 1 和 NoWrap 用于正确性核对。
所有负载使用两个字号和 brush 范围；没有真实 inline Figure。

完整导出行范围和指标、所有 positioned glyph、run 的来源范围与字体身份、
字号、方向、brush、变量坐标及 cluster 来源范围与 advance。
核对两种模板来源：尚未断行的模板和先前已对齐的布局；
旧布局始终保留，末尾重新核对它没有变化。
对齐覆盖 Core 当前公开的 Start、Center、End、Left、Right，
不将 Parley 的 Justify 当作本轮 Core 验证范围。
交互核对 caret、hit test、前后视觉移动和全文 selection：
小型复杂文字遍历全部 code point 边界，大负载采用约 32 个边界采样。
所有 Core 交互动作及任意 Unicode 内容的完整验证仍需后续补齐。

分配仪表记录 `System` allocator 的申请次数和请求字节，realloc 记新请求长度；
不表示峰值存活字节、物理内存、数组字段的逐项复制字节或产物体积。
provider 阶段只将布局移动到元组，刻意不重复 clone；
Core 的 Arc、TextInteractionMap 校验与其他发布成本需要另做适配测量。
总计时包含阶段仪表与 receipt 构造，阶段分位数不能相加推导总耗时。
样式边界在计时外建立；prepare 的新鲜路径包含 builder、分析、字体选择和 shaping，
不等同于纯字形塑形时间。

在仓库根目录复现：

```sh
CARGO_TARGET_AARCH64_APPLE_DARWIN_LINKER=/Library/Developer/CommandLineTools/usr/bin/clang \
CC=/Library/Developer/CommandLineTools/usr/bin/clang \
cargo +1.96.0 build --release --offline -j 2 \
  --manifest-path doc/verification/evidence/text-width-reuse-2026-11-02/Cargo.toml \
  --target-dir target/text-width-reuse-196
target/text-width-reuse-196/release/text-width-reuse-experiment > \
  doc/verification/evidence/text-width-reuse-2026-11-02/session-1.jsonl
python3 doc/verification/evidence/text-width-reuse-2026-11-02/summarize.py
```

正式会话须逐一运行，同名日志不要覆盖。原始观测在 `session-*.jsonl`，
汇总为 `summary.json`；首次工具链失败记录在 `build-run.log`。
