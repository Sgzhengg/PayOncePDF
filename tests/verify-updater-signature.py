#!/usr/bin/env python3
"""Verify a Tauri updater ``.sig`` against the public key in ``tauri.conf.json``.

This is a reproducible, private-key-free implementation of the exact algorithm
used at runtime by ``minisign-verify 0.3.0`` (the crate Tauri's updater uses).
It exists so the ``site/verify.html`` claim "you can check the update signature
yourself" is actually auditable instead of being a one-off manual check.

Wire format (both the pubkey and the .sig file are OUTER base64-encoded text):

  public key blob (42 B) = algorithm(2) + key_id(8) + Ed25519 public key(32)
  signature blob  (74 B) = algorithm(2) + key_id(8) + Ed25519 signature(64)
      b"Ed" (0x45,0x64) -> legacy, signs the raw file bytes
      b"ED" (0x45,0x44) -> prehashed, signs BLAKE2b-512(file bytes)
  global signature (64 B) signs signature(64) + trusted_comment[17:]
      [17:] strips the literal "trusted comment: " prefix

Usage:
    python tests/verify-updater-signature.py
    python tests/verify-updater-signature.py path/to/other-setup.exe

Requires Python 3.8+ and ``cryptography`` (Ed25519). Exit code 0 = valid,
1 = invalid, 2 = missing/malformed input.
"""

from __future__ import annotations

import base64
import hashlib
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
CONF = ROOT / "src-tauri" / "tauri.conf.json"
DEFAULT_INSTALLER = (
    ROOT / "src-tauri" / "target" / "release" / "bundle" / "nsis"
    / "PayOnce-PDF-Setup-1.0.0.exe"
)


def outer_decode(raw: str) -> str:
    """Tauri stores both pubkey and .sig as base64 of the minisign text."""
    return base64.b64decode(raw.strip(), validate=True).decode("utf-8")


def load_public_key() -> tuple[bytes, bytes]:
    conf = json.loads(CONF.read_text(encoding="utf-8"))
    text = outer_decode(conf["plugins"]["updater"]["pubkey"])
    lines = text.splitlines()
    blob = base64.b64decode(lines[1].strip(), validate=True)
    if len(blob) != 42 or blob[:2] not in (b"Ed", b"ED"):
        raise ValueError("pubkey blob 不是合法的 42 字节 minisign 公钥")
    return blob[2:10], blob[10:42]


def load_signature(sig_path: Path) -> tuple[bytes, bytes, bool, bytes, str]:
    text = outer_decode(sig_path.read_text(encoding="utf-8"))
    lines = text.splitlines()
    if len(lines) < 4:
        raise ValueError(".sig 内容不足 4 行")
    bin1 = base64.b64decode(lines[1].strip(), validate=True)
    bin2 = base64.b64decode(lines[3].strip(), validate=True)
    if len(bin1) != 74 or len(bin2) != 64:
        raise ValueError(".sig 的签名块长度不是 74 / 64 字节")
    if not lines[2].startswith("trusted comment: "):
        raise ValueError("缺少 'trusted comment: ' 前缀")
    algorithm = bin1[:2]
    if algorithm == b"ED":
        prehashed = True
    elif algorithm == b"Ed":
        prehashed = False
    else:
        raise ValueError("不支持的 minisign 算法：" + repr(algorithm))
    return bin1[2:10], bin1[10:74], prehashed, bin2, lines[2][17:]


def main() -> int:
    installer = Path(sys.argv[1]).resolve() if len(sys.argv) > 1 else DEFAULT_INSTALLER
    sig_path = Path(str(installer) + ".sig")
    try:
        from cryptography.exceptions import InvalidSignature
        from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PublicKey
    except ImportError:
        print("缺少依赖：pip install cryptography", file=sys.stderr)
        return 2

    for path in (installer, sig_path, CONF):
        if not path.is_file():
            print(f"文件不存在：{path}", file=sys.stderr)
            return 2

    try:
        pub_id, pub_bytes = load_public_key()
        sig_id, sig_bytes, prehashed, global_sig, trusted_comment = load_signature(sig_path)
    except (ValueError, KeyError, UnicodeDecodeError) as exc:
        print(f"格式错误：{exc}", file=sys.stderr)
        return 2

    data = installer.read_bytes()
    pub = Ed25519PublicKey.from_public_bytes(pub_bytes)

    print(f"安装包      : {installer.name} ({len(data):,} B)")
    print(f"签名文件    : {sig_path.name} ({sig_path.stat().st_size} B)")
    print(f"key id      : pubkey={pub_id.hex()} signature={sig_id.hex()}")
    print(f"算法        : {'ED / prehashed (BLAKE2b-512)' if prehashed else 'Ed / legacy (raw bytes)'}")
    print(f"trusted cmt : {trusted_comment}")

    if pub_id != sig_id:
        print("FAIL: key id 不匹配，签名不是用这把公钥对应的私钥生成的")
        return 1

    message = hashlib.blake2b(data, digest_size=64).digest() if prehashed else data
    try:
        pub.verify(sig_bytes, message)
        print("① 文件签名 OK   -> Ed25519(pubkey, BLAKE2b-512(安装包))")
    except InvalidSignature:
        print("FAIL: ① 文件签名无效（安装包被改动，或 .sig 不是这个包的）")
        return 1

    global_message = sig_bytes + trusted_comment.encode("utf-8")
    try:
        pub.verify(global_sig, global_message)
        print("② 全局签名 OK   -> trusted comment 未被篡改")
    except InvalidSignature:
        print("FAIL: ② 全局签名无效（trusted comment 或签名被改动）")
        return 1

    # Negative self-test: prove the check is real, not a vacuous success path.
    tampered = bytearray(message)
    tampered[-1] ^= 0x01
    try:
        pub.verify(sig_bytes, bytes(tampered))
        print("FAIL: ③ 负向自检失败（改动一个字节后仍验签通过？）")
        return 1
    except InvalidSignature:
        print("③ 负向自检 OK   -> 篡改 1 bit 即验签失败")

    print(f"=> 更新签名完整有效：与 tauri.conf.json 公钥匹配，覆盖当前 {len(data) / 1048576:.2f} MiB 安装包")
    return 0


if __name__ == "__main__":
    sys.exit(main())
