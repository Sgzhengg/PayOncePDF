# PayOnce PDF 授权公钥（Ed25519）

`license.rs` 在**首次联网激活**与**离线校验激活缓存**时，用本目录下的 `public.key`
验证服务端返回的 Ed25519 签名。`public.key` 是**公开**密钥，已入库并随发行包分发；
**私钥绝不入库、绝不随客户端分发**（开发私钥仅存于 gitignored `scripts/.dev-license-private.key`）。
2026-11-24 起公钥还有第二重保障：`include_str!` 编译期嵌入二进制，删除外部公钥文件不会改变验签行为。

## 文件格式

`public.key` = **base64 编码的原始 32 字节 Ed25519 公钥**（单行、无换行以外的空白）。
注意不是 PEM、不是 minisign 公钥文本，而是裸 32 字节经标准 base64（含 padding）后的字符串。

```
# 示意（仅格式示例，不是真实公钥）
cGxhY2Vob2xkZXIgMzItYnl0ZSBlZDI1NTE5IHB1YmxpYyBrZXkhIQ==
```

## 生成

在项目根目录运行：

```bash
node scripts/gen-license-keys.mjs
```

脚本应生成一对 Ed25519 密钥：

- 私钥 `keys/private.key`：**仅保存在激活服务端**，绝不放进客户端安装包；
- 公钥 `keys/public.key`：随 PayOnce PDF 安装包分发。

私钥签名对象 = 激活响应里 `payload`（base64 解码后）的**原始字节**；
`signature` = 该签名的 base64。响应形如：

```json
{ "payload": "<base64(JSON)>", "signature": "<base64(ed25519 64-byte sig)>" }
```

`payload` 的 JSON **必须**包含以下字段（缺任一字段或不一致，Rust 端一律拒绝激活 / 拒绝缓存）：

- `key`：完整 license key（明文），令牌与具体授权码强绑定；
- `fingerprint`：本机机器指纹哈希；
- `issuedAt`：ISO-8601 签发时间；
- `nonce`：一次性随机数（生产服务据此防重放；客户端只校验字段契约）；
- `expiresAt`：`null` 表示买断永久；给出时间则过期后拒绝。

`keyHash` 可继续保留作服务端索引，但**不能替代** `key` 字段。

## 运行时查找路径（按优先级）

`license.rs::candidate_key_paths()` 依次尝试：

1. `<resource_dir>/keys/public.key`
2. `<resource_dir>/public.key`
3. `<resource_dir>/resources/keys/public.key`
4. 开发态回退：`$CARGO_MANIFEST_DIR/keys/public.key`（`cargo run` 时有效）

`tauri.conf.json` 的 `bundle.resources` 已包含 `"keys/public.key"`，发行包会在
`<resource_dir>/keys/public.key` 放置一份；`scripts/verify-public-key.py` 校验这份
资源的 base64 解码为 32 字节，并核对 NSIS 生成清单确实收录了它。

若外部文件缺失或损坏，`read_public_key()` 回退到 `include_str!("../keys/public.key")`
编译期内嵌的那一份（信任根），因此**公钥永远可用**：`activation.json` 不再含
`verified` 自证字段，公钥不可用一律拒绝，不存在「删掉公钥即可离线放行」的路径。

> 轮换生产公钥时需同时更新 `keys/public.key` **并重新编译**，否则内嵌的旧公钥仍会
> 作为回退信任根生效。

## 安全说明

- 校验失败（签名不合法 / 32 字节长度不符 / payload 字段缺失或不匹配）一律拒绝激活，不写入缓存。
- 验签通过 + payload 强字段校验通过后才把 `payload`、`signature`、机器指纹缓存到
  `app_data_dir/activation.json`，之后完全离线可用；缓存不含任何「已验签」自证布尔值。
- 全程只发送 `{ key, fingerprint }`，`fingerprint` 为机器信息的 SHA256 哈希，
  不可逆、不含任何文件内容；绝不上传用户文件、不做遥测。
