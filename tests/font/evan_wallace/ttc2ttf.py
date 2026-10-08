from fontTools.ttLib.ttCollection import TTCollection
import os

# import sys

filename = "./PingFang.ttc"  # sys.argv[1]  # 替换为真实的文件路径
ttc = TTCollection(filename)
basename = os.path.basename(filename)
for i, font in enumerate(ttc):
    font_fullname = font["name"].getDebugName(4)
    font.save(f"{basename}#{font_fullname}.ttf")
    print(f"{font_fullname} 保存成功")
