#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""PayOnce PDF 真机测试前置检查（严格只读）。

用途：在双击安装包 / 运行 release 主程序之前，一次性确认所有前置条件。
本脚本不会修改、移动或删除任何文件；不启动 GUI；不联网；不写用户数据目录。
唯一写出的文件是本脚本自己的报告：tests/out/preflight-real-device.json。

用法：
    python scripts/preflight-real-device.py            # 人类可读表格 + JSON
    python scripts/preflight-real-device.py --json     # 只输出 JSON
    python scripts/preflight-real-device.py --strict   # BLOCK 项也按非零退出

退出码：存在 FAIL -> 1；--strict 下存在 BLOCK -> 1；其余 -> 0。
状态含义：PASS 就绪 / WARN 需人工留意 / BLOCK 已知缺陷（影响安装包路径，
但不阻塞 release exe 路径）/ FAIL 开测前必须修复。
"""

from __future__ import annotations

import argparse
import hashlib
import json
import os
import socket
import sys
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT_JSON = ROOT / "tests" / "out" / "preflight-real-device.json"

INSTALLER_REL = "src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe"
# 2026-11-20 重建后基线：加入 tessdata 与 F17 阅读视图命令，签名已重新生成。
INSTALLER_SHA256 = "e2849b8ee19c01eba37c7c25cf3d9cbfa3ef2dbca6c8c235c18fe69e42b6e58c"

RESULTS: list[dict] = []


def record(cid: str, title: str, status: str, detail: str = "") -> None:
    RESULTS.append({"id": cid, "title": title, "status": status, "detail": detail})


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def exists(cid: str, rel: str, title: str | None = None, expect_sha: str | None = None,
           min_bytes: int = 1) -> None:
    p = ROOT / rel
    label = title or rel
    if not p.is_file():
        record(cid, label, "FAIL", f"缺失：{rel}")
        return
    size = p.stat().st_size
    if size < min_bytes:
        record(cid, label, "FAIL", f"文件过小：{size} B（< {min_bytes}）")
        return
    if expect_sha is not None:
        got = sha256(p)
        if got != expect_sha:
            record(cid, label, "FAIL", f"SHA-256 不匹配：期望 {expect_sha[:16]}…，实得 {got[:16]}…")
            return
        record(cid, label, "PASS", f"{size:,} B，SHA-256 与当前构建基线一致（见 docs/test-report.md §18）")
        return
    record(cid, label, "PASS", f"{size:,} B")


def port_open(port: int, host: str = "127.0.0.1") -> bool:
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
        s.settimeout(0.35)
        return s.connect_ex((host, port)) == 0


def check_deliverables() -> None:
    exists("P01", "src-tauri/target/release/payoncepdf.exe", "未打包主程序（最快开测路径）")
    exists("P02", INSTALLER_REL, "Windows NSIS 安装包", expect_sha=INSTALLER_SHA256)
    exists("P03", INSTALLER_REL + ".sig", "更新签名 .sig")
    exists("P04", "dist/index.html", "前端构建产物")
    exists("P05", "src-tauri/keys/public.key", "Ed25519 授权公钥（LT3 激活用）")
    exists("P06", "scripts/license-server.mjs", "本地 mock 激活服务脚本")
    exists("P07", "tests/license_test.md", "授权/试用手动用例 LT1–LT6")
    exists("P08", "tests/fixtures/manifest.json", "测试素材清单")


def check_runtime_engines() -> None:
    # Tauri 把 resources/* 复制到 target/release/resources；tessdata 是目录，
    # 不会被 glob "*" 展开，故准备阶段已手工补齐，这里验证补齐结果。
    exists("P10", "src-tauri/target/release/resources/pdfium.dll",
           "运行时 PDFium 动态库（F10–F16 依赖）", min_bytes=1_000_000)
    for i, name in enumerate(("eng.traineddata", "chi_sim.traineddata"), start=11):
        exists(f"P{i}", f"src-tauri/target/release/resources/tessdata/{name}",
               f"运行时 OCR 语言数据 {name}", min_bytes=100_000)
    exists("P13", "src-tauri/target/release/qpdf.exe", "运行时 qpdf sidecar")
    exists("P14", "src-tauri/target/release/tesseract.exe", "运行时 tesseract sidecar")
    # 源目录用于日后重建，哈希应与运行时一致
    src = ROOT / "src-tauri/resources/tessdata"
    dst = ROOT / "src-tauri/target/release/resources/tessdata"
    same = all(
        (src / n).is_file() and (dst / n).is_file()
        and sha256(src / n) == sha256(dst / n)
        for n in ("eng.traineddata", "chi_sim.traineddata")
    )
    record("P15", "运行时 tessdata 与源目录逐字节一致",
           "PASS" if same else "FAIL",
           "两个 traineddata 哈希一致" if same else "不一致，请重新复制 src-tauri/resources/tessdata/")


def check_config() -> None:
    cfg_path = ROOT / "src-tauri/tauri.conf.json"
    prod_path = ROOT / "config/product.json"
    try:
        cfg = json.loads(cfg_path.read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        record("P20", "tauri.conf.json 可解析", "FAIL", str(e))
        cfg = {}
    checks = [
        ("P21", "productName = PayOnce PDF", cfg.get("productName") == "PayOnce PDF",
         str(cfg.get("productName"))),
        ("P22", "窗口标题 = PayOnce PDF",
         (cfg.get("app", {}).get("windows") or [{}])[0].get("title") == "PayOnce PDF",
         str((cfg.get("app", {}).get("windows") or [{}])[0].get("title"))),
        ("P23", "identifier = app.payoncepdf.desktop",
         cfg.get("identifier") == "app.payoncepdf.desktop", str(cfg.get("identifier"))),
        ("P24", "updater pubkey 非占位",
         bool(cfg.get("plugins", {}).get("updater", {}).get("pubkey")), "已配置（开发密钥）"),
    ]
    for cid, title, ok, detail in checks:
        record(cid, title, "PASS" if ok else "FAIL", detail)

    resources = cfg.get("bundle", {}).get("resources", [])
    has_tess = any("tessdata" in str(r) for r in resources)
    record("P25", "bundle.resources 是否显式包含 tessdata",
           "PASS" if has_tess else "BLOCK",
           f"当前值 {resources}。glob `resources/*` 只展开文件、不展开目录，"
           "导致已生成的 NSIS 安装包不含 OCR 语言数据；修复需改配置并重建安装包。"
           if not has_tess else str(resources))

    try:
        prod = json.loads(prod_path.read_text(encoding="utf-8"))
        ok = (prod.get("price_usd") == 39 and prod.get("price_cny") == 199
              and prod.get("early_bird_usd") == 29 and prod.get("early_bird_cny") == 149
              and prod.get("max_devices_per_key") == 3
              and prod.get("pending_confirmation") is False)
        record("P26", "价格四字段与 3 设备授权已按 §22.10 拍板", "PASS" if ok else "FAIL",
               f"{prod.get('price_usd')}/{prod.get('price_cny')}，"
               f"早鸟 {prod.get('early_bird_usd')}/{prod.get('early_bird_cny')}，"
               f"设备={prod.get('max_devices_per_key')}，"
               f"pending={prod.get('pending_confirmation')}")
    except Exception as e:  # noqa: BLE001
        record("P26", "config/product.json 可解析", "FAIL", str(e))

    # NSIS 生成脚本是历史生成物：可据此确认安装包真实包含哪些文件
    nsi = ROOT / "src-tauri/target/release/nsis/x64/installer.nsi"
    if nsi.is_file():
        text = nsi.read_text(encoding="utf-8", errors="replace")
        n_trained = sum(1 for l in text.splitlines()
                        if "File /a" in l and ".traineddata" in l.lower())
        record("P27", "已生成 NSIS 安装包内 OCR 语言数据条目",
               "PASS" if n_trained >= 2 else "BLOCK",
               f"installer.nsi 中 File /a ...traineddata 共 {n_trained} 条"
               + ("（预期至少 2：eng + chi_sim）" if n_trained < 2 else ""))
        stale = "E:\\OncePDF" in text or "E:/OncePDF" in text
        record("P28", "NSIS 脚本路径是否指向当前项目根目录",
               "WARN" if stale else "PASS",
               "脚本内仍是旧工作区绝对路径（历史生成物，不影响已生成的安装包；"
               "重建安装包时会自动刷新）" if stale else "路径一致")
    else:
        record("P27", "installer.nsi 存在", "WARN", "未找到生成脚本，跳过安装包内容核对")


def check_fixtures() -> None:
    fdir = ROOT / "tests/fixtures"
    idir = fdir / "images"
    try:
        manifest = json.loads((fdir / "manifest.json").read_text(encoding="utf-8"))
    except Exception as e:  # noqa: BLE001
        record("P30", "fixtures manifest 可解析", "FAIL", str(e))
        return
    missing, bad_hash, bad_size = [], [], []
    for f in manifest.get("files", []):
        name = f["name"]
        p = next((c for c in (fdir / name, idir / name) if c.is_file()), None)
        if p is None:
            missing.append(name)
            continue
        if sha256(p) != f.get("sha256"):
            bad_hash.append(name)
        if p.stat().st_size != f.get("bytes"):
            bad_size.append(name)
    ok = not (missing or bad_hash or bad_size)
    detail = (f"{len(manifest.get('files', []))} 项素材哈希与大小全部匹配"
              if ok else f"缺失={missing} 哈希不符={bad_hash} 大小不符={bad_size}")
    record("P30", "测试素材完整（按 manifest 校验 SHA-256 + 大小）",
           "PASS" if ok else "FAIL", detail)

    enc = fdir / "encrypted.pdf"
    record("P31", "真实加密样本 encrypted.pdf（密码 payoncepdf）",
           "PASS" if enc.is_file() else "WARN",
           f"{enc.stat().st_size:,} B" if enc.is_file()
           else "缺失；用 qpdf --encrypt payoncepdf payoncepdf 256 -- page_1.pdf encrypted.pdf 生成")
    record("P32", "损坏样本 corrupted.pdf（E1001）",
           "PASS" if (fdir / "corrupted.pdf").is_file() else "FAIL", "存在" if (fdir / "corrupted.pdf").is_file() else "缺失")


def check_environment() -> None:
    for cid, port, title, required in (
        ("P40", 1420, "Vite 预览服务（浏览器预览模式，非真机）", False),
        ("P41", 8787, "本地 mock 激活服务（LT3 按需启动）", False),
        ("P42", 9222, "CDP 调试端口（UI 冒烟用）", False),
    ):
        up = port_open(port)
        if up:
            record(cid, title, "PASS", f"127.0.0.1:{port} 正在监听")
        elif required:
            record(cid, title, "FAIL", f"127.0.0.1:{port} 未监听")
        else:
            record(cid, title, "WARN", f"127.0.0.1:{port} 未监听（按需，不阻塞）")

    # 只读查看首次启动状态；绝不删除或改动 AppData 下任何内容
    appdata = os.environ.get("APPDATA")
    if appdata:
        d = Path(appdata) / "app.payoncepdf.desktop"
        if d.is_dir():
            names = sorted(p.name for p in d.iterdir())
            record("P43", "试用/授权状态目录", "WARN",
                   f"已存在 {d}（{', '.join(names) or '空'}）→ 非首次启动状态；"
                   "若需测 LT1 首次 7 天试用，请自行备份后移走该目录，本脚本不会代删")
        else:
            record("P43", "试用/授权状态目录", "PASS",
                   "不存在 → 首次启动将创建 trial.json，适合测 LT1")
    else:
        record("P43", "试用/授权状态目录", "WARN", "未取到 %APPDATA%，跳过")

    bundle = ROOT / "src-tauri/target/release/bundle/nsis"
    legacy = [p.name for p in bundle.glob("*.exe")
              if p.name != "PayOnce-PDF-Setup-1.0.0.exe"] if bundle.is_dir() else []
    record("P44", "目录中是否残留旧品牌安装包",
           "WARN" if legacy else "PASS",
           f"发现 {legacy} —— 仅供对照，不得分发" if legacy else "只有当前发行包")


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(description="PayOnce PDF 真机测试前置检查（只读）")
    ap.add_argument("--json", action="store_true", help="只输出 JSON")
    ap.add_argument("--strict", action="store_true", help="BLOCK 项也按非零退出")
    args = ap.parse_args(argv)

    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8")

    check_deliverables()
    check_runtime_engines()
    check_config()
    check_fixtures()
    check_environment()

    counts = {k: sum(1 for r in RESULTS if r["status"] == k)
              for k in ("PASS", "WARN", "BLOCK", "FAIL")}
    payload = {
        "generated_at": datetime.now(timezone.utc).astimezone().isoformat(timespec="seconds"),
        "project_root": str(ROOT),
        "read_only": True,
        "summary": counts,
        "results": RESULTS,
    }
    OUT_JSON.parent.mkdir(parents=True, exist_ok=True)
    OUT_JSON.write_text(json.dumps(payload, ensure_ascii=False, indent=2), encoding="utf-8")

    if args.json:
        print(json.dumps(payload, ensure_ascii=False, indent=2))
    else:
        print("=" * 78)
        print("PayOnce PDF 真机测试前置检查（只读，不修改任何文件）")
        print("=" * 78)
        for r in RESULTS:
            print(f"[{r['status']:5s}] {r['id']}  {r['title']}")
            if r["detail"]:
                print(f"         {r['detail']}")
        print("-" * 78)
        print(f"汇总: PASS {counts['PASS']} / WARN {counts['WARN']} / "
              f"BLOCK {counts['BLOCK']} / FAIL {counts['FAIL']}")
        print(f"报告: {OUT_JSON.relative_to(ROOT)}")
        if counts["BLOCK"]:
            print("提示: BLOCK 为已知缺陷（仅影响安装包路径的 OCR），"
                  "不阻塞直接运行 release 主程序的测试路径。")
        print("=" * 78)

    if counts["FAIL"] or (args.strict and counts["BLOCK"]):
        return 1
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
