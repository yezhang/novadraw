#!/bin/bash
# Draw2D 示例程序启动脚本
# 用法: ./run.sh [类名]
# 示例:
#   ./run.sh                          # 默认运行 HelloWorld
#   ./run.sh HelloWorld               # 运行 HelloWorld
#   ./run.sh org.example.draw2d.ShapesDemo  # 运行 ShapesDemo（带完整包名）

# 切换到脚本所在目录
cd "$(dirname "$0")"

# 获取要运行的类名（默认为 HelloWorld）
CLASS_NAME="${1:-HelloWorld}"

# 如果类名不包含点，添加默认包名
if [[ "$CLASS_NAME" != *.* ]]; then
    CLASS_NAME="org.example.draw2d.$CLASS_NAME"
fi

echo "编译中..."
mvn compile -q

# 构建 classpath
CP="target/classes"
CP="${CP}:target/lib/*"
CP="${CP}:lib/*"

echo "运行: $CLASS_NAME"

# 运行（-XstartOnFirstThread 是 macOS 必需的）
java -XstartOnFirstThread -cp "${CP}" "$CLASS_NAME"
