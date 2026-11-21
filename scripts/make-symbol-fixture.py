#!/usr/bin/env python3
"""生成「英文 / 科学符号 / 数学公式」PDF 样本：tests/fixtures/text_symbols.pdf

为什么需要这个脚本
------------------
F19（按矩形精确替换）此前只用中文样本验证过。用户接着问：英文、科学符号、
数学公式能不能改？这些问题不能靠推理回答，必须拿真实 PDF 跑一遍。

本脚本造一页一场景的样本，覆盖四条能力边界：

  第 1 页  英文正文        Helvetica 内置 + WinAnsi，不嵌字体
  第 2 页  Latin-1 符号    ± ° µ × ÷（WinAnsi 里本来就有这些字节）
  第 3 页  数学符号        ≈ ≤ ≥ ∑ ∫ √ π Ω（Latin-1 之外，必须内嵌 Arial 子集）
  第 4 页  上下标          H₂O / x² + y² = z²（小字号 + 基线偏移的独立文本对象）
  第 5 页  分式            1 / 2（分子、横线路径、分母三个独立对象）

关键设计：第 3 页的字体子集**故意不包含 ≠**，用来验证「改成一个字体里没有的字」
时程序会拒绝、且绝不写出坏文件；而把 3.14 改成 2.72（数字都在子集里）应当成功。

依赖：pip install fonttools；字体用 Windows 自带 arial.ttf。
产物只用于本机测试，不分发。
"""

from __future__ import annotations

import io
import os
import sys
import zlib
from pathlib import Path

try:
    from fontTools import subset
    from fontTools.ttLib import TTFont
except ImportError:  # pragma: no cover - 开发期工具
    sys.exit("需要 fontTools：python -m pip install fonttools")

REPO_ROOT = Path(__file__).resolve().parent.parent
OUT = REPO_ROOT / "tests" / "fixtures" / "text_symbols.pdf"

FONT_CANDIDATES = [
    Path(os.environ.get("WINDIR", r"C:\Windows")) / "Fonts" / "arial.ttf",
    Path("/System/Library/Fonts/Supplemental/Arial.ttf"),
    Path("/usr/share/fonts/truetype/dejavu/DejaVuSans.ttf"),
]

# 每页的文本块：(字体 key, 字号, 相对基线偏移, 文字)
# y 由页面起始基线逐行递减；上标用 dy>0 抬高、下标用 dy<0 压低。
PAGES: list[dict] = [
    {
        "name": "en",
        "lines": [[("F1", 15, 0.0, "Invoice No. 30 - Total: USD 1,250.00")],
                  [("F1", 13, 0.0, "Payment due in 30 days from issue.")]],
    },
    {
        "name": "latin1",
        "lines": [[("F1", 14, 0.0, "Temp 25 \u00b1 2 \u00b0C | 5 \u00b5L | 3 \u00d7 4 \u00f7 2")],
                  [("F1", 13, 0.0, "Yield 98\u00b7 5%, tolerance \u00b1 0.2 mm.")]],
    },
    {
        # 内嵌 Arial 子集：Latin-1 之外的数学符号
        "name": "math",
        "lines": [[("F2", 15, 0.0, "3 \u2248 3.14, x \u2264 5, y \u2265 2")],
                  [("F2", 15, 0.0, "\u2211 \u222b \u221a\u03c0, \u03a9 \u0394")]],
    },
    {
        # 上下标：小字号 + 基线偏移，且必须是独立文本对象（真实 PDF 就是这样）
        "name": "supsub",
        "lines": [[("F2", 15, 0.0, "H"), ("F2", 9, 5.0, "2"), ("F2", 15, 0.0, "O and x"),
                   ("F2", 9, 5.0, "2"), ("F2", 15, 0.0, " + y"), ("F2", 9, 5.0, "2"),
                   ("F2", 15, 0.0, " = z"), ("F2", 9, 5.0, "2")],
                  [("F2", 15, 0.0, "Water is H"), ("F2", 9, -4.0, "2"),
                   ("F2", 15, 0.0, "O at 25 \u00b0C.")]],
    },
    {
        # 分式：分子 / 横线路径 / 分母，三者是独立对象
        "name": "fraction",
        "lines": [[("F2", 15, 0.0, "Result: ")]],
        "fraction": True,
    },
]


def pick_font() -> Path:
    for p in FONT_CANDIDATES:
        if p.is_file():
            return p
    sys.exit("找不到可用字体（尝试过：%s）" % ", ".join(map(str, FONT_CANDIDATES)))


