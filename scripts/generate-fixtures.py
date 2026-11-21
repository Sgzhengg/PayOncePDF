#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
generate-fixtures.py - PayOnce PDF 测试样本生成器（纯 Python 标准库，无第三方依赖）

用途
    依据 PRD 14.1 / 14.2 与 CONTRACT.md 第 8 节，生成 tests/fixtures/ 下的
    测试 PDF、PNG 图片序列与批量报告样本，并写出 manifest.json 供
    tests/acceptance.py 做页数、顺序、体积、错误码等断言。
    所有文件可重复生成：固定随机种子、不写入时间戳，同一组参数产出逐字节一致的结果。

用法
    python scripts/generate-fixtures.py                 # 生成基础样本（1/100/500 页等）
    python scripts/generate-fixtures.py --out DIR      # 指定输出目录（默认 <repo>/tests/fixtures）
    python scripts/generate-fixtures.py --huge         # 额外生成 5000 页样本
    python scripts/generate-fixtures.py --big          # 额外生成 20MB 级大扫描件
    python scripts/generate-fixtures.py --huge --big   # 全量
    python scripts/generate-fixtures.py --help

生成清单（写入 out 目录）
    page_1.pdf / page_100.pdf / page_500.pdf          基础多页 PDF（每页含 PAYONCEPDF_PAGE_NNNN 标记）
    page_5000.pdf                                     --huge 时生成
    encrypted.placeholder.pdf                         加密占位样本（非真加密；真实加密件须人工用 qpdf 生成，命令见 manifest.json）
    scan_gray.pdf / scan_color.pdf                    扫描件风格：zlib/FlateDecode 内嵌灰度/彩色图像 XObject + 表格线条 + 文字层
    scan_big.pdf                                      20MB 级大扫描件（--big）
    corrupted.pdf                                     截断损坏样本
    text_cjk_special.pdf                              中文 / 特殊符号样本（注明字体限制）
    images/page_01.png .. images/page_03.png          PNG 图片序列（纯 stdlib 生成）
    report.csv                                        批量处理报告样本（字段与错误码枚举由 acceptance.py 校验）
    manifest.json                                     样本清单（类别、页数、字节数、sha256、备注）

已知限制（均写入 manifest.json）
    - 加密 PDF：stdlib 不实现 PDF 标准加密，占位文件不是真加密；给出 qpdf 命令由人工生成。
    - JPG：stdlib 无 JPEG 编码器，图片序列只有 PNG；JPG 样本跳过并说明。
    - 中文：生成的 PDF 不内嵌 CJK 字体，中文字形依赖查看器的字体回退，可能显示为方框。
    - 20MB 大扫描件：由伪随机（近似不可压缩）灰度像素经 FlateDecode 内嵌，体积约 20MB。

工程红线
    - 仅用 Python 标准库（zlib / struct / hashlib / random / argparse / pathlib / json）。
    - 路径一律用 pathlib，不写死盘符；可在 Windows / macOS / Linux 上运行。
    - 不生成 emoji；不含任何真实证书、账号或网络请求。
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import random
import struct
import sys
import zlib
from pathlib import Path

# 固定的默认输出目录：<repo>/tests/fixtures（由本文件位置推导，与 cwd 无关）
REPO_ROOT = Path(__file__).resolve().parent.parent
DEFAULT_OUT = REPO_ROOT / "tests" / "fixtures"

# 固定种子，保证可重复生成
SEED = 20261116

# 页面标记：每页内容流里都放一个，acceptance.py 据此校验页数与顺序
PAGE_MARKER = "PAYONCEPDF_PAGE_{:04d}"

