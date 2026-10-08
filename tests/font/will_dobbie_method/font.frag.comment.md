## font.frag 代码注释

### 逐行解释

以下是结合 `font.frag` 代码的逐行注释，帮助理解每一行的功能和作用：

```glsl
#extension GL_OES_standard_derivatives : enable
precision highp float;

// 定义常量
#define numSS 4  // 抗锯齿的采样次数
#define pi 3.1415926535897932384626433832795  // 圆周率
#define kPixelWindowSize 1.0  // 像素窗口大小

// 纹理和变量声明
uniform sampler2D uAtlasSampler;  // 存储贝塞尔曲线数据的纹理
uniform vec2 uTexelSize;  // 纹理的像素大小
uniform int uShowGrids;  // 是否显示网格的标志

// 从顶点着色器传递的变量
varying vec2 vCurvesMin;  // 当前网格单元的起始坐标
varying vec2 vGridMin;  // 网格的最小坐标
varying vec2 vGridSize;  // 网格的大小
varying vec2 vNormCoord;  // 归一化的纹理坐标
varying vec4 vColor;  // 字形的颜色

// 计算贝塞尔曲线在参数 t 处的位置
float positionAt(float p0, float p1, float p2, float t) {
    float mt = 1.0 - t;
    return mt*mt*p0 + 2.0*t*mt*p1 + t*t*p2;
}

// 计算贝塞尔曲线在参数 t 处的切线
float tangentAt(float p0, float p1, float p2, float t) {
    return 2.0 * (1.0-t) * (p1 - p0) + 2.0 * t * (p2 - p1);
}

// 判断两个浮点数是否几乎相等
bool almostEqual(float a, float b) {
    return abs(a-b) < 1e-5;
}

// 从 vec2 中提取归一化的 unsigned short 值
float normalizedUshortFromVec2(vec2 v) {
    return (256.0/257.0) * v.x + (1.0/257.0) * v.y;
}

// 从纹理中读取 vec2 数据
vec2 fetchVec2(vec2 coord) {
    vec2 ret;
    vec4 tex = texture2D(uAtlasSampler, (coord + 0.5) * uTexelSize);
    ret.x = normalizedUshortFromVec2(tex.rg);
    ret.y = normalizedUshortFromVec2(tex.ba);
    return ret;
}

// 从纹理中读取贝塞尔曲线的控制点
void fetchBezier(int coordIndex, out vec2 p[3]) {
    for (int i=0; i<3; i++) {
        p[i] = fetchVec2(vec2(vCurvesMin.x + float(coordIndex + i), vCurvesMin.y)) - vNormCoord;
    }
}

// 计算贝塞尔曲线与水平线的交点
int getAxisIntersections(float p0, float p1, float p2, out vec2 t) {
    if (almostEqual(p0, 2.0*p1 - p2)) {
        t[0] = 0.5 * (p2 - 2.0*p1) / (p2 - p1);
        return 1;
    }

    float sqrtTerm = p1*p1 - p0*p2;
    if (sqrtTerm < 0.0) return 0;
    sqrtTerm = sqrt(sqrtTerm);
    float denom = p0 - 2.0*p1 + p2;
    t[0] = (p0 - p1 + sqrtTerm) / denom;
    t[1] = (p0 - p1 - sqrtTerm) / denom;
    return 2;
}

// 计算像素窗口内的积分值，用于抗锯齿
float integrateWindow(float x) {
    float xsq = x*x;
    return sign(x) * (0.5 * xsq*xsq - xsq) + 0.5;  // 抛物线窗口函数
}

// 生成单位线矩阵
mat2 getUnitLineMatrix(vec2 b1, vec2 b2) {
    vec2 V = b2 - b1;
    float normV = length(V);
    V = V / (normV*normV);

    return mat2(V.x, -V.y, V.y, V.x);
}

// 更新最近的交点
void updateClosestCrossing(in vec2 p[3], mat2 M, inout float closest) {
    for (int i=0; i<3; i++) {
        p[i] = M * p[i];
    }

    vec2 t;
    int numT = getAxisIntersections(p[0].y, p[1].y, p[2].y, t);

    for (int i=0; i<2; i++) {
        if (i == numT) break;
        if (t[i] > 0.0 && t[i] < 1.0) {
            float posx = positionAt(p[0].x, p[1].x, p[2].x, t[i]);
            if (posx > 0.0 && posx < abs(closest)) {
                float derivy = tangentAt(p[0].y, p[1].y, p[2].y, t[i]);
                closest = (derivy < 0.0) ? -posx : posx;
            }
        }
    }
}

// 计算 2x2 矩阵的逆矩阵
mat2 inverse(mat2 m) {
  return mat2(m[1][1],-m[0][1],
             -m[1][0], m[0][0]) / (m[0][0]*m[1][1] - m[0][1]*m[1][0]);
}

// 主函数
void main() {
    // 计算当前像素对应的网格单元
    vec2 integerCell = floor( clamp(vNormCoord * vGridSize, vec2(0.5), vec2(vGridSize)-0.5));
    vec2 indicesCoord = vGridMin + integerCell + 0.5;
    vec2 cellMid = (integerCell + 0.5) / vGridSize;

    // 生成初始旋转矩阵
    mat2 initrot = inverse(mat2(dFdx(vNormCoord) * kPixelWindowSize, dFdy(vNormCoord) * kPixelWindowSize));

    // 生成旋转矩阵，用于抗锯齿采样
    float theta = pi/float(numSS);
    mat2 rotM = mat2(cos(theta), sin(theta), -sin(theta), cos(theta));

    // 从纹理中读取贝塞尔曲线的索引
    ivec4 indices1, indices2;
    indices1 = ivec4(texture2D(uAtlasSampler, indicesCoord * uTexelSize) * 255.0 + 0.5);
    indices2 = ivec4(texture2D(uAtlasSampler, vec2(indicesCoord.x + vGridSize.x, indicesCoord.y) * uTexelSize) * 255.0 + 0.5);

    // 判断是否有超过 4 个索引
    bool moreThanFourIndices = indices1[0] < indices1[1];

    // 初始化最近的交点
    float midClosest = (indices1[2] < indices1[3]) ? -2.0 : 2.0;

    // 初始化抗锯齿采样数组
    float firstIntersection[numSS];
    for (int ss=0; ss<numSS; ss++) {
        firstIntersection[ss] = 2.0;
    }

    float percent = 0.0;

    // 生成中间变换矩阵
    mat2 midTransform = getUnitLineMatrix(vNormCoord, cellMid);

    // 遍历所有贝塞尔曲线
    for (int bezierIndex=0; bezierIndex<8; bezierIndex++) {
        int coordIndex;

        if (bezierIndex < 4) {
            coordIndex = indices1[bezierIndex];
        } else {
            if (!moreThanFourIndices) break;
            coordIndex = indices2[bezierIndex-4];
        }

        if (coordIndex < 2) {
            continue;
        }

        // 获取贝塞尔曲线的控制点
        vec2 p[3];
        fetchBezier(coordIndex, p);

        // 更新最近的交点
        updateClosestCrossing(p, midTransform, midClosest);

        // 将控制点转换到局部坐标系
        for (int i=0; i<3; i++) {
            p[i] = initrot * p[i];
        }

        // 遍历所有采样角度
        for (int ss=0; ss<numSS; ss++) {
            vec2 t;
            int numT = getAxisIntersections(p[0].x, p[1].x, p[2].x, t);

            for (int tindex=0; tindex<2; tindex++) {
                if (tindex == numT) break;

                if (t[tindex] > 0.0 && t[tindex] <= 1.0) {
                    float derivx = tangentAt(p[0].x, p[1].x, p[2].x, t[tindex]);
                    float posy = positionAt(p[0].y, p[1].y, p[2].y, t[tindex]);

                    if (posy > -1.0 && posy < 1.0) {
                        // 计算积分值
                        float delta = integrateWindow(posy);
                        percent = percent + (derivx < 0.0 ? delta : -delta);

                        // 更新最近的交点
                        float intersectDist = posy + 1.0;
                        if (intersectDist < abs(firstIntersection[ss])) {
                            firstIntersection[ss] = derivx < 0.0 ? -intersectDist : intersectDist;
                        }
                    }
                }
            }

            // 旋转控制点，准备下一次采样
            if (ss+1<numSS) {
                for (int i=0; i<3; i++) {
                    p[i] = rotM * p[i];
                }
            }
        }
    }

    // 判断当前像素是否在字形内部
    bool midVal = midClosest < 0.0;

    // 添加内部射线的贡献
    for (int ss=0; ss<numSS; ss++) {
        if ((firstIntersection[ss] >= 2.0 && midVal) || (firstIntersection[ss] > 0.0 && abs(firstIntersection[ss]) < 2.0)) {
            percent = percent + 1.0;
        }
    }

    // 计算最终的透明度值
    percent = percent / float(numSS);

    // 输出颜色
    gl_FragColor = vColor;
    gl_FragColor.a *= percent;

    // 如果显示网格，则修改颜色
    if (uShowGrids != 0) {
        vec2 gridCell = mod(floor(integerCell), 2.0);
        gl_FragColor.r = (gridCell.x - gridCell.y) * (gridCell.x - gridCell.y);
        gl_FragColor.a += 0.3;
    }
}
```

