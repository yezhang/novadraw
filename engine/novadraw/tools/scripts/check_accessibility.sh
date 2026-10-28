#!/bin/bash
# check_accessibility.sh - 检查并提示 Accessibility 权限

# 检查是否有 Accessibility 权限
if [[ -z "$(osascript -e 'tell application "System Events" to return first process whose frontmost is true' 2>/dev/null)" ]]; then
    echo "无法检查权限，可能需要先授予权限"
fi

# 打开 Accessibility 隐私设置
open "x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility"

echo "请在弹出的系统设置中添加并启用此应用"
echo "路径: 系统设置 → 隐私与安全性 → 辅助功能"
