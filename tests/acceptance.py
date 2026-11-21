#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
acceptance.py - PayOnce PDF 验收测试脚手架（纯 Python 标准库，无第三方依赖）

用途
    按 PRD 第 14 节（测试计划）与 CONTRACT.md 第 8 节（验收映射）逐项断言，
    产出 Markdown 表格报告 tests/out/acceptance-report.md。
    关键约束：在没有应用安装包（没有 Tauri app、没有真机、没有证书/公证）时，
    仍能通过 --smoke 运行；需要真实 app / 真机 / 双平台 / 证书 / 公证的项目
    明确输出 "SKIP (manual)"，绝不伪装成 PASS。

用法
    python scripts/generate-fixtures.py            # 先生成样本
    python tests/acceptance.py --smoke             # CI / 本地冒烟
    python tests/acceptance.py                     # 同上（manual 项仍为 SKIP）
    python tests/acceptance.py --out DIR           # 指定报告目录（默认 <repo>/tests/out）
    python tests/acceptance.py --help

检查项（--smoke 自动执行）
    A1  fixtures 页数 / 顺序              解析每页 PAYONCEPDF_PAGE_NNNN 标记并与 manifest.json 比对
    A2  qpdf 合并                        qpdf 可用时真实合并，断言合并后页数 = 输入页数之和
    A3  拆分份数推导                     500 页 / 每 10 页 = 50 个文件（从 fixtures 页数推导）
    A4  qpdf 真实拆分                   qpdf 可用时真实 --split-pages=10，断言产出 50 个文件
    A5  report.csv 字段                 校验批量报告样本字段 schema
    A6  错误码枚举完整性               解析 CONTRACT.md / PRD.md，断言 E1001..E1007 与 E2001..E2005 齐全
    A7  docs/site 交付物存在性         site/verify.html、docs/error-codes.md、docs/test-report.md

    A2 / A4 依赖 qpdf：未检测到时标记 SKIP (manual)，不阻塞 smoke。
    A7 属于其它工作流的交付物：文件缺失时标记 SKIP (manual) 并在备注写明，不阻塞 smoke；
       文件存在则 PASS。

    其余 PRD 14.2 项（压缩质量、OCR 准确率、大文件稳定性、离线可用、无上传、
    失败恢复、双平台真机、签名、公证、Gatekeeper、universal binary、updater 回滚、
    冷启动 / 内存 / 安装包体积 / 无障碍）一律 SKIP (manual)，
    并在备注指向 tests/license_test.md 或说明所需条件。

退出码
    0 = smoke 全通过（没有 FAIL）
    1 = 至少一项 auto 检查 FAIL
    2 = 环境错误（fixtures / manifest / CONTRACT.md 缺失等）

工程红线
    - 仅用 Python 标准库（re / csv / json / shutil / subprocess / platform / pathlib / datetime）。
    - 路径一律用 pathlib，不写死盘符；可在 Windows / macOS / Linux 上运行。
    - 不生成 emoji；不联网；不调用任何需要真实证书或账号的操作。