### 关键点总结

- **贝塞尔曲线的存储与读取**：通过纹理存储贝塞尔曲线的控制点数据，并在片段着色器中读取。
- **交点计算**：通过求解二次方程的根，计算贝塞尔曲线与水平线的交点。
- **抗锯齿**：通过多次采样和积分计算，实现平滑的抗锯齿效果。
- **网格显示**：可选地显示网格，便于调试和可视化。

通过这些注释，可以更清晰地理解 `font.frag` 片段着色器的实现细节和功能。

## integerCell/indicesCoord/cellMid 计算逻辑

在 `font.frag` 片段着色器中，`integerCell`、`indicesCoord` 和 `cellMid` 是与网格单元（grid cell）相关的变量，用于定位当前像素对应的贝塞尔曲线数据。以下是它们的详细解释和计算逻辑：

---

### 1. **`integerCell`：当前像素所在的网格单元坐标**

```glsl
vec2 integerCell = floor(clamp(vNormCoord * vGridSize, vec2(0.5), vec2(vGridSize) - 0.5));
```

- **`vNormCoord`**：归一化的纹理坐标，表示当前像素在字形中的相对位置。
- **`vGridSize`**：网格的大小，表示字形被划分为多少个网格单元。
- **`vNormCoord * vGridSize`**：将归一化坐标转换为网格坐标。例如，如果网格大小为 `10x10`，则 `vNormCoord * vGridSize` 的范围是 `[0, 10]`。
- **`floor`**：将浮点数坐标向下取整，得到当前像素所在的网格单元的整数坐标。
- **`clamp`**：将坐标限制在 `[0.5, vGridSize - 0.5]` 范围内，避免越界。

