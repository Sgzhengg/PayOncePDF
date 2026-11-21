# PayOnce PDF 授权公钥（Ed25519）

`license.rs` 在**首次联网激活**时，需要本目录下的 `public.key` 来验证服务端返回的
Ed25519 签名。此目录里的文件不会提交到仓库（属于部署密钥材料）。

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

`payload` 的 JSON 建议至少包含 `key` 与 `fingerprint`，后端会据此把令牌绑定到
本机与密钥（若字段存在则校验）。

## 运行时查找路径（按优先级）

`license.rs::candidate_key_paths()` 依次尝试：

1. `<resource_dir>/keys/public.key`
2. `<resource_dir>/public.key`
3. `<resource_dir>/resources/keys/public.key`
4. 开发态回退：`$CARGO_MANIFEST_DIR/keys/public.key`（`cargo run` 时有效）

> **需主流程协调**：当前 `tauri.conf.json` 的 `bundle.resources = ["resources/*"]`
> 并未包含 `keys/`，因此打包后的安装包里默认找不到公钥。
> 请选择其一：
> - 把生成的 `public.key` 复制到 `src-tauri/resources/`（随 `resources/*` 打包）；或
> - 在 `tauri.conf.json` 的 `bundle.resources` 增加 `"keys/public.key"`。
>
> 未随包提供公钥时：`activate_license` 会返回可识别的 E2004 错误，并在本地日志里
> 提示运行 `scripts/gen-license-keys.mjs`；`get_license_state`、试用与离线使用不受影响，
> 绝不会 panic。

## 安全说明

- 校验失败（签名不合法 / 公钥缺失 / 32 字节长度不符）一律拒绝激活，不写入缓存。
- 验签通过后才把 `payload`、`signature`、机器指纹缓存到 `app_data_dir/activation.json`，
  之后完全离线可用。
- 全程只发送 `{ key, fingerprint }`，`fingerprint` 为机器信息的 SHA256 哈希，
  不可逆、不含任何文件内容；绝不上传用户文件、不做遥测。
