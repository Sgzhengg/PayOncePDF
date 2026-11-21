# PayOnce PDF 错误码表与支持邮件模板

> 来源：`PRD.md` §13 的 **7 个基础码**（E1001–E1007）+ `CONTRACT.md` §2 的 **5 个扩展码**（E2001–E2005，以 E2xxx 标记且不改动原表）= **共 12 码**。
> 后端统一序列化为 `{ "code": "E1001", "message": "..." }`（`CONTRACT.md` §1）。长任务失败时同样以 `AppError` 返回，`ok=false`。
> 通用红线：导出前必调 `license::ensure_export_allowed()`；任何失败都**不修改、不删除用户原文件**；输出绝不原地覆盖输入。

---

## 1. 完整错误码表

| 码 | 含义 | 触发条件 | 用户提示（界面文案） | 用户排查 / 解决步骤 |
|---|---|---|---|---|
| **E1001** | 文件损坏 / 无法解析 | 文件被截断、不是合法 PDF、页对象/交叉引用损坏、无法解析的加密结构 | 文件无法读取，已跳过 | 1) 用其它 PDF 阅读器打开确认；2) 从原始来源重新导出/重新下载；3) 检查文件是否被截断（对比大小）；4) 批量任务中该文件记为 `failed`，队列不中断，可单独重做 |
| **E1002** | 需要密码 | 打开加密 PDF 但未提供 `password` | 该文件有密码，请输入 | 1) 在参数面板输入**你已知的密码**；2) 合并/拆分/压缩/OCR 均支持密码字段；3) 若不知道密码，本应用无法处理（不破解）；4) 批量中该文件跳过并记录 |
| **E1003** | 密码错误 | 提供的密码不正确 | 密码不正确 | 1) 重新输入，注意大小写、全半角、首尾空格；2) 确认是打开密码而非权限密码；3) 忘记密码无法找回 —— 我们不提供破解；4) 连续错误不会锁定文件 |
| **E1004** | 磁盘空间不足 | 写输出文件或临时文件时目标磁盘/系统临时盘空间不足 | 请清理磁盘后重试 | 1) 清理目标磁盘或换新输出目录；2) 检查系统临时目录所在盘（临时文件在 `payoncepdf-<jobId>/`）；3) 预留约为输入文件 2–3 倍的可用空间；4) 关闭占用磁盘的程序后重试 |
| **E1005** | 页数超限（>5000） | 单文件页数超过 5000 页的上限 | 单文件超过 5000 页，已拒绝 | 1) 先用「拆分」把大文件分批；2) 分卷后再执行目标操作；3) 确认文件是否异常巨大；4) 这是硬性上限，无法通过参数绕过 |
| **E1006** | OCR 语言包缺失 | 请求的 `tessdata` 语言未安装、路径错误或文件损坏；随包 `eng` + `chi_sim`，其余 40 种按需安装 | 请安装中文语言包 | 1) **首选**：左侧「打印与扩展」→ 填入语言代码 → 点「安装语言包」，从 42 种白名单语言中按需安装到用户目录（这是唯一需要联网的可选动作，请求只含语言代码）；2) 「关于 / 网络活动」→ 自检查看 `tessdata` 是否 `ready`；3) 只勾选已安装的语言；4) 若下载被网络阻断，可断网继续使用随包的中英文 OCR，不影响其它功能；5) 仍失败联系支持并附自检截图 |
| **E1007** | 输出被占用 | 目标输出文件/文件夹正被其它程序锁定（如 Adobe Reader、预览、同步盘） | 目标文件正被其他程序使用 | 1) 关闭正在打开该文件的程序；2) 换一个输出文件名或目录；3) 关闭网盘同步后再试；4) 重试即可，原文件不受影响；5) 本应用不会覆盖被占用文件 |
| **E2001** | sidecar 缺失或架构不匹配 | `qpdf` / `tesseract` 三平台二进制缺失、架构不符（arm64 与 x64 混用）、签名后权限/路径错误 | 组件缺失，请重新安装或运行自检 | 1) 重新安装对应平台的完整包；2) 「关于」→ 自检，逐项查看 `sidecars[].ready` 与 `arch`；3) macOS 确认使用通用二进制、Windows 确认 x64；4) 重新执行 sidecar 供给/自检；5) 附自检结果联系支持 |
| **E2002** | PDFium 渲染引擎未加载 | `pdfium.dll` / `libpdfium.dylib` 未找到、加载失败或被安全软件删除 | 渲染组件缺失，请重新安装 | 1) 重新安装完整包；2) 确认 `resources/` 中 PDFium 随包存在；3) 「关于」→ 自检；4) 检查安全软件是否隔离了该库并恢复；5) 涉及 PNG/JPG 导出与渲染的功能会受影响 |
| **E2003** | 用户取消 | 用户在进度条点击「取消」，`cancel_job` 置位 `AtomicBool`，任务在每个文件/每页后检查并退出 | 已取消，原文件未改动 | 1) 无需处理，属预期行为；2) 需要时重新发起任务；3) 该任务的临时目录会被清理；4) 已生成的输出以原子替换方式写入，不会留下半成品覆盖原文件 |
| **E2004** | 内部错误（含参数非法） | 未分类异常、参数校验失败、引擎返回非预期结果 | 操作失败，原文件未改动 | 1) 记录错误码与操作类型（可临时开启本地日志，仅记操作类型与错误码）；2) 用更小/更简单的输入重试以缩小范围；3) 升级到最新 1.x；4) 仍失败请附错误码、版本、OS、复现步骤联系支持 |
| **E2005** | 试用到期，导出被限制 | 导出前 `ensure_export_allowed()` 检测到 `status=expired` | 试用已结束；文件仍在，激活后可导出 | 1) 输入 `PAYONCE-XXXX-XXXX-XXXX` 密钥激活（首次需联网校验一次，之后离线）；2) 激活后立即恢复导出；3) **原文件与已导出文件绝不删除/改写**；4) 未激活前仍可查看与处理，但导出被拦截；5) 若认为试用计算有误，附购买/激活时间联系支持 |

