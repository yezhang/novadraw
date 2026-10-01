# Draw2D Reference Examples

此目录保存用于人工运行和行为核验的 Eclipse Draw2D 示例，不属于 Novadraw 自身的
示例、公共 API 或 workspace package。

## 来源

- 上游项目：<https://github.com/eclipse-gef/gef-classic>
- 项目参考 checkout：`4463d9d0ce13c19d10fbe769d29f28b7345a8cba`
- 允许参考范围：`org.eclipse.draw2d` 与 `org.eclipse.gef`
- 禁止作为 Novadraw 需求依据：`org.eclipse.zest`

目录中的 Java 示例可能包含为观察特定行为而编写的本地场景。每个源文件保留其自身的
版权和许可证声明；Eclipse 项目的许可证与通知见 [LICENSE](LICENSE) 和
[NOTICE.md](NOTICE.md)。

## 运行

需要 JDK 17 和 Maven。默认运行 `HelloWorld`：

```sh
./run.sh
```

也可以指定简单类名：

```sh
./run.sh TriangleShapeDemo
```

Maven 生成的 `target/` 目录不纳入版本控制。

## GA-2 性能对照

`run_performance.sh` 提供固定场景的 Draw2D 参考 runner。它不依赖本机 Maven，
会下载与当前 Draw2D JAR 要求相容的 SWT，并把依赖、原生库与编译输出保存在本目录
`target/`。

从 Novadraw 仓库根目录运行：

```sh
third_party/draw2d-examples/run_performance.sh
```

默认执行 5 次预热和 30 次采样，报告写入
`target/verification/reports/ga2-draw2d/`。当前自动入口支持 macOS Cocoa aarch64/x86_64
和 Linux GTK aarch64/x86_64；正式同环境证据以 macOS aarch64 为准。
