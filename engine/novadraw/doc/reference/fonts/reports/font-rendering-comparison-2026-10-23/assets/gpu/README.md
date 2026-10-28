# GPU demo 来源与集成说明

报告直接使用 `drawjs/tests/font/` 中的 GPU 实现副本。

| 入口 | 执行路径 | 输入 |
| --- | --- | --- |
| `evan_wallace/index_stencil_method.html` | WebGPU、stencil、4× MSAA | 原 STSong.ttf |
| `evan_wallace/index_blend_method.html` | WebGPU、颜色累计、奇偶读取 | 原 STSong.ttf |
| `will_dobbie_method/index.html` | WebGL、原 font.frag / web.vert | 原 atlas、vertices 与 pages |
| `lengyel_method/render_char.html` | WebGPU、原 WGSL | 原 STSong.ttf |

集成调整包括本地依赖路径、嵌入页面布局、字形与栅格尺寸控件，以及 GPU 初始化错误提示。Evan 原 Canvas2D 轮廓辅助绘制不再调用；GPU 输出仍由原管线生成。Lengyel 的黑红条带是原 shader 的调试着色，报告保留这一行为。

Dobbie 原 `atlas.bmp` 保存为 xxd 十六进制文本，末尾还包含一个额外换行字节。打包脚本按转储偏移逐行恢复 BMP 字节，并以 BMP 文件长度字段去掉额外换行，未生成或改变字形数据。恢复前后 SHA-256 记录在 `source-manifest.json`。

Dobbie 原源码只绘制首个 glyph。集成层根据原顶点包围盒初始化视图，并提供“聚焦首字形”按钮；它只修改原平移与缩放参数。原 Auto zoom 会在多页中随机选择目标，与单字绘制不匹配，因此隐藏此入口。平移、滚轮缩放、网格显示与全屏沿用原逻辑。

`bundle-gpu-demos.py` 从仓库参考目录重新生成这些副本，同时断言嵌入的 WGSL / shader 片段保持不变。独立的 Dobbie `font.frag` 和 `web.vert` 按字节复制。三次轮廓转换、根分类、颜色计数、滤波单位及原实现缺口均未重新编写。

所需运行依赖全部在 `vendor/`：

- opentype.js 1.3.3，MIT；许可见 `opentype-LICENSE.txt`。
- wgpu-matrix 3.0.1，MIT；原版权和许可正文保留在模块中。
- webgpu-utils 0.15.0，MIT；原版权和许可正文保留在模块中。

字体与图集沿用用户提供的仓库资源，来源摘要由清单记录；此报告没有赋予这些资源新的再分发许可。Lengyel 独立原版 `Lengyel2017FontRendering-GlyphShader.glsl` 标注 CC BY-NC-ND 4.0，未随报告打包。演示使用仓库 HTML 内的 WGSL。

运行方法：

```sh
python3 serve.py --port 8782
```

在报告根目录执行，打开输出的 localhost 地址。通过 HTTP 提供本地字体与 shader；直接双击 HTML 可能因浏览器本地文件限制而无法加载 demo。报告正文仍可阅读。

实际绘制与交互检查见报告根目录 `gpu-validation.json`；检查环境为 macOS、Chrome、Apple M1 Pro。此证据不代表完整字体语义、全部边缘覆盖率或跨平台验收。

本次修改将报告的 CPU 数学样例替换为原 GPU demo，移除旧采样验证结论、同步控件和索引图。四个 GPU 入口加载、绘制与交互检查通过，WebGL `getError()` 为 0；桌面和手机布局均无横向溢出。中文正文已执行机械扫描：半角标点、空泛元话语、翻案句式和双破折号均为 0 命中。代码、命令与源码标识符按原文保留。