# 批量报告（PRD F7）约定的字段；acceptance.py 校验该 schema 与错误码枚举
# 与 src-tauri/src/commands/jobs.rs 的 batch_run 实现逐字对齐（勿改成 `file`）。
REPORT_FIELDS = ["filename", "elapsed_ms", "result", "error_code", "output"]
# 错误码全集（PRD 13 + CONTRACT 2）；report.csv 中出现的错误码必须是其子集
ERROR_CODES = [
    "E1001", "E1002", "E1003", "E1004", "E1005", "E1006", "E1007",
    "E2001", "E2002", "E2003", "E2004", "E2005",
]

# 本脚本拥有的产物（重跑前先清除，避免 --huge/--big 的残留文件干扰）
OWNED_FILES = [
    "page_1.pdf", "page_100.pdf", "page_500.pdf", "page_5000.pdf",
    "encrypted.placeholder.pdf",
    "scan_gray.pdf", "scan_color.pdf", "scan_big.pdf",
    "corrupted.pdf", "text_cjk_special.pdf",
    "report.csv", "manifest.json",
]
OWNED_IMAGES = ["page_01.png", "page_02.png", "page_03.png"]

# 20MB 大扫描件的目标像素数（4096 * 5120 = 20,971,520 bytes ~ 20MB，灰度 8bpc）
BIG_SCAN_W, BIG_SCAN_H = 4096, 5120

ENCRYPTED_PDF_CMD = (
    "qpdf --encrypt USERPASS OWNERPASS 256 -- page_1.pdf encrypted.pdf"
)

FONT_LIMIT_NOTE = (
    "No CJK font is embedded. Chinese glyphs are stored as raw UTF-8 bytes "
    "(as a PDF comment plus an octal-escaped literal string) and rely on the "
    "viewer's font fallback; they may render as boxes. To produce a real CJK PDF, "
    "run scripts/make-cjk-fixture.py (dev-only, needs fontTools + a system CJK font) "
    "which embeds a subset CID font and writes tests/fixtures/text_cjk_real.pdf. "
    "Embedding a CID font here is out of scope for this stdlib-only generator."
)


# ---------------------------------------------------------------------------
# 最小 PDF 写入器（PDF 1.4，结构正确、可解析）
# ---------------------------------------------------------------------------
class PdfDoc:
    """按需追加对象并在 finalize() 时生成 xref / trailer / startxref。"""

    def __init__(self) -> None:
        self._objects: list[bytes] = []

    def add(self, body: bytes) -> bytes:
        self._objects.append(body)
        return f"{len(self._objects)} 0 R".encode("ascii")

    def reserve(self) -> int:
        """先占一个对象号，随后用 set() 填内容（用于 Pages 节点需先拿到 ref 的情形）。"""
        self._objects.append(b"")
        return len(self._objects)

    def set(self, number: int, body: bytes) -> None:
        self._objects[number - 1] = body

    def finalize(self, root_ref: bytes) -> bytes:
        out = bytearray()
        out += b"%PDF-1.4\n%\xe2\xe3\xcf\xd3\n"  # 二进制注释头，表明文件含二进制数据
        offsets = [0]
        for i, body in enumerate(self._objects, start=1):
            offsets.append(len(out))
            out += f"{i} 0 obj\n".encode("ascii")
            out += body
            out += b"\nendobj\n"
        xref_offset = len(out)
        count = len(self._objects) + 1
        out += b"xref\n"
        out += f"0 {count}\n".encode("ascii")
        out += b"0000000000 65535 f \n"
        for off in offsets[1:]:
            out += f"{off:010d} 00000 n \n".encode("ascii")
        out += b"trailer\n"
        out += f"<< /Size {count} /Root {root_ref.decode('ascii')} >>\n".encode("ascii")
        out += b"startxref\n"
        out += f"{xref_offset}\n".encode("ascii")
        out += b"%%EOF\n"
        return bytes(out)


def _stream_obj(body: bytes, extra: bytes = b"") -> bytes:
    """生成带 /Length 的流对象。extra 为附加字典项（如 /Filter /FlateDecode）。"""
    head = b"<< /Length " + str(len(body)).encode("ascii") + b" " + extra + b">>"
    return head + b"\nstream\n" + body + b"\nendstream"


