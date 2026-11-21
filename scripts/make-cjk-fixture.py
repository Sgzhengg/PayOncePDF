#!/usr/bin/env python3
"""生成真正内嵌中文字体的 PDF 样本：tests/fixtures/text_cjk_real.pdf

为什么需要这个脚本
------------------
`generate-fixtures.py` 受「仅用 Python 标准库」的红线约束，无法内嵌 CID 字体，
它产出的 `text_cjk_special.pdf` 里中文只是原始 UTF-8 字节，按 WinAnsi 解码后
必然是乱码（见该文件的 FONT_LIMIT_NOTE）。这不是渲染缺陷，但意味着我们一直
没有**真实中文 PDF** 可以验证渲染与就地改字。

本脚本是开发期辅助工具（不属于运行时代码，也不参与构建），做三件事：
  1. 用 fontTools 把系统中文字体裁剪到只含用到的字符（子集化，产物几十 KB）；
  2. 用标准库手工拼一个 Type0 / CIDFontType2 / Identity-H 的 PDF，
     带 ToUnicode CMap（PDFium 靠它还原文字层，改字与复制粘贴都依赖它）；
  3. 写出 `tests/fixtures/text_cjk_real.pdf`，供 Rust 单测与真机手册使用。

依赖：pip install fonttools；字体：Windows 自带 simhei.ttf（缺失时回退其他系统字体）。
产物只用于本机测试，不分发；字体子集来自用户本机字体，不随仓库对外发布。
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
OUT = REPO_ROOT / "tests" / "fixtures" / "text_cjk_real.pdf"

FONT_CANDIDATES = [
    Path(os.environ.get("WINDIR", r"C:\Windows")) / "Fonts" / "simhei.ttf",
    Path(os.environ.get("WINDIR", r"C:\Windows")) / "Fonts" / "Deng.ttf",
    Path("/System/Library/Fonts/PingFang.ttc"),
    Path("/usr/share/fonts/truetype/noto/NotoSansCJK-Regular.ttc"),
]

# 三行：纯中文 / 中英混排 / 需要就地改错字的真实场景
LINES = [
    "中文测试：就地改错字，不转 Word",
    "合同编号：2026 年第 018 号",
    "付款期限为 30 日，逾期按日计息。",
]
REPLACE_DEMO = ("30 日", "45 日")  # 真机改字演示：把 30 日改成 45 日


def pick_font() -> Path:
    for p in FONT_CANDIDATES:
        if p.is_file():
            return p
    sys.exit("找不到可用的中文字体（尝试过：%s）" % ", ".join(map(str, FONT_CANDIDATES)))


def subset_font(src: Path, chars: str) -> tuple[bytes, dict[int, int], dict[int, int], int]:
    """裁剪字体，返回 (ttf 字节, {字符: GID}, {GID: 宽度(1000/em)}, unitsPerEm)。"""
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

    # 重新读回子集，拿 GID 与宽度（子集后 GID 会重排，必须用子集自己的 cmap）
    sub = TTFont(io.BytesIO(ttf))
    upem = sub["head"].unitsPerEm
    cmap: dict[str, int] = {}
    for table in sub["cmap"].tables:
        if table.isUnicode():
            for cp, name in table.cmap.items():
                cmap.setdefault(chr(cp), sub.getGlyphID(name))
    hmtx = sub["hmtx"]
    order = sub.getGlyphOrder()
    widths = {}
    for gid, name in enumerate(order):
        adv = hmtx[name][0] if name in hmtx.metrics else upem
        widths[gid] = int(round(adv * 1000 / upem))
    conved = {ch: cmap[ch] for ch in chars if ch in cmap}
    missing = [ch for ch in chars if ch not in conved and not ch.isspace()]
    if missing:
        sys.exit(f"字体缺少字形：{''.join(missing)}")
    print(f"  子集字体 {len(before)} -> {len(order)} 字形，{len(ttf)} 字节，upem={upem}")
    return ttf, conved, widths, upem


def pdf_string(s: str) -> bytes:
    return s.encode("latin-1", "replace")


def build_pdf(ttf: bytes, gids: dict[int, int], widths: dict[int, int], upem: int) -> bytes:
    objs: list[bytes] = []

    def add(body: bytes) -> int:
        objs.append(body)
        return len(objs)

    font_stream = zlib.compress(ttf, 9)
    fdesc = add(
        b"<< /Type /FontDescriptor /FontName /PayOnceCJK /Flags 4 "
        b"/FontBBox [-120 -220 1000 900] /ItalicAngle 0 /Ascent 880 /Descent -120 "
        b"/CapHeight 720 /StemV 80 /FontFile2 %d 0 R >>" % 0
    )
    ff = add(
        b"<< /Length %d /Length1 %d /Filter /FlateDecode >>\nstream\n"
        % (len(font_stream), len(ttf))
        + font_stream
        + b"\nendstream"
    )
    objs[fdesc - 1] = objs[fdesc - 1].replace(b"%d 0 R" % 0, b"%d 0 R" % ff, 1)

    used = sorted({g for g in gids.values()})
    w_arr = " ".join(f"{g} [{widths.get(g, 1000)}]" for g in used).encode()
    cidfont = add(
        b"<< /Type /Font /Subtype /CIDFontType2 /BaseFont /PayOnceCJK "
        b"/CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> "
        b"/FontDescriptor %d 0 R /DW 1000 /W [%s] /CIDToGIDMap /Identity >>"
        % (fdesc, w_arr)
    )

    # ToUnicode：GID -> Unicode，PDFium 靠它建立文字层（复制、查找、改字都要用）
    bf = "".join(f"<{gid:04X}> <{ord(ch):04X}>\n" for ch, gid in sorted(gids.items(), key=lambda kv: kv[1]))
    tounicode = (
        "/CIDInit /ProcSet findresource begin\n12 dict begin\nbegincmap\n"
        "/CIDSystemInfo << /Registry (Adobe) /Ordering (UCS) /Supplement 0 >> def\n"
        "/CMapName /Adobe-Identity-UCS def\n/CMapType 2 def\n"
        "1 begincodespacerange\n<0000> <FFFF>\nendcodespacerange\n"
        f"{len(gids)} beginbfchar\n{bf}endbfchar\n"
        "endcmap\nCMapName currentdict /CMap defineresource pop\nend\nend\n"
    ).encode()
    tu = add(
        b"<< /Length %d >>\nstream\n" % len(tounicode) + tounicode + b"\nendstream"
    )

    type0 = add(
        b"<< /Type /Font /Subtype /Type0 /BaseFont /PayOnceCJK /Encoding /Identity-H "
        b"/DescendantFonts [%d 0 R] /ToUnicode %d 0 R >>" % (cidfont, tu)
    )

    content_lines = []
    y = 760
    for line in LINES:
        hexstr = "".join(f"{gids[ch]:04X}" for ch in line if ch in gids)
        content_lines.append(f"BT /F1 15 Tf 72 {y} Td <{hexstr}> Tj ET")
        y -= 30
    content = ("\n".join(content_lines) + "\n").encode()
    cstream = zlib.compress(content, 9)
    cref = add(
        b"<< /Length %d /Filter /FlateDecode >>\nstream\n" % len(cstream)
        + cstream
        + b"\nendstream"
    )

    pages_id = len(objs) + 2
    page = add(
        b"<< /Type /Page /Parent %d 0 R /MediaBox [0 0 595 842] "
        b"/Resources << /Font << /F1 %d 0 R >> >> /Contents %d 0 R >>"
        % (pages_id, type0, cref)
    )
    pages = add(
        b"<< /Type /Pages /Count 1 /Kids [%d 0 R] >>" % page
    )
    catalog = add(b"<< /Type /Catalog /Pages %d 0 R >>" % pages)

    out = bytearray(b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n")
    offsets = [0]
    for i, body in enumerate(objs, start=1):
        offsets.append(len(out))
        out += b"%d 0 obj\n" % i + body + b"\nendobj\n"
    xref = len(out)
    out += b"xref\n0 %d\n" % (len(objs) + 1)
    out += b"0000000000 65535 f \n"
    for off in offsets[1:]:
        out += b"%010d 00000 n \n" % off
    out += (
        b"trailer\n<< /Size %d /Root %d 0 R >>\nstartxref\n%d\n%%%%EOF\n"
        % (len(objs) + 1, catalog, xref)
    )
    return bytes(out)


def main() -> None:
    src = pick_font()
    print(f"字体: {src}")
    chars = "".join(LINES) + REPLACE_DEMO[0] + REPLACE_DEMO[1] + "0123456789 "
    ttf, gids, widths, upem = subset_font(src, chars)
    pdf = build_pdf(ttf, gids, widths, upem)
    OUT.write_bytes(pdf)
    print(f"写出 {OUT.relative_to(REPO_ROOT)}：{len(pdf)} 字节")
    for line in LINES:
        print(f"  行：{line}")


if __name__ == "__main__":
    main()
