# PayOnce PDF 客服回复模板

> 用途：邮件支持（无账号体系，只用邮箱与订单号定位问题）。中英各一份，覆盖退款、激活、OCR 效果、macOS 安装被拦截等高频场景。
> 通用原则：**诚实、不夸大**——不写「100% 准确」「永久免费」「lifetime updates」「零网络请求」；做不到的事（破解密码、**高保真** Word/Excel 转换、云端文件处理/AI 问答）直接说明不支持（F14/F16 只是文字层降级能力，务必说明边界）；不向用户索要 PDF 原件（我们本就不接收文件）；涉及时如实说明应用有**三类可枚举网络请求**（激活 / 可选更新检查 / 用户主动安装 OCR 语言包），三者都不含文件内容与文件名；必要时引导用户自行抓包（`site/verify.html`）。

<!-- 联系邮箱已统一为 hello@payoncepdf.com；上线前需确认 DNS / MX 与收件可达 -->
<!-- TODO(商务信息): 填入 Gumroad（海外）与 爱发电（国内）退款/订单查询入口 -->

---

## 0. 通用占位符

| 占位符 | 含义 |
|---|---|
| `hello@payoncepdf.com` | 支持邮箱 |
| `hello@payoncepdf.com` | 退款邮箱 |
| `TODO(order-id)` | 店铺订单号 |
| `TODO(version)` | PayOnce PDF 版本号 |
| `TODO(refund-link)` | 店铺退款/订单查询入口 |
| `hello@payoncepdf.com` | 团队采购邮箱 |

---

## 1. 退款（Refund）

### 中文

```
主题：PayOnce PDF 退款已受理 —— 订单 TODO(order-id)

你好，

已收到你的退款申请，符合「购买后 7 天无理由退款」政策，我们已受理，无需说明理由，
也没有账号或订阅需要取消。

- 订单号：TODO(order-id)
- 退款渠道：原支付渠道退回（Gumroad / 爱发电）
- 到账时间：以支付渠道为准，通常为若干工作日
- 后续：本应用不收集用户数据，退款后我们这边不保留任何与你相关的记录

如 7 个工作日内未在原渠道看到退款，请回复本邮件并附上订单号，我们会跟进。
也可直接在店铺订单页自助申请：TODO(refund-link)。

若之后仍想使用，已生成的许可证密钥在退款后不再有效；欢迎重新购买。

PayOnce PDF 支持 · hello@payoncepdf.com
```

### English

```
Subject: PayOnce PDF refund accepted — order TODO(order-id)

Hello,

Your refund request is accepted under our 7-day no-questions-asked policy.
There is no reason to give and no account or subscription to cancel.

- Order: TODO(order-id)
- Refund goes back to the original payment method (Gumroad / 爱发电)
- Timing depends on the payment provider (usually a few business days)
- We keep no user data, so nothing about you remains on our side afterwards

If the refund has not appeared within 7 business days, reply with the order
number and we will follow up. You can also self-serve at: TODO(refund-link).
After a refund the issued licence key is no longer valid.

PayOnce PDF Support · hello@payoncepdf.com
```

---

## 2. 激活问题（Activation）

### 中文

```
主题：PayOnce PDF 激活问题 —— 请按以下步骤重试

你好，

激活流程是：输入 PAYONCE-XXXX-XXXX-XXXX 密钥 → **首次联网校验一次** → 本机缓存签名令牌 →
之后完全离线可用。请按顺序检查：

1. 确认密钥完整、无多余空格，形如 PAYONCE-XXXX-XXXX-XXXX。
2. 首次激活需要联网一次；若当前断网，请临时联网后再试（仅此一次）。
3. 若提示「密码错误 / 无效密钥」，请核对是否复制了完整四段。
4. 若曾重装系统或更换机器，可能需要重新激活一次。
5. 若组件自检异常（E2001/E2002），先重装完整安装包。

我们不会索要你的 PDF 文件；激活只发送密钥与不可逆的机器指纹哈希，
不含任何文件信息（详见 site/privacy.html）。

如果仍失败，请回复：错误码（如有）、PayOnce PDF 版本 TODO(version)、操作系统、
以及「关于 / 网络活动」里的自检结果。注意：试用到期只限制导出，不会删除文件。

PayOnce PDF 支持 · hello@payoncepdf.com
```

### English

```
Subject: PayOnce PDF activation — please retry these steps

Hello,

Activation: enter your PAYONCE-XXXX-XXXX-XXXX key -> one online check -> a signed
token is cached locally -> fully offline afterwards. Please check in order:

1. Key is complete, no extra spaces, format PAYONCE-XXXX-XXXX-XXXX.
2. The first activation needs one online check; connect briefly if offline.
3. "Invalid key" usually means a truncated copy — re-copy all four segments.
4. After an OS reinstall or a new machine you may need to activate once more.
5. If self-check shows E2001/E2002, reinstall the full package first.

We will never ask for your PDFs. Activation sends only the key and an
irreversible machine-fingerprint hash — no file data (see site/privacy.html).

If it still fails, reply with any error code, version TODO(version), your OS,
and the About / Network self-check result. Trial expiry limits export only and
never deletes your files.

PayOnce PDF Support · hello@payoncepdf.com
```