def _font_obj() -> bytes:
    return b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding >>"


def _page_obj(pages_ref: bytes, content_ref: bytes, font_ref: bytes,
              xobject_ref: bytes | None) -> bytes:
    if xobject_ref is not None:
        res = (b"<< /Font << /F1 " + font_ref + b" >> "
               b"/XObject << /Im0 " + xobject_ref + b" >> >>")
    else:
        res = b"<< /Font << /F1 " + font_ref + b" >> >>"
    return (b"<< /Type /Page /Parent " + pages_ref +
            b" /MediaBox [0 0 612 792] /Resources " + res +
            b" /Contents " + content_ref + b" >>")


def _pages_obj(kids: list[bytes]) -> bytes:
    return (b"<< /Type /Pages /Count " + str(len(kids)).encode("ascii") +
            b" /Kids [" + b" ".join(kids) + b"] >>")


def _catalog_obj(pages_ref: bytes) -> bytes:
    return b"<< /Type /Catalog /Pages " + pages_ref + b" >>"


def _text_content(page_no: int, total: int, extra_lines: list[str] | None = None) -> bytes:
    """未压缩内容流：文本页用 WinAnsi Helvetica，含可解析的页面标记。"""
    marker = PAGE_MARKER.format(page_no)
    lines = [
        b"BT",
        b"/F1 16 Tf",
        b"72 740 Td",
        b"(PayOnce PDF test fixture) Tj",
        b"0 -26 Td",
        (f"({marker} of {total:04d}) Tj").encode("ascii"),
        b"/F1 11 Tf",
        b"0 -24 Td",
        b"(Local PDF toolbox - merge / split / compress / OCR) Tj",
    ]
    if extra_lines:
        for line in extra_lines:
            safe = line.encode("latin-1", "replace")
            b0 = b"0 -18 Td (" + safe.replace(b"(", b"\\(").replace(b")", b"\\)") + b") Tj"
            lines.append(b0)
    lines.append(b"ET")
    return b"\n".join(lines) + b"\n"


def build_text_pdf(pages: int, extra_lines: list[str] | None = None) -> bytes:
    """构造 N 页纯文本 PDF（内容流不压缩，便于 stdlib 解析标记与顺序）。"""
    doc = PdfDoc()
    font_ref = doc.add(_font_obj())
    pages_number = doc.reserve()
    pages_ref = f"{pages_number} 0 R".encode("ascii")
    kids: list[bytes] = []
    for i in range(1, pages + 1):
        content_ref = doc.add(_stream_obj(_text_content(i, pages, extra_lines)))
        kids.append(doc.add(_page_obj(pages_ref, content_ref, font_ref, None)))
    doc.set(pages_number, _pages_obj(kids))
    root_ref = doc.add(_catalog_obj(pages_ref))
    return doc.finalize(root_ref)


