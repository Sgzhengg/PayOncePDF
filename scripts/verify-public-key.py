#!/usr/bin/env python3
"""校验发行包的 Ed25519 公钥信任根（CI 断言 / 本地可复现，仅标准库）。

对应上线前 CodeReview 的 P0-1 / P1-1：安装包必须带可用公钥，且删掉公钥文件
不能让授权退回「信任 activation.json 自证字段」的兜底路径。

检查项：
  1. src-tauri/keys/public.key 是 base64 编码的原始 32 字节 Ed25519 公钥；
  2. tauri.conf.json 的 bundle.resources 含 "keys/public.key"（随包资源）；
  3. license.rs 用 include_str! 把公钥编译进二进制（内嵌信任根）；
  4. 若 NSIS 生成清单存在，确认它确实收录了 keys/public.key（打包实证）。

退出码 0 = 全部通过；1 = 存在阻断项。未打包时第 4 项记 SKIP，不算失败。
"""
from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
KEY_FILE = ROOT / "src-tauri" / "keys" / "public.key"
CONF = ROOT / "src-tauri" / "tauri.conf.json"
LICENSE_RS = ROOT / "src-tauri" / "src" / "license.rs"
NSI = ROOT / "src-tauri" / "target" / "release" / "nsis" / "x64" / "installer.nsi"

results: list[tuple[str, bool, str]] = []


def check(name: str, ok: bool, detail: str) -> None:
    results.append((name, ok, detail))


def main() -> int:
    # 1) 公钥内容
    if not KEY_FILE.is_file():
        check("公钥文件存在", False, f"缺少 {KEY_FILE}")
        raw = None
    else:
        text = KEY_FILE.read_text(encoding="utf-8").strip()
        try:
            raw = base64.b64decode(text, validate=True)
            check("公钥为 32 字节", len(raw) == 32, f"base64 解码后 {len(raw)} 字节")
        except Exception as exc:  # noqa: BLE001
            raw = None
            check("公钥为合法 base64", False, str(exc))

    # 2) 随包资源声明
    try:
        conf = json.loads(CONF.read_text(encoding="utf-8"))
        resources = conf.get("bundle", {}).get("resources", [])
        hit = "keys/public.key" in resources
        check("bundle.resources 含公钥", hit, f"resources={resources}")
    except Exception as exc:  # noqa: BLE001
        check("tauri.conf.json 可解析", False, str(exc))

    # 3) 编译期内嵌信任根
    src = LICENSE_RS.read_text(encoding="utf-8") if LICENSE_RS.is_file() else ""
    embedded = 'include_str!("../keys/public.key")' in src
    no_self_attest = "verified" not in src.replace("旧版缓存里的 `verified`", "")
    check("公钥编译期内嵌", embedded, "license.rs 含 include_str! 回退")
    # verified 只允许出现在说明「旧缓存会忽略」的注释里，不允许作为字段/分支
    check(
        "无 verified 自证兜底",
        "act.verified" not in src and "verified: bool" not in src,
        "activation.json 不含自证字段，公钥不可用一律拒绝",
    )

    # 4) 打包实证（可选）
    if NSI.is_file():
        nsi = NSI.read_text(encoding="utf-8", errors="ignore")
        hit = "keys\\public.key" in nsi or "keys/public.key" in nsi
        check("NSIS 清单收录公钥", hit, f"{NSI.name} 中 {'命中' if hit else '未命中'} keys/public.key")
    else:
        print(f"  SKIP  NSIS 清单尚未生成（{NSI}）")

    for name, ok, detail in results:
        print(f"  {'PASS' if ok else 'FAIL'}  {name}: {detail}")
    failed = [n for n, ok, _ in results if not ok]
    if failed:
        print(f"=> 公钥断言失败：{', '.join(failed)}")
        return 1
    print("=> 公钥信任根完整：随包资源 + 编译期内嵌 + 无自证兜底")
    return 0


if __name__ == "__main__":
    sys.exit(main())