---

## 3. OCR 效果不佳（OCR quality）

### 中文

```
主题：关于 PayOnce PDF 本地 OCR 的识别效果

你好，

谢谢反馈。先说明我们的口径，避免误解：本地 OCR 的验收目标是
**英文 ≥ 95%、中文 ≥ 90%（清晰扫描件、标准测试集）**，这是测试指标，不是对所有文档的保证。
我们不承诺「100% 准确」，也不提供云端 OCR 兜底。

效果下降常见于：复杂表格、手写体、低分辨率或倾斜扫描、印章压字、特殊字体、多栏混排。
可尝试：

1. 只勾选需要的语言（eng / chi_sim），避免多语言混识。
2. 用「页码范围」先试几页，确认效果后再整本处理。
3. 尽量使用 300DPI 清晰扫描件；先做压缩/去歪斜不是 v1 能力，请从源头保证扫描质量。
4. 输出选「可搜索 PDF」时，文字层是叠加的，个别字符识别错属正常。

对于复杂版式，我们宁可说明局限，也不会夸大效果。如果你的文档属于上述难点类型，
说明当前版本确实不适合，我们不会假装能做好。欢迎附上「文档类型描述」（不必发文件）
与页码范围，我们可给出更具体的建议。

PayOnce PDF 支持 · hello@payoncepdf.com
```

### English

```
Subject: About PayOnce PDF local OCR accuracy

Hello,

Thanks for writing. So expectations are clear: our acceptance targets are
>= 95% English and >= 90% Chinese on clean, standard test scans. These are
measured targets, not a guarantee for every document. We do not claim
"100% accurate" and we do not offer cloud OCR as a fallback.

Accuracy commonly drops on complex tables, handwriting, low-resolution or
skewed scans, stamps over text, unusual fonts, and multi-column layouts.

Things to try:
1. Select only the language you need (eng / chi_sim).
2. Use a page range to test a few pages first.
3. Prefer clean 300DPI scans; v1 has no deskew/preprocessing stage.
4. With searchable-PDF output the text layer is overlaid, so an occasional
   mis-recognised character is expected.

For complex layouts we would rather state the limitation than overpromise;
if your document falls into those categories, v1 is honestly not a good fit.
You may describe the document type (no need to send the file) and page range
and we will advise.

PayOnce PDF Support · hello@payoncepdf.com
```

---

## 4. macOS 安装被拦截 / Gatekeeper（macOS install blocked）

### 中文

```
主题：PayOnce PDF 在 macOS 上被 Gatekeeper 拦截怎么办

你好，

官方发行版使用 Apple Developer ID 签名，并经过 notarytool 公证与 staple，
正常情况下 Gatekeeper 会直接放行。我们**不上 Mac App Store**（沙箱会限制
内置组件与任意文件访问）。

若出现「无法打开，因为无法验证开发者」：

1. 右键点击 PayOnce PDF → 「打开」，在弹窗里再点「打开」。
2. 或：系统设置 → 隐私与安全性 → 找到提示 → 「仍要打开」。
3. 确认下载来源是官网的签名 .dmg，而不是第三方转发包。
4. 打开「关于 / 网络活动」核对组件自检是否正常（E2001/E2002）。

重要：如果应用**没有**被公证、或签名主体不是官方，请不要绕过 Gatekeeper，
直接把拦截截图发给我们 —— 这可能不是官方构建，我们不会要求你禁用系统保护。

<!-- TODO(macOS 签名): 公证流水线就绪后补充官方签名主体名称与 spctl/codesign 验证命令 -->

如需自行核对签名，可运行（发布后补充确切值）：TODO(codesign / spctl 验证命令)。

PayOnce PDF 支持 · hello@payoncepdf.com
```

### English

```
Subject: PayOnce PDF blocked by Gatekeeper on macOS

Hello,

Official builds are signed with an Apple Developer ID, notarized with
notarytool and stapled, so Gatekeeper should allow them normally. We are not
on the Mac App Store (sandboxing would restrict bundled components and file
access).

If you see "cannot be opened because the developer cannot be verified":
1. Right-click PayOnce PDF -> Open, then confirm Open in the dialog.
2. Or System Settings -> Privacy & Security -> "Open Anyway".
3. Make sure the .dmg came from our official signed download, not a mirror.
4. Check About / Network for component self-check (E2001/E2002).

Important: if the app is NOT notarized or the signer is not ours, do NOT
bypass Gatekeeper — send us a screenshot instead. It may not be an official
build, and we will never ask you to disable system protection.

<!-- TODO(macOS signing): add the official signer name and spctl/codesign verification commands once notarization is live -->
Verification command: TODO(codesign / spctl command).

PayOnce PDF Support · hello@payoncepdf.com
```

---

## 5. 加密 PDF / 密码（Encrypted PDF / password）

### 中文