# ---------------------------------------------------------------------------
# 图像生成（纯 stdlib）
# ---------------------------------------------------------------------------
def _scan_pixels(width: int, height: int, channels: int, seed: int) -> bytes:
    """生成一张"扫描表格"灰度/RGB 像素：浅色底 + 深色网格线 + 文字块 + 轻微噪点。"""
    rng = random.Random(seed)
    bpp = channels
    row_stride = width * bpp
    buf = bytearray(row_stride * height)
    # 浅色底 + 噪点
    for i in range(0, len(buf), bpp):
        v = 238 + rng.randint(0, 12)
        for c in range(bpp):
            buf[i + c] = min(255, v)
    # 表格网格线
    cols, rows = 8, 12
    for r in range(1, rows):
        y = height * r // rows
        for x in range(width):
            p = (y * width + x) * bpp
            for c in range(bpp):
                buf[p + c] = 40
    for c in range(1, cols):
        x = width * c // cols
        for y in range(height):
            p = (y * width + x) * bpp
            for ch in range(bpp):
                buf[p + ch] = 40
    # 表头深色条
    y0, y1 = 0, max(2, height // rows)
    for y in range(y0, y1):
        for x in range(width):
            p = (y * width + x) * bpp
            for ch in range(bpp):
                buf[p + ch] = 90
    # 文字块（模拟字符行）
    for r in range(1, rows):
        y = height * r // rows + 6
        for c in range(cols):
            x0 = width * c // cols + 6
            x1 = width * (c + 1) // cols - 6
            for yy in range(y, min(y + 6, height)):
                for xx in range(x0, min(x1, width)):
                    p = (yy * width + xx) * bpp
                    for ch in range(bpp):
                        buf[p + ch] = 70
    return bytes(buf)


def _image_xobject(width: int, height: int, channels: int, raw: bytes) -> bytes:
    """把原始像素经 zlib/FlateDecode 压缩，包成 PDF 图像 XObject。"""
    compressed = zlib.compress(raw, 9)
    cs = b"/DeviceGray" if channels == 1 else b"/DeviceRGB"
    extra = (b"/Type /XObject /Subtype /Image /Width " + str(width).encode("ascii") +
             b" /Height " + str(height).encode("ascii") + b" /ColorSpace " + cs +
             b" /BitsPerComponent 8 /Filter /FlateDecode")
    return _stream_obj(compressed, extra)


def _scan_content(page_no: int) -> bytes:
    """扫描页内容流：先画内嵌图像，再叠加矢量表格线条与文字层。"""
    ops = [
        b"q 612 0 0 792 0 0 cm /Im0 Do Q",
        b"0.6 w 0 0 0 RG",
    ]
    # 表格线条
    cols, rows = 8, 12
    for r in range(1, rows):
        y = 792 - 792 * r // rows
        ops.append(f"72 {y} m 540 {y} l S".encode("ascii"))
    for c in range(1, cols):
        x = 72 + 468 * c // cols
        ops.append(f"{x} 72 m {x} 720 l S".encode("ascii"))
    # 文字层（含页面标记，便于逐页校验）
    marker = PAGE_MARKER.format(page_no)
    ops += [
        b"BT /F1 10 Tf 80 700 Td",
        f"({marker} - scanned table page) Tj".encode("ascii"),
        b"ET",
    ]
    return b"\n".join(ops) + b"\n"


def build_scan_pdf(channels: int, seed: int) -> bytes:
    width, height = 300, 400
    raw = _scan_pixels(width, height, channels, seed)
    doc = PdfDoc()
    font_ref = doc.add(_font_obj())
    img_ref = doc.add(_image_xobject(width, height, channels, raw))
    pages_number = doc.reserve()
    pages_ref = f"{pages_number} 0 R".encode("ascii")
    content_ref = doc.add(_stream_obj(_scan_content(1)))
    kids = [doc.add(_page_obj(pages_ref, content_ref, font_ref, img_ref))]
    doc.set(pages_number, _pages_obj(kids))
    root_ref = doc.add(_catalog_obj(pages_ref))
    return doc.finalize(root_ref)


def build_big_scan_pdf() -> bytes:
    """20MB 级大扫描件：伪随机灰度像素近似不可压缩，FlateDecode 后文件约 20MB。"""
    raw = random.Random(SEED + 7).randbytes(BIG_SCAN_W * BIG_SCAN_H)
    compressed = zlib.compress(raw, 1)
    doc = PdfDoc()
    font_ref = doc.add(_font_obj())
    extra = (b"/Type /XObject /Subtype /Image /Width " + str(BIG_SCAN_W).encode("ascii") +
             b" /Height " + str(BIG_SCAN_H).encode("ascii") +
             b" /ColorSpace /DeviceGray /BitsPerComponent 8 /Filter /FlateDecode")
    img_ref = doc.add(_stream_obj(compressed, extra))
    pages_number = doc.reserve()
    pages_ref = f"{pages_number} 0 R".encode("ascii")
    content_ref = doc.add(_stream_obj(
        b"q 612 0 0 792 0 0 cm /Im0 Do Q\nBT /F1 10 Tf 72 720 Td (" +
        PAGE_MARKER.format(1).encode("ascii") + b" - 20MB scan) Tj ET\n"))
    kids = [doc.add(_page_obj(pages_ref, content_ref, font_ref, img_ref))]
    doc.set(pages_number, _pages_obj(kids))
    root_ref = doc.add(_catalog_obj(pages_ref))
    return doc.finalize(root_ref)


def png_bytes(width: int, height: int, channels: int, seed: int) -> bytes:
    """用纯标准库手写 PNG（IHDR / IDAT / IEND + CRC32 + zlib）。"""
    raw = _scan_pixels(width, height, channels, seed)
    bpp = channels
    scan = bytearray()
    for y in range(height):
        scan.append(0)  # filter type: None
        scan += raw[y * width * bpp:(y + 1) * width * bpp]
    comp = zlib.compress(bytes(scan), 9)
    color_type = 0 if channels == 1 else 2

    def chunk(tag: bytes, data: bytes) -> bytes:
        return (struct.pack(">I", len(data)) + tag + data +
                struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF))

    ihdr = struct.pack(">IIBBBBB", width, height, 8, color_type, 0, 0, 0)
    return (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", ihdr) +
            chunk(b"IDAT", comp) + chunk(b"IEND", b""))


