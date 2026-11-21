# PayOnce PDF 授权与试用 手动测试步骤（F8 / F9）

本文件是 **人工测试清单**，覆盖 PRD 7 F8/F9 与 CONTRACT 第 8 节中
"断网可用、试用到期只限导出"等无法在 CI 冒烟中判定的项目。
`tests/acceptance.py --smoke` 会把这些项目输出为 `SKIP (manual)`，
**不会伪装成 PASS**；发布前必须由人工在真机 / 真实安装包上执行并签字。

| 项目 | 对应验收 | 关联错误码 |
|---|---|---|
| LT1 7 天全功能试用 | F8 "7 天全功能，无需邮箱" | — |
| LT2 断网可用（除激活） | F8 "断网可正常使用" / PRD 14.2 | — |
| LT3 首次联网激活 | F8 "首次联网校验一次" | E2004（参数非法） |
| LT4 离线缓存签名 | F8 "之后离线可用（缓存签名）" | — |
| LT5 试用到期只限导出、不删文件 | F8 "到期后只限制导出，不删除用户文件" | E2005 |
| LT6 更新检查可关闭 | F9 "检查更新可关闭" | — |

## 0. 前置条件（每次执行前确认）

1. 拥有**真实安装包**（Windows `.msi/.exe` 或 macOS `.dmg`，已签名/公证），
   在干净虚拟机或真机安装，非浏览器预览模式（预览模式不调用本地引擎）。
2. 准备开发联调服务（仅本地）：
   ```bash
   node scripts/gen-license-keys.mjs     # 生成开发密钥对
   node scripts/license-server.mjs       # http://127.0.0.1:8787/api/activate
   ```
   Rust 端用 `PAYONCEPDF_ACTIVATE_URL=http://127.0.0.1:8787/api/activate` 指向 mock。
   **生产环境必须替换为 Gumroad / 爱发电 的真实校验服务**，见
   `src-tauri/keys/README.md`。
3. 准备测试密钥 `PAYONCE-ABCD-EFGH-IJKL`（符合 `PAYONCE-XXXX-XXXX-XXXX`）。
4. 准备一份用户文件（例如 `tests/fixtures/page_100.pdf`）及其 sha256，
   用于 LT5 验证"原文件不被修改"。
5. 抓包工具（如 Wireshark / mitmproxy）用于 LT2 / LT6 验证"无上传 / 无更新请求"。
6. 记录每项的实际结果、构建版本号、平台、日期、执行人。

---

## LT1 7 天全功能试用

前置：全新安装，未激活，系统时间为真实当前时间，不提供任何邮箱/账号。

步骤：
1. 首次启动，记录 `get_license_state` 返回：
   `status = "trial"`、`trialDaysLeft = 7`、`canExport = true`。
2. 依次执行合并、拆分、压缩、图片互转、OCR、签名、批量（F1–F7）各一次。
3. 确认全程无"登录 / 注册 / 购买"弹窗，PRD 19 禁止项均不存在。

期望：
- 无需邮箱、无需联网即可使用全部功能；
- 试用剩余天数随日期递减，界面显示与 `trialDaysLeft` 一致；
- 首屏英文原文 "Your files never leave this computer." 完整显示。

---

## LT2 断网可用（除激活）

前置：全新安装且**未激活**（试用期内在网/断网都应可用）。

步骤：
1. 物理断开网络（或防火墙阻断全部出站）。
2. 冷启动应用，确认 ≤3 秒内可用（PRD 8）。
3. 在断网状态下重复 LT1 的 F1–F7 全部操作。
4. 抓包确认无任何文件外发请求（PRD 6 "可验证"、14.2 "无上传"）；
   断网期间允许**仅激活**相关请求失败，但不得影响其它功能。

期望：
- 除"首次激活"外，所有功能断网可用；
- 抓包无文件外发；本地日志默认关闭，开启后仅记录操作类型与错误码。

---

## LT3 首次联网激活

前置：全新安装，试用期第 1 天，网络可用，mock 服务已启动。