```
主题：关于加密 PDF 与密码（E1002 / E1003）

你好，

PayOnce PDF 只支持**你已知的密码**：输入正确密码后可读取、可移除该已知密码。
我们**不破解密码、不绕过 DRM、不找回遗忘的密码**（PRD §19 明确不做）。

- E1002「该文件有密码，请输入」：在参数面板填入已知密码后重试。
- E1003「密码不正确」：检查大小写、全半角、首尾空格，重新输入。
- 忘记密码：无法处理，请向文件来源索取密码。

批量处理时，这类文件会记为 failed 并写入 report.csv 的 error_code，不会中断队列，
也不会改动原文件。

PayOnce PDF 支持 · hello@payoncepdf.com
```

### English

```
Subject: Encrypted PDFs and passwords (E1002 / E1003)

Hello,

PayOnce PDF only supports passwords you already know: enter it to open the file,
and we can remove that known password from the output. We do not crack
passwords, bypass DRM, or recover forgotten ones (explicit non-goal, PRD §19).

- E1002 "this file has a password": enter the known password and retry.
- E1003 "incorrect password": check case, full-width characters, and spaces.
- Forgotten password: we cannot process the file — ask the source for it.

In batch mode such files are recorded as failed with an error code in
report.csv; the queue continues and originals are never modified.

PayOnce PDF Support · hello@payoncepdf.com
```

---

## 6. 试用到期（Trial expired / E2005）

### 中文

```
主题：试用已结束（E2005）—— 你的文件安全

你好，

试用期为 7 天全功能、无需邮箱。到期后**只限制导出**，并返回 E2005：

- 你的原始文件、此前导出的文件**绝不会被删除或改写**；
- 应用不会去改动你文件夹里的任何内容；
- 输入 PAYONCE-XXXX-XXXX-XXXX 密钥激活后，导出立即恢复（首次需联网一次）。

如果你认为试用天数计算有误，请回复购买/首次启动时间与版本 TODO(version)，
我们按订单号 TODO(order-id) 核对。退款政策见「7 天无理由退款」。

PayOnce PDF 支持 · hello@payoncepdf.com
```

### English

```
Subject: Trial ended (E2005) — your files are safe

Hello,

The 7-day full-feature trial needs no email. After it ends, only export is
limited and the app returns E2005:

- Your original files and previously exported files are never deleted or
  modified;
- The app does not touch anything else in your folders;
- Entering a PAYONCE-XXXX-XXXX-XXXX key restores export immediately (one online
  check on first activation).

If you believe the trial days were miscalculated, reply with purchase/first
launch dates and version TODO(version); we will check against order
TODO(order-id). Refund policy: 7-day no-questions-asked.

PayOnce PDF Support · hello@payoncepdf.com
```

---

## 7. 更新失败 / 自动更新（Update failed）

### 中文

```
主题：自动更新失败的处理

你好，

更新检查是**可选**的，可在设置里关闭；关闭后应用仍完全可用、离线可用，
只是不会提示新版本。更新包经过签名校验（Tauri Updater + minisign 公钥），失败应可回滚。

请按顺序：

1. 确认当前版本 TODO(version) 与网络（更新检查需联网，但关闭后不影响功能）。
2. 重试一次；若反复失败，先关闭「检查更新」继续使用，不影响任何本地功能。
3. 确认安装包来自官网签名构建，不要手动替换应用内部文件。
4. 若升级后出现异常，保留旧版本可回滚，并附错误码（如有）回复我们。

更新请求只携带版本 / 平台 / 架构，不含文件内容；关闭后且未安装额外语言包时，
「首次激活」是应用唯一一次网络请求。应用共有三类可枚举请求（一次性激活、可选更新检查、
用户主动安装 OCR 语言包），三者都不含文件内容与文件名，详见 site/verify.html。

PayOnce PDF 支持 · hello@payoncepdf.com
```

### English

```
Subject: Handling a failed automatic update

Hello,

Update checks are optional and can be turned off in settings; the app stays
fully usable offline when they are off. Updates are signature-verified
(Tauri Updater + minisign public key) and a failed update can be rolled back.

Please try, in order:
1. Confirm current version TODO(version) and connectivity (checks need
   network; turning them off does not affect features).
2. Retry once; if it keeps failing, switch off update checks and keep working.
3. Use only official signed builds; do not manually replace files inside the
   app bundle.
4. If something breaks after upgrading, roll back to the previous version and
   reply with any error code.

Update requests carry only version/platform/architecture — no file content.
With checks off and no extra language pack installed, the single first
activation is the app's only request. The app has exactly three enumerable
request types (one-time activation, optional update check, and an OCR language
pack download you trigger yourself); none carries file content or file names
(see site/verify.html).

PayOnce PDF Support · hello@payoncepdf.com
```

---

## 8. 关联文档

- `docs/error-codes.md`（错误码、排查步骤、用户报错邮件模板）
- `site/faq.html`（用户侧 FAQ）
- `site/verify.html`（抓包与断网验证）
- `site/privacy.html`（激活与网络请求说明）
- `docs/user-manual.md`（安装、操作、卸载与日志清理）