**作用**：

- `integerCell` 表示当前像素所在的网格单元的整数坐标。例如，如果网格大小为 `10x10`，则 `integerCell` 的范围是 `[0, 9]`。

---

### 2. **`indicesCoord`：贝塞尔曲线索引的纹理坐标**

```glsl
vec2 indicesCoord = vGridMin + integerCell + 0.5;
```

- **`vGridMin`**：网格的起始坐标，表示当前字形在纹理中的起始位置。
- **`integerCell`**：当前像素所在的网格单元坐标。
- **`+ 0.5`**：将整数坐标转换为纹理坐标的中心点。纹理坐标的范围是 `[0, 1]`，而网格单元的整数坐标是离散的，因此需要加上 `0.5` 来获取网格单元的中心。

**作用**：

- `indicesCoord` 是当前网格单元在纹理中的坐标，用于从纹理中读取贝塞尔曲线的索引数据。

---

### 3. **`cellMid`：网格单元的中心点坐标**

```glsl
vec2 cellMid = (integerCell + 0.5) / vGridSize;
```

- **`integerCell + 0.5`**：将网格单元的整数坐标转换为中心点坐标。
- **`/ vGridSize`**：将中心点坐标归一化到 `[0, 1]` 范围。

**作用**：

- `cellMid` 表示当前网格单元的中心点在归一化坐标系中的位置。它用于计算贝塞尔曲线与网格单元的相对位置。

---

### 为什么要这么计算？

1. **网格单元的作用**：

   - 字形被划分为多个网格单元，每个网格单元存储与该单元相交的贝塞尔曲线的索引和控制点数据。
   - 通过网格单元，可以快速定位当前像素需要处理的贝塞尔曲线，避免遍历所有曲线。

2. **纹理坐标的计算**：

   - 纹理坐标需要精确到网格单元的中心点，以确保读取的贝塞尔曲线数据是准确的。
   - `indicesCoord` 的计算确保了从纹理中读取的数据对应于当前网格单元。

3. **归一化坐标的作用**：

   - `cellMid` 的归一化坐标用于计算贝塞尔曲线与当前像素的相对位置，从而确定像素是否被字形覆盖。

