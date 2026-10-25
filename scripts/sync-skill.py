"""把 ai-cross skill 源仓同步进 skill-bundle/ai-cross/（随版本内置进可执行文件）。

用法：python scripts/sync-skill.py [源目录]
源目录缺省为 D:\\work\\skill\\xiaoyu-skill\\skills\\ai-cross。同步是整份替换：
目标目录先删后建；源目录只读，不会被改动。排除项：任何层级的 tests/ 目录
与以 .git 开头的文件或目录。
"""

import argparse
import shutil
import sys
from pathlib import Path

DEFAULT_SOURCE = r"D:\work\skill\xiaoyu-skill\skills\ai-cross"
EXCLUDED_DIR_NAMES = {"tests", "__pycache__", ".pytest_cache"}
EXCLUDED_NAME_PREFIXES = (".git",)
EXCLUDED_SUFFIXES = (".pyc",)


def excluded(name: str) -> bool:
    return (
        name in EXCLUDED_DIR_NAMES
        or name.startswith(EXCLUDED_NAME_PREFIXES)
        or name.endswith(EXCLUDED_SUFFIXES)
    )


def copy_tree(source: Path, target: Path) -> int:
    copied = 0
    for entry in sorted(source.iterdir()):
        if excluded(entry.name):
            continue
        destination = target / entry.name
        if entry.is_dir():
            destination.mkdir(parents=True, exist_ok=True)
            copied += copy_tree(entry, destination)
        elif entry.is_file():
            shutil.copy2(entry, destination)
            copied += 1
    return copied


def main() -> None:
    parser = argparse.ArgumentParser(description="同步 skill 源仓到 skill-bundle/ai-cross/")
    parser.add_argument("source", nargs="?", default=DEFAULT_SOURCE, help="skill 源目录")
    arguments = parser.parse_args()
    source = Path(arguments.source)
    if not (source / "SKILL.md").is_file():
        sys.exit(f"源目录里没有 SKILL.md：{source}")
    target = Path(__file__).resolve().parent.parent / "skill-bundle" / "ai-cross"
    if target.exists():
        shutil.rmtree(target)
    target.mkdir(parents=True)
    copied = copy_tree(source, target)
    print(f"已同步 {copied} 个文件：{source} -> {target}")


if __name__ == "__main__":
    main()