---

## 2. 通用排查顺序（遇到任何错误码）

1. **记下错误码**：界面提示与批量 `report.csv` 的 `error_code` 列（见 `docs/user-manual.md` §4）。
2. **确认原文件安全**：任何错误都不会改写、删除输入文件；输出到指定路径，从不原地覆盖。
3. **检查输出目录**：磁盘空间（E1004）、文件占用（E1007）、路径可写。
4. **组件自检**：「关于 / 网络活动」查看 sidecar 与 tessdata 状态（E2001/E2002/E1006）。
5. **必要时开启本地日志**：默认关闭，开启后仅记录操作类型与错误码，**不上传**；排查完可关闭并删除日志。
6. **仍无法解决**：使用下方模板联系支持，并附错误码、版本、操作系统、复现步骤。
7. **隐私相关**：如需验证「不上传」，按 `site/verify.html` 抓包；错误信息里不含文件内容。

> 措辞约束：对外沟通不写「100% 准确」「永久免费」「lifetime updates」；做不到的事（破解密码、Word/Excel）直接说明不支持。

---

## 3. 支持邮件模板

<!-- 联系邮箱已统一为 hello@payoncepdf.com；上线前需确认 DNS / MX 与收件可达 -->

### 3.1 中文模板

```
收件人：hello@payoncepdf.com
主题：[PayOnce PDF 错误码 E____] 操作 ____ 失败，请求协助

你好，

我在使用 PayOnce PDF 时遇到错误，需要协助排查。

- 错误码：E____（界面提示：「____」）
- 操作类型：合并 / 拆分 / 压缩 / 图片互转 / OCR / 签名 / 批量（勾选其一）
- PayOnce PDF 版本：TODO(版本号)
- 操作系统：Windows 10/11（x64） / macOS 11+（Apple Silicon / Intel）
- 是否已激活 / 试用剩余天数：____
- 复现步骤：
  1. ____
  2. ____
  3. ____
- 输入文件情况（仅描述，不附文件）：页数约 ____、大小约 ____、是否加密（已知密码：是/否）、是否为扫描件
- 组件自检结果（关于 / 网络活动）：qpdf ____ / tesseract ____ / pdfium ____ / tessdata ____
- 已尝试的排查：____（清磁盘 / 换输出目录 / 重装 / 关闭占用程序 等）
- 本地日志（如已开启，仅含操作类型与错误码）：____（可粘贴相关行，已确认不含文件内容）

我了解到 PayOnce PDF 不上传文件，因此不会附任何 PDF 原件；如需要，我可按
site/verify.html 自行抓包并提供脱敏后的片段。

谢谢。
```