步骤：
1. 调用 `activate_license({ payload: { key: "PAYONCE-ABCD-EFGH-IJKL" } })`。
2. 用无效密钥（如 `BAD-KEY`）再调用一次，观察拒绝。
3. 检查返回 `get_license_state`：`status = "active"`、`canExport = true`。
4. 检查 mock 返回的 `payload`（base64 解码后）含
   `keyHash` / `fingerprint` / `issuedAt` / `expiresAt: null`，
   且 `signature` 能用 `src-tauri/keys/public.key` 验证通过
   （可用 `node scripts/gen-license-keys.mjs --check` 核对公钥指纹）。

期望：
- 合法密钥激活成功并变为 `active`；
- 非法密钥返回 `E2004`（参数非法），不写入缓存、不改动用户文件；
- 激活请求只发送密钥与机器指纹哈希，**不发送任何文件内容**（PRD 10）。

---

## LT4 离线缓存签名

前置：已完成 LT3 激活。

步骤：
1. 完全断网并重启应用（冷启动）。
2. 检查 `get_license_state` 仍为 `active`、`canExport = true`。
3. 断网状态下执行导出类操作（合并/拆分/压缩/转图/OCR 输出/签名）。
4. 抓包确认**没有任何激活或更新请求**发出。
5. 反复断网/联网/重启 3 次，状态保持一致。

期望：
- 激活签名被缓存到本地，断网重启后依旧 `active`；
- 除首次激活外不再产生网络请求；
- 缓存文件不被误删；清理临时文件时不得清除激活缓存。

---

## LT5 试用到期只限导出、不删除用户文件（重点）

前置：全新安装，未激活，记录用户文件 `page_100.pdf` 的 sha256 与大小。

步骤：
1. 让试用期满（推荐：在测试构建中提供"时间偏移"开关模拟第 8 天；
   或在隔离虚拟机里修改系统时间，避免影响其它用例）。
2. 确认 `get_license_state` 变为 `status = "expired"`、`trialDaysLeft = 0`、
   `canExport = false`。
3. 尝试任一导出命令（合并/拆分/压缩/转图/OCR 输出/签名/批量导出），
   确认返回 `E2005`（试用已结束；文件仍在，激活后可导出）。
4. **核对原文件**：sha256 与大小与步骤 0 完全一致，未被删除、截断或改写。
5. 确认仍可打开、预览、浏览文件列表（只限制导出，不限制查看）。
6. 执行 LT3 激活后重试导出，确认成功且原文件依旧不变。
7. 检查 `std::env::temp_dir()/payoncepdf-<jobId>/` 临时文件在任务结束后被清理。

期望：
- 到期后仅导出被限制，返回 `E2005`；
- **绝不删除或改写任何用户原文件**（CONTRACT 1 工程红线）；
- 失败不损坏原文件；任何写操作先写临时文件再原子替换；
- 激活后导出能力恢复，不需要重新导入文件。

---

## LT6 更新检查可关闭

前置：已安装旧版本，更新端点可达（或本地 mock 更新端点）。

步骤：
1. 在设置中**关闭**"检查更新"。
2. 冷启动应用，抓包确认没有向
   `https://payoncepdf.com/api/update/{{target}}/{{arch}}/{{current_version}}`
   或任何更新端点发出请求。
3. 重新开启检查更新，确认能检测到新版本并升级；
   升级失败时确认可回滚（F9）。
4. 确认 Updater 使用签名校验（`tauri.conf.json` 的 `plugins.updater.pubkey`
   为 minisign 公钥，与授权公钥是两套密钥，见 `src-tauri/keys/README.md`）。

期望：
- 关闭后零更新请求；开启后能检测/升级/失败回滚；
- 更新包签名校验失败时拒绝安装，不覆盖现有可用版本。

---

## 结果记录

| 用例 | 平台 / 版本 | 结果 (PASS / FAIL / SKIP manual) | 备注 / 缺陷号 |
|---|---|---|---|
| LT1 7 天试用 | | | |
| LT2 断网可用 | | | |
| LT3 首次激活 | | | |
| LT4 离线缓存 | | | |
| LT5 到期只限导出 | | | |
| LT6 更新可关闭 | | | |

执行人：__________  日期：__________  构建号：__________

> 说明：以上用例需要真实 app / 真机 / 双平台 / 证书 / 公证，`acceptance.py --smoke`
> 中统一输出 `SKIP (manual)`。自动可判定的部分（fixtures 页数/顺序、
> report.csv 字段、错误码枚举 E1001–E1007 + E2001–E2005 完整性）见
> `tests/out/acceptance-report.md`。