4. **抗锯齿和交点计算**：
   - 通过 `integerCell` 和 `cellMid`，可以确定当前像素与贝塞尔曲线的交点，进而计算像素的透明度值，实现抗锯齿效果。

---

### 示例

假设：

- 网格大小 `vGridSize = (10, 10)`，即字形被划分为 `10x10` 的网格。
- 当前像素的归一化坐标 `vNormCoord = (0.15, 0.25)`。

计算过程：

1. **`integerCell`**：

   - `vNormCoord * vGridSize = (1.5, 2.5)`
   - `floor(clamp((1.5, 2.5), (0.5, 0.5), (9.5, 9.5))) = (1, 2)`
   - 结果：`integerCell = (1, 2)`

2. **`indicesCoord`**：

   - 假设 `vGridMin = (0, 0)`（字形从纹理的左上角开始）。
   - `indicesCoord = vGridMin + integerCell + 0.5 = (0, 0) + (1, 2) + (0.5, 0.5) = (1.5, 2.5)`
   - 结果：`indicesCoord = (1.5, 2.5)`

3. **`cellMid`**：
   - `cellMid = (integerCell + 0.5) / vGridSize = (1.5, 2.5) / (10, 10) = (0.15, 0.25)`
   - 结果：`cellMid = (0.15, 0.25)`

---

### 总结

- **`integerCell`**：当前像素所在的网格单元坐标。
- **`indicesCoord`**：当前网格单元在纹理中的坐标，用于读取贝塞尔曲线数据。
- **`cellMid`**：当前网格单元的中心点坐标，用于计算贝塞尔曲线与像素的相对位置。

这些变量的计算是为了高效地定位和处理贝塞尔曲线数据，从而实现高质量的字体渲染。

## 采样过程分析

在 `font.frag` 中，遍历所有采样角度的逻辑是为了实现抗锯齿效果。通过在不同角度下采样贝塞尔曲线，计算像素的覆盖面积，从而得到平滑的边缘效果。以下是针对一个采样角度的详细分析，逐行解释其目的和计算逻辑。

---

### 采样角度的核心逻辑

```glsl
for (int ss = 0; ss < numSS; ss++) {
    vec2 t;
    int numT = getAxisIntersections(p[0].x, p[1].x, p[2].x, t);

    for (int tindex = 0; tindex < 2; tindex++) {
        if (tindex == numT) break;

        if (t[tindex] > 0.0 && t[tindex] <= 1.0) {
            float derivx = tangentAt(p[0].x, p[1].x, p[2].x, t[tindex]);
            float posy = positionAt(p[0].y, p[1].y, p[2].y, t[tindex]);

            if (posy > -1.0 && posy < 1.0) {
                float delta = integrateWindow(posy);
                percent = percent + (derivx < 0.0 ? delta : -delta);

                float intersectDist = posy + 1.0;
                if (intersectDist < abs(firstIntersection[ss])) {
                    firstIntersection[ss] = derivx < 0.0 ? -intersectDist : intersectDist;
                }
            }
        }
    }

    if (ss + 1 < numSS) {
        for (int i = 0; i < 3; i++) {
            p[i] = rotM * p[i];
        }
    }
}
```

---

### 逐行解释

#### 1. **初始化采样角度**

```glsl
for (int ss = 0; ss < numSS; ss++) {
```

- **目的**：遍历所有采样角度。`numSS` 是采样次数，通常为 4。
- **作用**：通过多次采样，计算像素在不同角度下的覆盖面积，实现抗锯齿。

---

#### 2. **计算贝塞尔曲线与水平线的交点**

```glsl
vec2 t;
int numT = getAxisIntersections(p[0].x, p[1].x, p[2].x, t);
```

- **目的**：计算当前贝塞尔曲线与水平线的交点。
- **`getAxisIntersections`**：求解二次方程的根，得到贝塞尔曲线与水平线的交点参数 `t`。
- **`t`**：存储交点的参数值（范围 `[0, 1]`）。
- **`numT`**：交点的数量（0、1 或 2）。

---

#### 3. **遍历所有交点**

```glsl
for (int tindex = 0; tindex < 2; tindex++) {
    if (tindex == numT) break;
```

- **目的**：遍历所有交点，计算每个交点对像素覆盖面积的贡献。
- **`tindex`**：当前交点的索引。
- **`break`**：如果交点数少于 2，则提前退出循环。

---

#### 4. **检查交点是否在有效范围内**

```glsl
if (t[tindex] > 0.0 && t[tindex] <= 1.0) {
```