def build_cjk_special_pdf() -> bytes:
    """中文 / 特殊符号样本。特殊符号用 WinAnsi 八进制转义（可正常渲染）；
    中文以 UTF-8 原始字节写入（注释 + 八进制转义串），不内嵌字体，详见 FONT_LIMIT_NOTE。"""
    doc = PdfDoc()
    font_ref = doc.add(_font_obj())
    pages_number = doc.reserve()
    pages_ref = f"{pages_number} 0 R".encode("ascii")
    # WinAnsi 特殊符号： (c) (r) +/- x / div section paragraph
    specials = b"(\\251 \\256 \\261 \\327 \\367 \\247 \\266 \\200) Tj"
    zh = "中文测试 一次PDF 特殊符号 © ® ± × ÷ § ¶ 契約書"
    zh_bytes = zh.encode("utf-8")
    zh_octal = "".join("\\%03o" % b for b in zh_bytes).encode("ascii")
    content = b"\n".join([
        b"BT /F1 14 Tf 72 740 Td",
        b"(PayOnce PDF - CJK and special symbols fixture) Tj",
        b"0 -26 Td",
        specials,
        b"0 -26 Td",
        b"(" + zh_octal + b") Tj",  # 无 CJK 字体，仅含原始字节
        b"ET",
        b"% CJK raw bytes (UTF-8) below; no CID font embedded:",
        b"% " + zh_bytes.replace(b"\n", b" "),
    ]) + b"\n"
    content_ref = doc.add(_stream_obj(content))
    kids = [doc.add(_page_obj(pages_ref, content_ref, font_ref, None))]
    doc.set(pages_number, _pages_obj(kids))
    root_ref = doc.add(_catalog_obj(pages_ref))
    return doc.finalize(root_ref)


# ---------------------------------------------------------------------------
# 样本写出
# ---------------------------------------------------------------------------
def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest()


def clean_owned(out: Path) -> None:
    """重跑前清除本脚本拥有的产物，保证结果可重复且不残留旧标志文件。"""
    for name in OWNED_FILES:
        p = out / name
        if p.is_file():
            p.unlink()
    img_dir = out / "images"
    for name in OWNED_IMAGES:
        p = img_dir / name
        if p.is_file():
            p.unlink()
    if img_dir.is_dir() and not any(img_dir.iterdir()):
        img_dir.rmdir()


