# Shapes 编辑器首批消费者

类型：`implementation-guide`

```sh
cargo +1.96.0 run -p gef-editor-demo
cargo +1.96.0 test -p gef-editor-demo --lib -- --test-threads=1
```

从仓库根目录执行。demo 使用根 workspace 的锁文件，默认二进制为
`gef-editor-demo`；体积基线需显式指定 `--bin ui-shell-baseline`。
UI 依赖固定的未修改上游 Git 提交，Cargo 管理源码缓存，无需本地 vendor。
需要安装 Rust 1.96。验证沿用上游输入 harness，绘制经独立的真实 GPU renderer；
不会调用上游 CPU 截图来替代 GPU 证据。
本次迁移与成本证据见
[验证记录](../../../doc/verification/reviews/masonry-upstream-migration-2026-10-28.md)。

macOS 的标准 bundle 启动路径：

```sh
python3 tools/scripts/run_gef_macos.py
```

脚本构建 release、在 `target/masonry` 生成 `.app`，通过 LaunchServices 打开
该 bundle 并等待关闭。用户置前台后已确认画面显示，真实窗口截图已保存；
已登记 `gef-shapes-macos` Run action。自动前台激活和原生完整交互仍分别待验收。

矩形/椭圆、选择与拖动、创建/删除、撤销/重做通过同一 ModelAdapter →
EditPart → Policy → CommandStack 链路。Xilem 工具栏把动作放入应用队列，
rebuild 消费队列，Masonry Widget 保留 Viewer 和 history。main 仅组合应用入口。

这是 GEF Shapes 对等实现的首批消费证据，完整 Shapes 尚未验收。
CreationTool、marquee、模板拖放和连接编辑仍按
[实施队列](../../../doc/roadmap/p2-delta-backlog.md)推进。
`ui-shell-baseline` 是同配置 Native UI 体积测量基线，不是用户样例。