- **目的**：确保交点参数 `t` 在有效范围内（`[0, 1]`）。
- **作用**：排除无效的交点，避免错误计算。

---

#### 5. **计算交点的切线和位置**

```glsl
float derivx = tangentAt(p[0].x, p[1].x, p[2].x, t[tindex]);
float posy = positionAt(p[0].y, p[1].y, p[2].y, t[tindex]);
```

- **`tangentAt`**：计算贝塞尔曲线在交点处的切线（导数）。
- **`positionAt`**：计算贝塞尔曲线在交点处的垂直位置（y 坐标）。
- **`derivx`**：交点的切线值，用于判断曲线的方向（进入或退出字形）。
- **`posy`**：交点的垂直位置，用于计算像素的覆盖面积。

---

#### 6. **检查交点是否在像素窗口内**

```glsl
if (posy > -1.0 && posy < 1.0) {
```

- **目的**：确保交点位于像素窗口内（`[-1, 1]` 范围）。
- **作用**：排除位于像素窗口外的交点，避免错误计算。

---

#### 7. **计算交点的贡献**

```glsl
float delta = integrateWindow(posy);
percent = percent + (derivx < 0.0 ? delta : -delta);
```

- **`integrateWindow`**：计算交点对像素覆盖面积的贡献。
- **`delta`**：交点的贡献值，根据切线方向（`derivx`）决定是增加还是减少覆盖面积。
- **`percent`**：累加所有交点的贡献，最终得到像素的覆盖面积。

---

#### 8. **更新最近的交点**

```glsl
float intersectDist = posy + 1.0;
if (intersectDist < abs(firstIntersection[ss])) {
    firstIntersection[ss] = derivx < 0.0 ? -intersectDist : intersectDist;
}
```

- **`intersectDist`**：交点与像素窗口边界的距离。
- **`firstIntersection[ss]`**：存储当前采样角度下的最近交点。
- **作用**：记录最近的交点，用于后续判断像素是否在字形内部。

---

#### 9. **旋转控制点，准备下一次采样**

```glsl
if (ss + 1 < numSS) {
    for (int i = 0; i < 3; i++) {
        p[i] = rotM * p[i];
    }
}
```

- **目的**：旋转贝塞尔曲线的控制点，准备下一个采样角度。
- **`rotM`**：旋转矩阵，用于将控制点旋转到下一个采样角度。
- **作用**：通过旋转控制点，模拟不同角度下的采样过程。

---

### 总结

- **采样角度**：通过多次采样，计算像素在不同角度下的覆盖面积，实现抗锯齿。
- **交点计算**：求解贝塞尔曲线与水平线的交点，确定像素的覆盖面积。
- **贡献累加**：根据交点的位置和切线方向，累加像素的覆盖面积。
- **旋转控制点**：通过旋转控制点，模拟不同角度下的采样过程。

通过这一过程，`font.frag` 能够高效地计算像素的覆盖面积，并实现高质量的字体渲染效果。

## 窗口函数的积分

这段代码定义了一个函数 `integrateWindow`，它接受一个浮点数 `x` 作为输入，并返回一个浮点数。这个函数的主要目的是计算一个窗口函数的积分值。窗口函数通常用于信号处理中，用于平滑信号或减少信号的边缘效应。

### 代码解析

1. **计算 `xsq`**:

   ```cpp
   float xsq = x*x;
   ```

   这行代码计算了输入 `x` 的平方，并将结果存储在变量 `xsq` 中。

2. **返回窗口函数的积分值**:

   ```cpp
   return sign(x) * (0.5 * xsq*xsq - xsq) + 0.5;           // parabolic window
   ```

   这行代码计算并返回一个窗口函数的积分值。具体来说，它计算了一个抛物线窗口（parabolic window）的积分值。

   - `sign(x)` 是一个符号函数，返回 `x` 的符号（即 `1` 表示正数，`-1` 表示负数）。
   - `0.5 * xsq*xsq - xsq` 是一个关于 `x` 的四次多项式，表示抛物线窗口的形状。
   - 最后加上 `0.5` 是为了将窗口函数的积分值调整到合适的范围。

3. **注释掉的代码**:

   ```cpp
   //return 0.5 * (1.0 - sign(x) * xsq);                     // box window
   ```

   这行代码被注释掉了，但它展示了另一种窗口函数（box window）的积分值计算方式。如果取消注释，函数将返回一个矩形窗口（box window）的积分值。

   - `0.5 * (1.0 - sign(x) * xsq)` 是一个关于 `x` 的二次多项式，表示矩形窗口的形状。

