# -*- coding: utf-8 -*-
"""src-ui/src 文案体检：提取用户可见中文字符串，按 AGENTS.md「界面文案与命名规范」检查。
只报告，不修改。误报由人工复核。

用法：python scripts/copy-audit.py [--tests]
  默认跳过 __tests__ 与 i18n 非中文语种；--tests 连测试一起扫。
移植自 skillmanager/scripts/copy-audit.py，规则后半段是本仓的命名决定。"""
import re, sys, io
from pathlib import Path

sys.stdout = io.TextIOWrapper(sys.stdout.buffer, encoding="utf-8")

ROOT = Path(__file__).resolve().parent.parent / "src-ui" / "src"
WITH_TESTS = "--tests" in sys.argv
HAN = "一-鿿"


def strip_comments(src: str) -> str:
    src = re.sub(r"/\*.*?\*/", " ", src, flags=re.S)
    src = re.sub(r"^\s*//.*$", " ", src, flags=re.M)
    src = re.sub(r"(?<=[\s{(\[,:;])//[^\"'`\n]*$", " ", src, flags=re.M)
    return src


def extract(path: Path):
    raw = path.read_bytes().replace(b"\x00", b"").decode("utf-8", "replace")
    src = strip_comments(raw)
    out = []
    # JSX 文本：> 与 < 之间
    for m in re.finditer(r">([^<>{}]*[" + HAN + r"][^<>]*)<", src):
        t = m.group(1).strip()
        if t:
            out.append(t)
    # 字符串字面量与模板串
    for m in re.finditer(r"'([^'\\\n]*[" + HAN + r"][^'\\\n]*)'", src):
        out.append(m.group(1).strip())
    for m in re.finditer(r'"([^"\\\n]*[' + HAN + r'][^"\\\n]*)"', src):
        out.append(m.group(1).strip())
    for m in re.finditer(r"`([^`\\]*[" + HAN + r"][^`\\]*)`", src):
        out.append(m.group(1).strip())
    seen, uniq = set(), []
    for t in out:
        if t not in seen:
            seen.add(t)
            uniq.append(t)
    return uniq


# 规则：名称, 正则。前半段沿用 skillmanager 通用规则，后半段是本仓命名决定。
RULES = [
    ("口语-语气词", r"[呗哈哦呀啦嘛]$|[，,][呗哈哦呀啦嘛]"),
    ("口语-连接词", r"其实|说白了|换句话说|简单来说|当然了|毕竟"),
    ("口语-劝导", r"看一眼|瞧一瞧|记得|别忘了|放心|轻轻松松|省事|就完了|那儿|这里的话"),
    ("口语-得/先", r"得先|得自己|你得|你得先"),
    ("第一人称", r"我(们|已经|看|帮|给)?(?![们法])"),
    ("第二人称", r"你(们|的|自己|机器)?(?![好们])"),
    ("聊天腔-欢迎", r"欢迎|你好|让我们|一起来"),
    ("机器腔-才能方可", r"才能|方可|才可|才可以"),
    ("机器腔-只有才", r"只有[^,;。;]{0,12}才"),
    ("机器腔-仍", r"仍有|仍需|尚未|暂未"),
    ("机器腔-军语", r"暂无目标|无处可|落库|入库失败|归一|外溢|门禁|脱管|幂等|物化"),
    ("机器腔-双否", r"不[^,;。;]{1,6}不[^,;。;的]"),
    ("机制词-作用域", r"作用域"),
    ("英文式破折号", r"——|──|—"),
    ("弯引号", r"“|”|‘|’"),
    ("中英标点混用", r"[" + HAN + r"）」】][,;:?!]"),
    ("营销词", r"永远|极致|深度赋能|一站式|强大的|轻松搞定|完美"),
    ("拟人化", r"智能地|努力|伪装|自动帮"),
    # 本仓命名决定（AGENTS.md 界面文案与命名规范）
    ("内部词", r"留痕|对账|口径|判定|闸门|落库|\bwire\b|\bG[1-4]\b|CAID|\bR[1-9]\b"),
    ("任务编号", r"\b(TASK|REPORT|fix|impl)-\d|\b0\d[a-dA-D]\b|第 ?\d+ ?期|期次|本期|下期"),
    ("动作词-冒烟", r"冒烟"),
    ("动作词-添加", r"添加"),
    ("动作词-不统一", r"下发|分派|指派|合入|中止|终止"),
    ("预设命名", r"（我的）|\(我的\)|评审团|红蓝(?!模式)|群聊(?!模式)|后续期次"),
    ("预设命名-缺模式", r"(?<![" + HAN + r"])(盲审|擂台|评审|蜂群)(?!模式)(?![" + HAN + r"])"),
]


def rule_hit(rule, text):
    name, pat = rule
    return re.search(pat, text)


def wanted(f: Path) -> bool:
    rel = f.relative_to(ROOT).as_posix()
    if not WITH_TESTS and ("__tests__" in rel or rel.endswith(".test.ts") or rel.endswith(".test.tsx")):
        return False
    if rel.startswith("i18n/") and not rel.startswith("i18n/zh"):
        return False
    return True


hits_total = 0
files_hit = 0
for f in sorted(list(ROOT.rglob("*.tsx")) + list(ROOT.rglob("*.ts"))):
    if not wanted(f):
        continue
    strings = extract(f)
    hits = []
    for t in strings:
        for rule in RULES:
            m = rule_hit(rule, t)
            if m:
                hits.append((rule[0], m.group(0), t))
    if hits:
        files_hit += 1
        print(f"\n=== {f.relative_to(ROOT.parent)} ===")
        for name, frag, t in hits:
            hits_total += 1
            print(f"  [{name}] «{frag}» → {t[:90]}")
print(f"\n共 {hits_total} 处命中，{files_hit} 个文件（含误报，需人工复核）")
