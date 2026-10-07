#!/usr/bin/env python3
# aicross 图标流水线（PNG 母版）
#
# 两张母版：icons/icon-master.png（1024，两行 ai / cross，用于 128 及以上）与
# icons/icon-master-small.png（512，同一图案，用于 64 及以下；需要小尺寸专用图案
# 时换这张）。字体 Leckerli One（SIL OFL），绿色对角渐变 #5ACE8D → #219E60。
# 先运行 `npx -p @tauri-apps/cli@2 tauri icon icons/icon-master.png` 生成各平台
# 文件（icns、Square*Logo、android 等），再运行本脚本覆盖 tauri.conf.json 列出的
# PNG 与 icon.ico。
#
# Run: python scripts/rebuild-icons.py

from pathlib import Path
from PIL import Image

REPO = Path(__file__).resolve().parent.parent
ICONS = REPO / "icons"
SMALL_MAX = 64

PNG_SIZES = {
    "32x32.png": 32,
    "64x64.png": 64,
    "128x128.png": 128,
    "128x128@2x.png": 256,
    "256x256.png": 256,
    "512x512.png": 512,
    "icon.png": 512,
}
ICO_SIZES = [16, 20, 24, 32, 40, 48, 64, 128, 256]


def render(master: Image.Image, small: Image.Image, size: int) -> Image.Image:
    source = small if size <= SMALL_MAX else master
    return source.resize((size, size), Image.LANCZOS)


def main() -> None:
    master = Image.open(ICONS / "icon-master.png").convert("RGBA")
    small = Image.open(ICONS / "icon-master-small.png").convert("RGBA")
    for name, size in PNG_SIZES.items():
        render(master, small, size).save(ICONS / name)
    frames = [render(master, small, size) for size in ICO_SIZES]
    frames[-1].save(ICONS / "icon.ico", format="ICO", sizes=[(s, s) for s in ICO_SIZES],
                    append_images=frames[:-1])
    render(master, small, 64).save(REPO / "src-ui" / "public" / "favicon.png")
    print("icons rebuilt")


if __name__ == "__main__":
    main()