### 总结

- 这个函数 `integrateWindow` 主要用于计算窗口函数的积分值。
- 默认情况下，它计算的是抛物线窗口的积分值。
- 如果取消注释，它也可以计算矩形窗口的积分值。

窗口函数的选择取决于具体的应用场景。抛物线窗口通常用于平滑信号，而矩形窗口则用于简单的信号截取。

## 贝塞尔曲线坐标的存储方式

### 问题1：贝塞尔曲线控制点坐标的转换与统一

在字体渲染中，贝塞尔曲线的控制点坐标值确实会影响采样计算，但通过**坐标归一化**和**预处理步骤**，em坐标系与像素的`[0,1]`范围可以统一。具体流程如下：

1. **字体预处理**：

   - 字体文件中的贝塞尔曲线控制点坐标通常定义在 **em坐标系**（例如，范围 `[0, 1000]`）。
   - 在生成纹理数据时，这些坐标会被归一化到 `[0, 1]` 范围内。例如，将每个坐标除以 `1000`，得到归一化的值。
   - 归一化后的坐标存储在纹理中（如 `uAtlasSampler`），供片段着色器使用。

2. **片段着色器中的坐标转换**：
   在 `fetchBezier` 函数中，控制点坐标从纹理中读取后，会减去 `vNormCoord`（当前像素的归一化坐标）：

   ```glsl
   void fetchBezier(int coordIndex, out vec2 p[3]) {
       for (int i=0; i<3; i++) {
           p[i] = fetchVec2(...) - vNormCoord; // 将控制点转换为局部坐标
       }
   }
   ```

   - `vNormCoord` 是从顶点着色器传递的归一化坐标，范围 `[0, 1]`。
   - 通过减去 `vNormCoord`，控制点坐标被转换为相对于当前像素的局部坐标，方便后续计算。

3. **最终统一**：
   - 所有计算（如交点检测、抗锯齿积分）都在归一化后的局部坐标系中进行，无需直接处理em单位。

---

### 问题2：从像素中心采样的实现

**像素中心采样**是图形渲染的默认行为，通过以下代码体现：

1. **顶点着色器的坐标映射**：
   在顶点着色器（`web.vert`）中，顶点坐标通过以下公式映射到屏幕空间：

   ```glsl
   vec2 pos = aPosition;
   pos.y = 1.0 - pos.y; // 翻转Y轴（纹理坐标系）
   pos = pos * uPositionMul + uPositionAdd; // 缩放和平移
   gl_Position = vec4(pos, 0.0, 1.0);
   gl_Position.x *= uCanvasSize.y / uCanvasSize.x; // 保持宽高比
   ```

   - `aPosition` 是顶点原始坐标（例如，四边形的四个角点）。
   - 经过变换后，顶点坐标被映射到 `[-1, 1]` 的裁剪空间，**片段着色器中的插值坐标 `vNormCoord` 会自动落在像素中心**。

2. **片段着色器的网格定位**：
   在 `font.frag` 中，通过 `vNormCoord` 计算当前像素所在的网格单元：

   ```glsl
   vec2 integerCell = floor(clamp(vNormCoord * vGridSize, vec2(0.5), vec2(vGridSize)-0.5));
   ```

   - `vNormCoord` 是插值后的归一化坐标，对应像素中心。
   - `vNormCoord * vGridSize` 将归一化坐标映射到网格坐标系，`floor` 确保取整到当前网格单元。

3. **抗锯齿采样的实现**：
   在抗锯齿循环中，通过旋转贝塞尔曲线模拟不同角度的采样，但采样点始终基于像素中心：
   ```glsl
   mat2 initrot = inverse(mat2(dFdx(vNormCoord) * kPixelWindowSize, dFdy(vNormCoord) * kPixelWindowSize));
   ```
   - `dFdx` 和 `dFdy` 计算屏幕空间的导数，确保采样窗口围绕像素中心展开。

---

### 总结

1. **坐标统一**：
   - em坐标通过预处理归一化到 `[0, 1]`，片段着色器中的所有计算均在归一化坐标系中进行。
2. **像素中心采样**：
   - 顶点着色器的坐标映射和片段插值确保 `vNormCoord` 位于像素中心。
   - 网格定位和抗锯齿逻辑均基于像素中心展开。