def write_bytes(path: Path, data: bytes, files: list[dict], kind: str,
                pages: int | None, note: str = "") -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(data)
    files.append({
        "name": path.name,
        "kind": kind,
        "pages": pages,
        "bytes": len(data),
        "sha256": sha256(data),
        "note": note,
    })


def make_report_csv() -> bytes:
    """批量报告样本：覆盖成功与各类错误码，供 acceptance.py 校验字段与枚举。"""
    header = ",".join(REPORT_FIELDS) + "\n"
    rows = [
        ("page_1.pdf", 12, "ok", ""),
        ("page_100.pdf", 240, "ok", ""),
        ("corrupted.pdf", 3, "failed", "E1001"),
        ("encrypted.pdf", 4, "failed", "E1002"),
        ("wrongpass.pdf", 4, "failed", "E1003"),
        ("full_disk.pdf", 2, "failed", "E1004"),
        ("page_5000.pdf", 1, "failed", "E1005"),
        ("ocr_cn.pdf", 5, "failed", "E1006"),
        ("locked_out.pdf", 6, "failed", "E1007"),
        ("no_sidecar.pdf", 1, "failed", "E2001"),
        ("no_pdfium.pdf", 1, "failed", "E2002"),
        ("cancelled.pdf", 1, "cancelled", "E2003"),
        ("bad_args.pdf", 1, "failed", "E2004"),
        ("trial_expired.pdf", 1, "blocked", "E2005"),
    ]
    # 实现里还有第五列 output（成功为输出文件、失败为空）。
    lines = [
        ",".join(map(str, r + (f"out/{r[0]}" if r[2] == "ok" else "",)))
        for r in rows
    ]
    return (header + "\n".join(lines) + "\n").encode("utf-8")