"""

from __future__ import annotations

import argparse
import csv
import datetime
import json
import platform
import re
import shutil
import subprocess
import sys
import tempfile
import zlib
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
FIXTURES = REPO_ROOT / "tests" / "fixtures"
DEFAULT_OUT = REPO_ROOT / "tests" / "out"

# PRD 13 + CONTRACT 2 的错误码全集
EXPECTED_CODES = {
    "E1001", "E1002", "E1003", "E1004", "E1005", "E1006", "E1007",
    "E2001", "E2002", "E2003", "E2004", "E2005",
}
# PRD F7 批量报告字段
# 与 src-tauri/src/commands/jobs.rs::batch_run 实际写出的表头一致。
REPORT_FIELDS = ["filename", "elapsed_ms", "result", "error_code", "output"]
# 其它工作流交付的文档（exists -> PASS，缺失 -> SKIP (manual)）
EXTERNAL_DOCS = [
    "site/verify.html",
    "docs/error-codes.md",
    "docs/test-report.md",
]

STATUS_PASS = "PASS"
STATUS_FAIL = "FAIL"
STATUS_SKIP = "SKIP (manual)"

# results 中每项: {id, item, category, status, note}
results: list[dict] = []


def record(cid: str, item: str, category: str, status: str, note: str = "") -> None:
    results.append({
        "id": cid, "item": item, "category": category,
        "status": status, "note": note,
    })
    print(f"[{status:<14}] {cid} {item}" + (f" -- {note}" if note else ""))


# 流对象起始：字典结束 >> 后紧跟 stream 关键字。用 \b 语义的 >> 限定，
# 避免匹配到 endstream 中的 "stream" 子串。
_STREAM_START = re.compile(rb">>\s*stream(?:\r\n|\n|\r)")
_MARKER_RE = re.compile(rb"PAYONCEPDF_PAGE_(\d{4})")


def _stream_blobs(data: bytes) -> list[bytes]:
    """按文件顺序取出所有流的内容；/Filter /FlateDecode 的流用 zlib 解压。

    qpdf 合并/拆分输出默认会把内容流重新压成 FlateDecode，故必须解压后才能
    看到页面标记。/Length 为直接数字；非线性化、无对象流的 PDF 均适用。
    """
    blobs = []
    for m in _STREAM_START.finditer(data):
        start = m.end()
        dict_start = data.rfind(b"<<", max(0, m.start() - 8192), m.start())
        if dict_start < 0:
            continue
        dict_text = data[dict_start:m.start()]
        lm = re.search(rb"/Length\s+(\d+)", dict_text)
        if not lm:
            continue
        length = int(lm.group(1))
        raw = data[start:start + length]
        if not raw:
            continue
        if b"/FlateDecode" in dict_text:
            try:
                raw = zlib.decompress(raw)
            except zlib.error:
                continue  # 非 Flate 数据或损坏流，跳过
        blobs.append(raw)
    return blobs


def page_markers(pdf: Path) -> list[int]:
    """按出现顺序提取 PAYONCEPDF_PAGE_NNNN 标记（兼容未压缩与 FlateDecode 内容流）。"""
    data = pdf.read_bytes()
    markers: list[int] = []
    for blob in _stream_blobs(data):
        markers.extend(int(x) for x in _MARKER_RE.findall(blob))
    if not markers:  # 兜底：极简 PDF 直接在原始字节里扫
        markers = [int(x) for x in _MARKER_RE.findall(data)]
    return markers


def current_triple() -> str:
    """推导当前平台的 Rust target triple，用于定位 externalBin sidecar。"""
    system = platform.system()
    machine = platform.machine().lower()
    if system == "Windows":
        return "x86_64-pc-windows-msvc"
    if system == "Darwin":
        if machine in ("arm64", "aarch64"):
            return "aarch64-apple-darwin"
        return "x86_64-apple-darwin"
    return f"{machine}-unknown-linux-gnu"


def find_qpdf() -> str | None:
    """优先 PATH，其次 src-tauri/bin/ 下与当前 triple 匹配的 externalBin。"""
    found = shutil.which("qpdf")
    if found:
        return found
    bindir = REPO_ROOT / "src-tauri" / "bin"
    if not bindir.is_dir():
        return None
    triple = current_triple()
    candidates = [
        bindir / f"qpdf-{triple}",
        bindir / f"qpdf-{triple}.exe",
    ]
    for c in candidates:
        if c.is_file():
            return str(c)
    # 兜底：任意 qpdf-* （排除 Windows .exe 在非 Windows 平台上）
    for c in sorted(bindir.glob("qpdf-*")):
        if not c.is_file():
            continue
        if platform.system() != "Windows" and c.suffix.lower() == ".exe":
            continue
        return str(c)
    return None


def run_qpdf(args: list[str]) -> tuple[bool, str]:
    """执行 qpdf，返回 (是否成功, 输出摘要)。"""
    try:
        proc = subprocess.run(args, capture_output=True, timeout=180)
    except (OSError, subprocess.TimeoutExpired) as exc:
        return False, f"qpdf 调用失败: {exc}"
    if proc.returncode != 0:
        err = (proc.stderr or proc.stdout).decode("utf-8", "replace").strip()
        return False, f"qpdf 退出码 {proc.returncode}: {err[:300]}"
    return True, "ok"


# ---------------------------------------------------------------------------
# 自动检查
# ---------------------------------------------------------------------------
def check_fixture_pages(manifest: dict) -> None:
    """A1: fixtures 页数 / 顺序。"""
    pages_by_name = {f["name"]: f["pages"] for f in manifest["files"]}
    targets = ["page_1.pdf", "page_100.pdf", "page_500.pdf"]
    if "page_5000.pdf" in pages_by_name:
        targets.append("page_5000.pdf")
    problems = []
    for name in targets:
        path = FIXTURES / name
        if not path.is_file():
            problems.append(f"{name} 缺失")
            continue
        markers = page_markers(path)
        expected = pages_by_name.get(name)
        if expected is None:
            problems.append(f"{name} manifest 无 pages 字段")
            continue
        if len(markers) != expected:
            problems.append(f"{name} 标记数 {len(markers)} != manifest pages {expected}")
        if markers != list(range(1, expected + 1)):
            problems.append(f"{name} 页序不连续/乱序")
    if problems:
        record("A1", "fixtures 页数 / 顺序 (PRD 14.1)", "auto", STATUS_FAIL,
               "; ".join(problems))
    else:
        record("A1", "fixtures 页数 / 顺序 (PRD 14.1)", "auto", STATUS_PASS,
               f"校验 {len(targets)} 个多页样本，标记数连续且与 manifest.json 一致")


def check_qpdf_merge(qpdf: str | None, work_dir: Path) -> None:
    """A2: qpdf 合并后页数 = 输入页数之和。"""
    if qpdf is None:
        record("A2", "qpdf 合并页数 = 输入之和 (PRD F1)", "qpdf", STATUS_SKIP,
               "未检出 qpdf（PATH 与 src-tauri/bin 均无）；需先运行 provision-sidecars.mjs 或安装 qpdf")
        return
    a, b = FIXTURES / "page_100.pdf", FIXTURES / "page_1.pdf"
    # 每次用全新临时目录，避免删除既有文件（tests/out 为 gitignore 的产物目录）
    work_dir.mkdir(parents=True, exist_ok=True)
    run_dir = Path(tempfile.mkdtemp(prefix="merge-", dir=work_dir))
    merged = run_dir / "merged.pdf"
    ok, note = run_qpdf([qpdf, "--empty", "--pages", str(a), str(b), "--", str(merged)])
    if not ok:
        record("A2", "qpdf 合并页数 = 输入之和 (PRD F1)", "qpdf", STATUS_FAIL, note)
        return
    if not merged.is_file():
        record("A2", "qpdf 合并页数 = 输入之和 (PRD F1)", "qpdf", STATUS_FAIL,
               "qpdf 返回成功但未生成 merged.pdf")
        return
    got = len(page_markers(merged))
    expected = len(page_markers(a)) + len(page_markers(b))
    if got != expected:
        record("A2", "qpdf 合并页数 = 输入之和 (PRD F1)", "qpdf", STATUS_FAIL,
               f"合并后 {got} 页 != 输入之和 {expected} 页")
    else:
        record("A2", "qpdf 合并页数 = 输入之和 (PRD F1)", "qpdf", STATUS_PASS,
               f"qpdf 真实合并: {got} = {len(page_markers(a))} + {len(page_markers(b))}")


def check_split_derivation(manifest: dict) -> None:
    """A3: 从 fixtures 页数推导 500 页 / 每 10 页 = 50 个文件。"""
    path = FIXTURES / "page_500.pdf"
    if not path.is_file():
        record("A3", "拆分份数推导 500 页 / 每 10 页 -> 50 文件 (PRD F2)", "auto",
               STATUS_FAIL, "page_500.pdf 缺失，请先运行 generate-fixtures.py")
        return
    pages = len(page_markers(path))
    expected_parts = pages // 10
    if pages != 500 or expected_parts != 50:
        record("A3", "拆分份数推导 500 页 / 每 10 页 -> 50 文件 (PRD F2)", "auto",
               STATUS_FAIL, f"pages={pages}, 推导份数={expected_parts}（期望 500 / 50）")
    else:
        record("A3", "拆分份数推导 500 页 / 每 10 页 -> 50 文件 (PRD F2)", "auto",
               STATUS_PASS, "500 页 / 每 10 页 = 50 个文件（份数由样本页数推导；真实文件拆分见 A4）")


def check_qpdf_split(qpdf: str | None, work_dir: Path) -> None:
    """A4: qpdf 真实拆分产出 50 个文件。"""
    if qpdf is None:
        record("A4", "qpdf 真实拆分 50 个文件 (PRD F2)", "qpdf", STATUS_SKIP,
               "未检出 qpdf；真实拆分需 qpdf sidecar / 系统 qpdf")
        return
    src = FIXTURES / "page_500.pdf"
    # 每次用全新临时目录，不做任何删除操作，避免破坏既有产物
    work_dir.mkdir(parents=True, exist_ok=True)
    split_dir = Path(tempfile.mkdtemp(prefix="split-", dir=work_dir))
    out_pattern = split_dir / "part-%d.pdf"
    ok, note = run_qpdf([qpdf, "--split-pages=10", str(src), str(out_pattern)])
    if not ok:
        record("A4", "qpdf 真实拆分 50 个文件 (PRD F2)", "qpdf", STATUS_FAIL, note)
        return
    parts = sorted(split_dir.glob("part-*.pdf"))
    total_pages = sum(len(page_markers(p)) for p in parts)
    if len(parts) != 50 or total_pages != 500:
        record("A4", "qpdf 真实拆分 50 个文件 (PRD F2)", "qpdf", STATUS_FAIL,
               f"产出 {len(parts)} 个文件 / 合计 {total_pages} 页（期望 50 / 500）")
    else:
        record("A4", "qpdf 真实拆分 50 个文件 (PRD F2)", "qpdf", STATUS_PASS,
               f"qpdf --split-pages=10 产出 {len(parts)} 个文件，合计 {total_pages} 页")


def check_report_csv() -> None:
    """A5: report.csv 字段 schema（PRD F7）。"""
    path = FIXTURES / "report.csv"
    if not path.is_file():
        record("A5", "report.csv 字段完整性 (PRD F7 / CONTRACT 8)", "auto",
               STATUS_FAIL, "report.csv 缺失，请先运行 generate-fixtures.py")
        return
    with path.open("r", encoding="utf-8", newline="") as fh:
        reader = csv.reader(fh)
        header = next(reader, [])
        rows = list(reader)
    missing = [f for f in REPORT_FIELDS if f not in header]
    if missing:
        record("A5", "report.csv 字段完整性 (PRD F7 / CONTRACT 8)", "auto",
               STATUS_FAIL, f"缺少字段 {missing}; 实际表头 {header}")
    else:
        record("A5", "report.csv 字段完整性 (PRD F7 / CONTRACT 8)", "auto",
               STATUS_PASS, f"表头 {header} 含全部必需字段；{len(rows)} 行样本")


def check_error_codes(manifest: dict) -> None:
    """A6: 错误码枚举完整性（CONTRACT 2 / PRD 13）。"""
    problems = []
    contract = REPO_ROOT / "CONTRACT.md"
    prd = REPO_ROOT.parent / "docs" / "pdf-launch" / "PRD.md"
    if not contract.is_file():
        problems.append("CONTRACT.md 缺失")
        found: set[str] = set()
    else:
        found = set(re.findall(r"E\d{4}", contract.read_text(encoding="utf-8", errors="replace")))
    missing = sorted(EXPECTED_CODES - found)
    if missing:
        problems.append(f"CONTRACT.md 缺少错误码 {missing}")
    if prd.is_file():
        prd_codes = set(re.findall(r"E\d{4}", prd.read_text(encoding="utf-8", errors="replace")))
        e1 = {c for c in EXPECTED_CODES if c.startswith("E1")}
        if not e1.issubset(prd_codes):
            problems.append(f"PRD 13 缺少 E1xxx 错误码 {sorted(e1 - prd_codes)}")
    # manifest 中的枚举
    manifest_codes = set(manifest.get("error_codes", []))
    if manifest_codes != EXPECTED_CODES:
        problems.append("manifest.json error_codes 与预期全集不一致")
    # report.csv 中引用的错误码必须是全集子集
    rc = FIXTURES / "report.csv"
    if rc.is_file():
        with rc.open("r", encoding="utf-8", newline="") as fh:
            reader = csv.DictReader(fh)
            used = {r.get("error_code", "").strip() for r in reader}
        used.discard("")
        unknown = sorted(used - EXPECTED_CODES)
        if unknown:
            problems.append(f"report.csv 含未知错误码 {unknown}")
    if problems:
        record("A6", "错误码枚举完整性 (PRD 13 / CONTRACT 2)", "auto",
               STATUS_FAIL, "; ".join(problems))
    else:
        record("A6", "错误码枚举完整性 (PRD 13 / CONTRACT 2)", "auto",
               STATUS_PASS, f"E1001..E1007 + E2001..E2005 共 {len(EXPECTED_CODES)} 个码齐全，report.csv 引用合法")


def check_external_docs() -> None:
    """A7: docs/site 交付物存在性（其它工作流交付，缺失不阻塞 smoke）。"""
    for idx, rel in enumerate(EXTERNAL_DOCS):
        cid = "A7" + chr(ord("a") + idx)
        path = REPO_ROOT / rel
        if path.is_file():
            record(cid, f"{rel} 存在性", "external", STATUS_PASS, "文件已就位")
        else:
            record(cid, f"{rel} 存在性", "external", STATUS_SKIP,
                   "由 docs/site 工作流交付；当前未就位，smoke 不阻塞（全量验收时须补上）")


# ---------------------------------------------------------------------------
# PRD 14.2 其余项：需要真实 app / 真机 / 双平台 / 证书 / 公证 -> SKIP (manual)
# ---------------------------------------------------------------------------
MANUAL_ITEMS = [
    ("M1", "合并 100 文件 ≤60s、页数之和 (F1/14.2)", "需真实 app 与 100 个真机文件；smoke 仅覆盖页数逻辑 (A1/A2)"),
    ("M2", "压缩 20MB -> ≤5MB、文字层仍可选 (F3/14.2)", "需真实 app 的压缩引擎与前后对比图"),
    ("M3", "300DPI 10 页 ≤30s、页序正确 (F4/14.2)", "需真实 app + PDFium 渲染"),
    ("M4", "OCR 10 页 ≤60s、英≥95% 中≥90% (F5/14.2)", "需真实 app + tesseract sidecar + 标准测试集"),
    ("M5", "批量 200 文件、report.csv 错误码不中断队列 (F7/14.2)", "需真实 app；smoke 仅校验样本 schema (A5/A6)"),
    ("M6", "大文件 500MB 不崩溃 / 失败恢复 原文件不被修改 (14.2)", "需真实 app 与 500MB 样本"),
    ("M7", "离线可用（断网 15 工具全功能，除激活与用户主动装语言包）(F8/14.2)", "需真实 app / 真机断网；三类可枚举请求见 site/verify.html；步骤见 tests/license_test.md"),
    ("M8", "无上传：抓包无文件外发 (6/14.2)", "需真实 app + 抓包工具 + site/verify.html"),
    ("M9", "试用到期只限导出、不删除用户文件 (F8/14.2)", "需真实 app；错误码 E2005；步骤见 tests/license_test.md"),
    ("M10", "双平台真机安装包无警告 (M7/8/12)", "需 Windows EV/Azure 与 macOS Developer ID 真实证书"),
    ("M11", "macOS codesign + notarize + staple + Gatekeeper (12)", "需 Apple Developer ID、Apple ID、公证服务"),
    ("M12", "macOS universal binary / dmg ≤80MB (8/12)", "需 arm64 + x64 两份产物与 lipo 合成"),
    ("M13", "安装包体积 / 冷启动 ≤3s / 内存 / 无障碍 (8)", "需真机安装包与测量工具"),
    ("M14", "Tauri Updater 升级成功、失败可回滚 (F9)", "需真实 app + 更新端点 + 签名密钥"),
]


def check_manual_items() -> None:
    for cid, item, note in MANUAL_ITEMS:
        record(cid, item, "manual", STATUS_SKIP, note)


# ---------------------------------------------------------------------------
# 报告
# ---------------------------------------------------------------------------
def write_report(out_dir: Path, mode: str, qpdf: str | None) -> Path:
    out_dir.mkdir(parents=True, exist_ok=True)
    report_path = out_dir / "acceptance-report.md"
    passed = sum(1 for r in results if r["status"] == STATUS_PASS)
    failed = sum(1 for r in results if r["status"] == STATUS_FAIL)
    skipped = sum(1 for r in results if r["status"] == STATUS_SKIP)
    now = datetime.datetime.now().strftime("%Y-%m-%d %H:%M:%S")

    lines = [
        "# PayOnce PDF 验收报告（PRD 14 / CONTRACT 8）",
        "",
        f"- 模式: `{mode}`",
        f"- 生成时间: {now}",
        f"- fixtures: `{FIXTURES}`",
        f"- qpdf: `{qpdf if qpdf else '未检出'}`",
        f"- platform: `{platform.system()} {platform.machine()}` / target `{current_triple()}`",
        f"- 汇总: **PASS {passed} / FAIL {failed} / SKIP (manual) {skipped}**",
        "",
        "| 编号 | 验收项 | 类别 | 结果 | 备注 |",
        "|---|---|---|---|---|",
    ]
    for r in results:
        note = r["note"].replace("|", "\\|")
        lines.append(f"| {r['id']} | {r['item']} | {r['category']} | "
                     f"{r['status']} | {note} |")
    lines += [
        "",
        "## 说明",
        "",
        "- `SKIP (manual)` = 需要真实 app / 真机 / 双平台 / 证书 / 公证，smoke 阶段**不判定为 PASS**。",
        "- A2 / A4 依赖 qpdf：检测到 qpdf（PATH 或 `src-tauri/bin/qpdf-<triple>[.exe]`）时执行真实调用，否则 SKIP。",
        "- A3 的 50 个文件为份数推导；A4 才是产出真实文件的断言。",
        "- A7 的 docs/site 三项由其它工作流交付，缺失时标记 SKIP (manual)，不阻塞 smoke；全量验收须补齐。",
        "- 授权/试用手动步骤见 `tests/license_test.md`（7 天试用、断网可用、首次激活、离线缓存、到期只限导出、更新可关闭）。",
        "- 退出码: 0 = smoke 全通过（无 FAIL）；1 = 有 auto 项 FAIL；2 = 环境错误。",
        "",
    ]
    report_path.write_text("\n".join(lines) + "\n", encoding="utf-8")
    return report_path


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(
        prog="acceptance.py",
        description="PayOnce PDF 验收测试脚手架（纯 Python 标准库）",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog=(
            "示例:\n"
            "  python scripts/generate-fixtures.py\n"
            "  python tests/acceptance.py --smoke\n"
        ),
    )
    parser.add_argument("--smoke", action="store_true",
                        help="冒烟模式：无应用安装包也可运行（manual 项输出 SKIP）")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT,
                        help=f"报告输出目录（默认 {DEFAULT_OUT}）")
    args = parser.parse_args(argv)
    mode = "smoke" if args.smoke else "full"
    out_dir = args.out.resolve()

    # 环境检查
    manifest_path = FIXTURES / "manifest.json"
    if not manifest_path.is_file():
        print("[ERROR] 缺少 tests/fixtures/manifest.json，请先运行 "
              "python scripts/generate-fixtures.py", file=sys.stderr)
        return 2
    if not (REPO_ROOT / "CONTRACT.md").is_file():
        print("[ERROR] 缺少 CONTRACT.md", file=sys.stderr)
        return 2
    try:
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    except json.JSONDecodeError as exc:
        print(f"[ERROR] manifest.json 解析失败: {exc}", file=sys.stderr)
        return 2

    qpdf = find_qpdf()
    print(f"[acceptance] mode={mode} qpdf={qpdf or '未检出'} out={out_dir}")

    # 自动检查
    check_fixture_pages(manifest)
    check_qpdf_merge(qpdf, out_dir)
    check_split_derivation(manifest)
    check_qpdf_split(qpdf, out_dir)
    check_report_csv()
    check_error_codes(manifest)
    check_external_docs()
    # 手动项
    check_manual_items()

    report_path = write_report(out_dir, mode, qpdf)
    failed = [r for r in results if r["status"] == STATUS_FAIL]
    print(f"[acceptance] 报告: {report_path}")
    if failed:
        print(f"[acceptance] {len(failed)} 项 FAIL:", ", ".join(r["id"] for r in failed))
        return 1
    print("[acceptance] smoke 全通过（无 FAIL）。")
    return 0


if __name__ == "__main__":
    sys.exit(main())
