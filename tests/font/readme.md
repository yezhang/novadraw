字体文件下载：
https://github.com/Haixing-Hu/latex-chinese-fonts

## 坐标系转换

在 glyph 中贝塞尔曲线的顶点坐标是相对于左下角为原点计算的。
为了在canvas中绘制，需要将该坐标系转换到canvas中左上角为原点的坐标系中。
如果将 glyph 中的坐标值成为局部坐标系，将 canvas 中的坐标系称为世界坐标系，那么需要执行转换。

为了将局部坐标系中的任一坐标转换到世界坐标系中，我们需要知道局部坐标系的原点在世界坐标系中的位置 `(x0, y0)`。假设局部坐标系中的点 `(x_local, y_local)`，转换到世界坐标系中的点 `(x_world, y_world)` 的公式如下：

```javascript
x_world = x0 + x_local;
y_world = y0 - y_local; 相当于 y_world = y0 + (-y_local)
```

注意，`y_local` 需要取反，因为在 canvas 中，y 轴是向下增加的，而在局部坐标系中，y 轴是向上增加的。

## 本地调试

使用 python 模块 http.server 来启动一个本地服务器，然后在浏览器中打开 http://localhost:8080 即可查看效果。

```bash
python -m http.server 8080
```