### 3.2 English template

```
To: hello@payoncepdf.com
Subject: [PayOnce PDF E____] ____ operation failed — request for help

Hello,

I hit an error in PayOnce PDF and would like help diagnosing it.

- Error code: E____ (on-screen message: "____")
- Operation: merge / split / compress / images / OCR / sign / batch
- PayOnce PDF version: TODO(version)
- OS: Windows 10/11 (x64) / macOS 11+ (Apple Silicon / Intel)
- Activated / trial days left: ____
- Steps to reproduce:
  1. ____
  2. ____
  3. ____
- Input file (description only, no attachment): approx. pages ____, size ____,
  encrypted (known password: yes/no), scanned: yes/no
- Component self-check (About / Network): qpdf ____ / tesseract ____ /
  pdfium ____ / tessdata ____
- Troubleshooting already tried: ____ (free disk space / change output dir /
  reinstall / close the app locking the file)
- Local log (if enabled; operation type + error code only): ____ (confirmed
  to contain no file content)

I understand PayOnce PDF does not upload files, so I am not attaching any PDF. If
needed I can follow site/verify.html to capture traffic and share a redacted
excerpt.

Thank you.
```

---

## 4. 对照与约定

| 来源 | 码段 | 说明 |
|---|---|---|
| `PRD.md` §13 | E1001–E1007 | 基础七码，用户提示文案为规范原文，不改动 |
| `CONTRACT.md` §2 | E2001–E2005 | 扩展码，以 E2xxx 标记；含取消、sidecar、渲染、内部错误、试用导出限制 |

- 扩展码**不改动**基础表；新增码须先改 `CONTRACT.md` 再改代码与本文档。
- `E2003`（取消）与 `E2005`（试用到期）是预期业务状态，不是缺陷。
- **F10–F16 全部未新增错误码**（仍是 7 基础码 + 5 扩展码 = 12 码）。参数非法、页码越界、查找不到、颜色非法、字体缺字形、**页眉/页脚含中文**、系统 CJK 字体缺失、书签页码越界、语言代码不在白名单、语言包下载失败或下载内容过小、未知操作统一走 `E2004`；但 `message` 必须写清具体原因与可操作建议（见 `CONTRACT.md` §10.3 / §11.3）。客服遇到此类工单时按 message 判断，不要笼统回复「内部错误」。
- **F13 中文水印已支持**：只有「页眉 / 页脚含中文」和「找不到系统 CJK 字体」才报 `E2004`；水印文字含中文是正常功能，不应报错。旧口径「叠加文字含中文 → E2004」已作废。
- **F16 语言包下载失败**不属于 sidecar 缺失，不走 `E2001`；现有语言包不会被改动，随包中英文 OCR 仍可用。
- **F16 只读查询命令**（`list_bookmarks` / `list_ocr_languages`）不占导出授权，试用到期（`E2005`）时仍可查询；只有产生输出文件的任务命令才受 `E2005` 限制。
- 客服回复模板（退款、激活、OCR 效果、macOS 拦截等）见 `docs/support-template.md`。