def all_chars() -> str:
    s = set()
    for page in PAGES:
        for line in page["lines"]:
            for _, _, _, text in line:
                s |= set(text)
    # 分式用到的字符
    s |= set("12 Result:")
    # 只把「应当能改成功」的目标字符补进子集；≠ 故意不加，用于验证缺字形被拦。
    s |= set("3.14 2.72 - 45 ")
    return "".join(sorted(s))


def subset_font(src: Path, chars: str):
    """裁剪字体，返回 (ttf 字节, {字符: GID}, {GID: 宽度/1000em})。"""
    options = subset.Options()
    options.desubroutinize = True
    options.drop_tables += ["DSIG"]
    options.layout_features = []
    font = TTFont(str(src), fontNumber=0)
    before = set(font.getGlyphOrder())
    subsetter = subset.Subsetter(options=options)
    subsetter.populate(text=chars)
    subsetter.subset(font)
    buf = io.BytesIO()
    font.save(buf)
    ttf = buf.getvalue()

    sub = TTFont(io.BytesIO(ttf))
    upem = sub["head"].unitsPerEm
    cmap: dict[str, int] = {}
    for table in sub["cmap"].tables:
        if table.isUnicode():
            for cp, name in table.cmap.items():
                cmap.setdefault(chr(cp), sub.getGlyphID(name))
    hmtx = sub["hmtx"]
    widths = {}
    for gid, name in enumerate(sub.getGlyphOrder()):
        adv = hmtx[name][0] if name in hmtx.metrics else upem
        widths[gid] = int(round(adv * 1000 / upem))
    conved = {ch: cmap[ch] for ch in chars if ch in cmap}
    missing = [ch for ch in chars if ch not in conved and not ch.isspace()]
    if missing:
        sys.exit(f"字体缺少字形：{''.join(missing)}")
    print(f"  子集字体 {len(before)} -> {len(sub.getGlyphOrder())} 字形，{len(ttf)} 字节")
    return ttf, conved, widths


class Doc:
    def __init__(self) -> None:
        self.objs: list[bytes] = []

    def add(self, body: bytes) -> int:
        self.objs.append(body)
        return len(self.objs)

    def set(self, number: int, body: bytes) -> None:
        self.objs[number - 1] = body

    def finish(self, root: int) -> bytes:
        out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
        offsets = [0]
        for i, body in enumerate(self.objs, start=1):
            offsets.append(len(out))
            out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
        xref = len(out)
        out += b"xref\n0 %d\n" % (len(self.objs) + 1)
        out += b"0000000000 65535 f \n"
        for off in offsets[1:]:
            out += b"%010d 00000 n \n" % off
        out += (b"trailer\n<< /Size %d /Root %d 0 R >>\nstartxref\n%d\n%%%%EOF\n"
                % (len(self.objs) + 1, root, xref))
        return bytes(out)


def winansi_bytes(s: str) -> bytes:
    """WinAnsiEncoding：Latin-1 直接落字节；PDF 里用八进制转义避免括号/反斜杠问题。"""
    out = []
    for ch in s:
        b = ch.encode("latin-1")
        o = b[0]
        if o in (0x28, 0x29, 0x5C):  # ( ) \
            out.append("\\" + chr(o))
        elif 0x20 <= o <= 0x7E:
            out.append(ch)
        else:
            out.append("\\%03o" % o)
    return "".join(out).encode("ascii")