def generate(out: Path, huge: bool, big: bool) -> dict:
    clean_owned(out)
    out.mkdir(parents=True, exist_ok=True)
    files: list[dict] = []

    # 基础多页 PDF
    for n in (1, 100, 500):
        write_bytes(out / f"page_{n}.pdf", build_text_pdf(n), files,
                    "text-pdf", n, "每页含 PAYONCEPDF_PAGE_NNNN 标记，用于页数/顺序断言")
    if huge:
        write_bytes(out / "page_5000.pdf", build_text_pdf(5000), files,
                    "text-pdf", 5000, "--huge 生成；PRD 14.1 5000 页样本")

    # 加密占位（非真加密）
    enc_note = (
        "PLACEHOLDER - this file is NOT actually encrypted. "
        "Generate a real encrypted PDF with qpdf, see manifest.json ENCRYPTED_PDF_CMD."
    )
    # 用 extra_lines 生成内容流，保证 /Length 正确（不可对已生成 PDF 做字节替换）
    write_bytes(out / "encrypted.placeholder.pdf",
                build_text_pdf(1, extra_lines=[enc_note]),
                files, "encrypted-placeholder", 1,
                "占位；非真加密；已知密码样本须人工用 qpdf 生成")

    # 扫描件风格（灰度 / 彩色）
    write_bytes(out / "scan_gray.pdf", build_scan_pdf(1, SEED + 1), files,
                "scan-pdf", 1, "FlateDecode 灰度图像 XObject + 矢量表格线 + 文字层")
    write_bytes(out / "scan_color.pdf", build_scan_pdf(3, SEED + 2), files,
                "scan-pdf", 1, "FlateDecode 彩色(DeviceRGB)图像 XObject + 表格线 + 文字层")
    if big:
        write_bytes(out / "scan_big.pdf", build_big_scan_pdf(), files,
                    "scan-pdf-big", 1, "--big 生成；约 20MB 单页大扫描件")

    # 损坏文件（截断）
    good = build_text_pdf(1)
    write_bytes(out / "corrupted.pdf", good[: max(1, len(good) // 2)], files,
                "corrupted", None, "对合法 PDF 截断约 50%，用于 E1001 文件损坏断言")

    # 中文 / 特殊符号
    write_bytes(out / "text_cjk_special.pdf", build_cjk_special_pdf(), files,
                "cjk-special-pdf", 1, FONT_LIMIT_NOTE)

    # PNG 图片序列（JPG 无 stdlib 编码器，跳过）
    for idx, channels in enumerate((1, 2, 3), start=1):
        # PNG color_type 仅支持 0(gray)/2(rgb)，这里用 1 或 3 通道
        ch = 1 if channels == 1 else 3
        write_bytes(out / "images" / f"page_{idx:02d}.png",
                    png_bytes(240, 320, ch, SEED + 10 + idx), files,
                    "png-image", None, "纯 stdlib 生成；用于 F4 图片->PDF 页序测试")

    # 批量报告样本
    write_bytes(out / "report.csv", make_report_csv(), files,
                "report-csv", None, "字段: " + ",".join(REPORT_FIELDS))

    manifest = {
        "generator": "scripts/generate-fixtures.py",
        "version": 1,
        "seed": SEED,
        "stdlib_only": True,
        "page_marker": "PAYONCEPDF_PAGE_NNNN",
        "report_csv_fields": REPORT_FIELDS,
        "error_codes": ERROR_CODES,
        "notes": {
            "ENCRYPTED_PDF_CMD": ENCRYPTED_PDF_CMD,
            "encrypted_pdf": (
                "encrypted.placeholder.pdf 不是真加密文件。stdlib 不实现 PDF 标准加密；"
                "请用上面的 qpdf 命令人工生成已知密码的加密 PDF（PRD 14.1）。"
            ),
            "jpg": (
                "跳过 JPG 样本：Python 标准库没有 JPEG 编码器。"
                "图片->PDF 的 JPG 分支须用真实应用/真机测试（SKIP manual）。"
            ),
            "font_limit": FONT_LIMIT_NOTE,
            "big_scan": (
                "scan_big.pdf 由伪随机灰度像素经 FlateDecode 近似不可压缩生成，约 20MB；"
                "把 BIG_SCAN_W/BIG_SCAN_H 调大即可得到 PRD 14.1 的 50MB 单页样本。"
            ),
            "third_party": "本脚本不依赖 reportlab / Pillow / qpdf；qpdf 仅由 acceptance.py 在可用时调用。",
        },
        "files": files,
    }
    (out / "manifest.json").write_text(
        json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    return manifest


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="generate-fixtures.py",
        description="PayOnce PDF 测试样本生成器（纯 Python 标准库）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "示例:\n"
            "  python scripts/generate-fixtures.py\n"
            "  python scripts/generate-fixtures.py --huge --big\n"
            "  python scripts/generate-fixtures.py --out /tmp/payoncepdf-fixtures\n"
        ),
    )
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT,
                        help=f"输出目录（默认 {DEFAULT_OUT}）")
    parser.add_argument("--huge", action="store_true", help="额外生成 5000 页样本")
    parser.add_argument("--big", action="store_true", help="额外生成 20MB 级大扫描件")
    args = parser.parse_args(argv)

    out = args.out.resolve()
    manifest = generate(out, args.huge, args.big)
    total = sum(f["bytes"] for f in manifest["files"])
    print(f"[generate-fixtures] out={out}")
    print(f"[generate-fixtures] files={len(manifest['files'])} total_bytes={total}")
    for f in manifest["files"]:
        pg = f["pages"] if f["pages"] is not None else "-"
        print(f"  - {f['name']:<32} kind={f['kind']:<22} pages={pg:<5} bytes={f['bytes']}")
    print("[generate-fixtures] manifest.json 已写出。")
    print("[generate-fixtures] 注意: 加密 PDF 为占位样本，真实加密件用 qpdf 人工生成（见 manifest.json）。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