def build_pdf(ttf: bytes, gids: dict[str, int], widths: dict[int, int]) -> bytes:
    doc = Doc()

    fdesc = doc.add(b"<< >>")  # 占位，稍后回填 FontFile2
    fstream = zlib.compress(ttf, 9)
    ff = doc.add(b"<< /Length %d /Length1 %d /Filter /FlateDecode >>\nstream\n"
                 % (len(fstream), len(ttf)) + fstream + b"\nendstream")
    doc.set(fdesc,
            b"<< /Type /FontDescriptor /FontName /PayOnceSym /Flags 32 "
            b"/FontBBox [-100 -220 1100 900] /ItalicAngle 0 /Ascent 900 /Descent -210 "
            b"/CapHeight 700 /StemV 80 /FontFile2 %d 0 R >>" % ff)

    used = sorted(set(gids.values()))
    w_arr = " ".join(f"{g} [{widths.get(g, 1000)}]" for g in used).encode()
    cidfont = doc.add(
        b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /PayOnceSym "
        b"/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> "
        b"/FontDescriptor %d 0 R /DW 1000 /W [%s] /CIDToGIDMap /Identity >>" % (fdesc, w_arr))

    bf = "".join(f"<{gid:04X}> <{ord(ch):04X}>\n"
                 for ch, gid in sorted(gids.items(), key=lambda kv: kv[1]))
    tounicode = ("/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n"
                 "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n"
                 "/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n"
                 "1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n"
                 f"{len(gids)} beginbfchar\n{bf}endbfchar\n"
                 "endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n").encode()
    tu = doc.add(b"<< /Length %d >>\nstream\n" % len(tounicode) + tounicode + b"\nendstream")

    type0 = doc.add(
        b"<< /Type /Font /Subtype /Type0 /BaseFont /PayOnceSym /Encoding /Identity-H "
        b"/DescendantFonts [%d 0 R] /ToUnicode %d 0 R >>" % (cidfont, tu))
    helv = doc.add(b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>")

    def hex_str(s: str) -> bytes:
        return ("".join(f"{gids[ch]:04X}" for ch in s if ch in gids)).encode()

    # 内容流按页生成；每页一组独立 BT/ET（真实 PDF 里一行常被拆成多个文本对象）
    contents: list[bytes] = []
    for page in PAGES:
        ops: list[str] = []
        y = 750.0
        for line in page["lines"]:
            x = 72.0
            for font_key, size, dy, text in line:
                fid = "/F1" if font_key == "F1" else "/F2"
                if font_key == "F1":
                    ops.append(f"BT {fid} {size} Tf {x:.2f} {y + dy:.2f} Td "
                               f"({winansi_bytes(text).decode()}) Tj ET")
                else:
                    ops.append(f"BT {fid} {size} Tf {x:.2f} {y + dy:.2f} Td "
                               f"<{hex_str(text).decode()}> Tj ET")
                # 用 /1000 em 宽度推进 x（Helvetica 与 Arial 度量几乎一致）
                for ch in text:
                    g = gids.get(ch)
                    adv = widths.get(g, 556) if g is not None else 556
                    x += adv / 1000.0 * size
            y -= 34.0
        if page.get("fraction"):
            # 分式：分子 1（上）、横线（路径）、分母 2（下），三者互不相干
            ops.append(f"BT /F2 15 Tf 120.00 736.00 Td <{hex_str('1').decode()}> Tj ET")
            ops.append("100 722 m 112 722 l 112 723 l 100 723 l h f")
            ops.append(f"BT /F2 15 Tf 120.00 700.00 Td <{hex_str('2').decode()}> Tj ET")
            ops.append(f"BT /F2 13 Tf 72.00 660.00 Td <{hex_str('Result: 1 / 2 done').decode()}> Tj ET")
        contents.append(("\n".join(ops) + "\n").encode())

    to_resolve = []
    page_ids = []
    for _ in PAGES:
        cstream = zlib.compress(b"", 9)
        cref = doc.add(b"<< /Length %d /Filter /FlateDecode >>\nstream\n" % len(cstream)
                       + cstream + b"\nendstream")
        pid = doc.add(b"<< >>")
        to_resolve.append((pid, cref))
        page_ids.append(pid)
    pages_id = doc.add(b"<< >>")
    for (pid, cref), content in zip(to_resolve, contents):
        cstream = zlib.compress(content, 9)
        doc.set(cref, b"<< /Length %d /Filter /FlateDecode >>\nstream\n" % len(cstream)
                + cstream + b"\nendstream")
        doc.set(pid,
                b"<< /Type /Page /Parent %d 0 R /MediaBox [0 0 595 842] "
                b"/Resources << /Font << /F1 %d 0 R /F2 %d 0 R >> >> /Contents %d 0 R >>"
                % (pages_id, helv, type0, cref))
    doc.set(pages_id, b"<< /Type /Pages /Count %d /Kids [%s] >>"
            % (len(page_ids), b" ".join(b"%d 0 R" % p for p in page_ids)))
    catalog = doc.add(b"<< /Type /Catalog /Pages %d 0 R >>" % pages_id)
    return doc.finish(catalog)


def main() -> None:
    src = pick_font()
    print(f"字体: {src}")
    chars = all_chars()
    ttf, gids, widths = subset_font(src, chars)
    pdf = build_pdf(ttf, gids, widths)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(pdf)
    print(f"写出 {OUT.relative_to(REPO_ROOT)}：{len(pdf)} 字节，{len(PAGES)} 页")
    for i, page in enumerate(PAGES, start=1):
        head = " / ".join("".join(t[3] for t in line) for line in page["lines"])
        print(f"  第 {i} 页 {page['name']}: {head}{'  [含分式]' if page.get('fraction') else ''}")


if __name__ == "__main__":
    main()
