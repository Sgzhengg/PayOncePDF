# PayOnce PDF 真机测试手册（准备完成，用例未执行）

> 状态：**准备完成 / 用例全部未执行**。本文件与 `tests/out/real-device-results.csv` 配套使用。
> 生成时间：2026-11-16 22:35　项目根目录：`E:\PayOncePDF`
> 前置检查：`python scripts/preflight-real-device.py` → PASS 28 / WARN 2 / **BLOCK 0** / FAIL 0（2026-11-20 配置修复 + F18 后，以每次实跑为准）
>
> **本文件里的用例一条都还没有跑过。** 自动化冒烟（`npm test` / `cargo test` /
> `acceptance.py --smoke` / UI 冒烟）验证的是代码与浏览器预览模式，**不等于真机 GUI 验证**。
> 结果必须人工逐条填写，未填的用例一律视为「未测试」，不得推断为 PASS。

---

## 0. 目标、分工与诚实边界

| 项 | 说明 |
|---|---|
| 本次要证明什么 | 真实 Tauri 窗口里，15 个功能入口能否调用本地 Rust 引擎产出**真实文件**；原生对话框 / 拖放 / 进度 / 取消 / 授权 / 断网 / 阅读器兼容性是否成立 |
| 谁执行 | 真机 GUI 操作由人执行；我负责准备、预检、记录模板、结果回收与缺陷定位修复 |
| 我不能代做 | 点击原生文件对话框、拖放、系统打印对话框、邮件客户端、系统时间修改、抓包、安装/卸载、macOS 实机 |
| 不算真机 | `http://localhost:1420/` 浏览器预览模式。它走 `src/lib/backend.ts` 的 mock 后端，**不调用 Rust/PDFium**，界面能点但产物是假的 |
| 已知不做 | 高保真 Word/Excel、AI 问答、移动端（按用户指示暂缓）；macOS 实机构建与公证 |

---

## 1. 三种启动方式（先选一种）

| 方式 | 命令 / 入口 | 优点 | 限制 |
|---|---|---|---|
| **A. 未打包 release 主程序（推荐先测）** | 双击 `E:\PayOncePDF\src-tauri\target\release\payoncepdf.exe` | 最快；`resources/pdfium.dll` + 已补齐的 `resources/tessdata/` 就位，**OCR 可用**；不写注册表、不装系统 | 不是「安装态」，无法验证安装/卸载/快捷方式（G8） |
| **B. NSIS 安装包（安装态）** | `E:\PayOncePDF\src-tauri\target\release\bundle\nsis\PayOnce-PDF-Setup-1.0.0.exe` | 唯一能验证 G8 安装/卸载、SmartScreen、真实 `resource_dir` | tessdata 已随包（打包配置已修），**仍需安装态真机回归 OCR**；无 Authenticode 签名，SmartScreen 会警告；会改动系统 |
| **C. 浏览器预览** | `http://localhost:1420/` | 只看 UI 布局 | **不算真机**，禁止用它判定任何功能通过 |

> 建议顺序：**先用 A 跑完 G0–G7、G9**（功能面），再决定是否为 G8 走 B。
>
> **2026-11-24 更新（opener 权限加固后）**：B 路径安装包再次重建为 30.25 MiB / 31,719,938 B，SHA-256 `8307c7b57aefedc9d5da18ce25a6523d750066aab95a5cf5ca826a01616d2d92`（`docs/test-report.md` §41），`f1276027…721ae83f03`（§40）、`bd80c265…fc41048e0`（§39）及更早的所有哈希作废。**安装态请优先核对 `keys/public.key` 存在**、再点一次「运行自检」，要求 `summary.total === 32 && summary.failed === 0` 且 `env.license_public_key.ok === true`，然后按 §9 执行人工用例；本轮额外建议验证「打开缴费页」能跳到价格页、而任意其它外链不会应用内打开（opener allow-list）。
> 若先修 BLOCK 缺陷（`tauri.conf.json` → 重建安装包），则 B 可完整覆盖 G8 与 OCR。

**A 的等价命令行**（PowerShell，便于同时设置环境变量）：

```powershell
& "E:\PayOncePDF\src-tauri\target\release\payoncepdf.exe"
```

---

## 2. 开测前一步：跑预检

```bash
cd E:\PayOncePDF
python scripts/preflight-real-device.py
```

脚本**只读**，不删除任何文件、不启动 GUI、不联网，只写出
`tests/out/preflight-real-device.json`。重点看：

- `FAIL` 必须为 0（当前为 0）。
- `BLOCK 2` = 已确认的打包缺陷，见 §6 的 RD-BLOCK-01/02，**不影响 A 路径**。
- `P43 试用状态目录`：当前「不存在」，正好适合测 LT1 首次 7 天试用。若之后变成 `已存在`，
  说明不是首次启动了；**需要干净状态时请自行备份并移走
  `%APPDATA%\app.payoncepdf.desktop`**，我不会代为删除 AppData 下的任何内容。
- `P44` 提示目录里有改名前遗留的旧安装包 `OncePDF_1.0.0_x64-setup.exe`，**仅供对照，不得分发**。

---

## 3. 干净状态、授权 mock 与抓取证据

### 3.1 本地激活服务（仅 LT3/LT4/LT6 需要）

`src-tauri/src/license.rs` 的默认激活地址是 `https://payoncepdf.com/api/activate`，
联调时必须用环境变量覆盖，**且必须在启动 app 的同一个 shell 里设置**：

```powershell
# 终端 1：启动 mock
cd E:\PayOncePDF
node scripts/license-server.mjs          # http://127.0.0.1:8787/api/activate

# 终端 2：带着环境变量启动 app
$env:PAYONCEPDF_ACTIVATE_URL = "http://127.0.0.1:8787/api/activate"
& "E:\PayOncePDF\src-tauri\target\release\payoncepdf.exe"
```

- 测试密钥：`PAYONCE-ABCD-EFGH-IJKL`（22 字符，旧 `ONCE-` 前缀应被拒绝）。
- 公钥：`src-tauri/keys/public.key`（开发密钥，非生产）。
- 生产必须替换为真实校验服务，见 `src-tauri/keys/README.md`。

### 3.2 每次失败要留的证据

无论 PASS/FAIL，都请把以下东西放进 `tests/out/rd-evidence/<case_id>/`：

1. 输入文件原始 SHA-256（`sha256sum <file>`），以及处理后的 SHA-256（验证「原文件不动」）；
2. 输出文件本身（或截图 + 大小）；
3. 界面状态栏文字 / 错误码（如 `E1001`、`E2004`、`E2005`）；
4. 复现步骤（哪几个文件、参数面板每一项的值）；
5. 若开启日志：`%APPDATA%\app.payoncepdf.desktop\payoncepdf.log`（默认关闭，只记时间/操作/错误码）；
6. 平台 / 构建号 / 执行时间，填进 CSV 的 `executed_at` 与 `notes`。

---

## 4. 用例清单

优先级：**P0 = 必测（核心承诺）**、P1 = 应测、P2 = 有余力再测。
`tests/out/real-device-results.csv` 已预填全部 case_id / group / priority / title / fixture / expected，
只需填 `result`、`actual`、`error_code`、`evidence`、`executed_at`、`notes`。

### G0　启动与自检（P0）

| ID | 用例 | 预期 |
|---|---|---|
| RD-G0-01 | 首次启动 A 路径 | 窗口标题 `PayOnce PDF`；侧栏 `PayOnce PDF / 一次付费 PDF`；**15 个**工具入口；首屏 "Your files never leave this computer." |
| RD-G0-02 | 「关于」对话框 | `1.0.0`、platform `windows`、arch `x86_64`、`preview=false`、`telemetry=false`（**不得是 `preview / browser / n/a`**） |
| RD-G0-03 | 组件自检（关于 / 网络活动） | `qpdf` / `tesseract` / `pdfium` / `tessdata` 四项 `ready=true`；已装语言至少 `eng` + `chi_sim` |
| RD-G0-04 | 授权面板 | `status=trial`、`trialDaysLeft=7`、`canExport=true`；全程无登录/注册/邮箱/购买弹窗 |
| RD-G0-05 | 网络活动面板 | 三类请求口径（激活 / 可选更新检查 / 主动装语言包）；无遥测声明 |
| RD-G0-06 | B 路径安装后重复 G0-01~03 | 安装态同样达标（**注意 §6 BLOCK**）；SmartScreen 警告属预期，记入 `notes` |
| RD-G0-07 | F17 内置阅读视图渲染真实 PDF | 画布显示真实页面；页码 / 纸张尺寸 / DPI 正确；不落盘不联网 |
| RD-G0-08 | F17 翻页 / 缩放 / 密码 / 用当前页 | 翻页缩放刷新图像；加密 PDF 提示密码；「用当前页」写入右侧参数面板 |
| RD-G0-09 | F18 框选高亮与文字层坐标 | 拖选一段文字后命中字符高亮与页面位置对齐；选中文字与 PDF 内文字一致；不落盘不联网 |
| RD-G0-10 | F18 唯一性守卫与三个动作联动 | 唯一命中时可填入改字 / 批注 / 脱敏参数且页码正确；同页重复文字被拦截并可「扩到整行」；执行后结果 PDF 自动进入预览 |
| RD-G0-11 | F18 禁用场景与结果预览 | 旋转页 / 无文字层页 / 超长页 / 浏览器预览均禁用并给出原因；结果预览态禁用框选且可「返回原文件」 |
| RD-G0-12 | F19 按矩形精确替换只改一处 | 同页两处相同文字，框选第一处 → 填新字 →「直接改这一处」：只有这一处变，另一处保持原样；输出同目录 `*_edited.pdf`，原文件字节不变 |
| RD-G0-13 | F19 失败即拒绝 | 旋转页 / 跨行框选 / 框到空白：均给出明确原因且**不产生输出文件**；「扩到整行」后替换范围不超出该行 |
| RD-G0-14 | 中文 PDF 渲染与改字（真实内嵌字体） | `tests/fixtures/text_cjk_real.pdf`：三行中文清晰无乱码；拖选第三行「30 日」改成「45 日」→ 只有这一处变化，另两行原样；结果 `*_edited.pdf` 在预览中同样清晰 |
| RD-G0-15 | 英文与数字改字 | `tests/fixtures/text_symbols.pdf` 第 1 页：把「Payment due in 30 days」的 30 改成 45 → 只有这一处变，同页「Invoice No. 30」保持 30；英文不出现方块或空格 |
| RD-G0-16 | 常见符号与数学符号改字 | 第 2 页 × 改 ÷ → 显示「3 ÷ 4」，± ° µ 不变（该页用内置 Helvetica，也要成功）；第 3 页 3.14 改 2.72 →「3 ≈ 2.72」，≤ ≥ ∑ ∫ √π Ω 原样无豆腐块 |
| RD-G0-17 | 上下标与公式的拒绝与单独改 | 第 4 页整段框选 H₂O → 明确拒绝（提示选区有大小不同的字）且不产出文件；单独框上标 2 改 3 → 成功；第 5 页跨行拖选分子分母 → 拒绝 |
| RD-G0-18 | 同一文档连续改多处 | 第 1 页先改「Invoice No. 30」→60，再直接框选「Payment due in 30 days」→45：不重新导入也能继续改，两处改动累积在同一个 `text_symbols_edited.pdf`；预览自动刷新且停在原页码；原始文件字节不变；第二次改完不应弹出「只能改一次」或坐标错位提示 |
| RD-G0-19 | 改完一处后仍可继续框选 | 改完「Invoice No. 30」→60 后，页面仍显示十字光标、可继续拖选并高亮；「改成」输入框已清空；结果预览里不出现「填入改字/批注/脱敏」；在第 2 页改动后仍停在第 2 页，不弹回第 1 页 |
| RD-G0-20 | 布局可收缩与画布占满 | 导入 PDF 后底部文件区应自动收起、画布变高；顶栏「工具」「参数」可收起并让画布横向占满；开关状态重启后保持；浅色/深色下都清晰；收起后不应出现列错位（工作区跑到第一列） |

### G1　F1–F7 基础七工具（P0）

| ID | 用例 | 素材 / 预期 |
|---|---|---|
| RD-G1-01 | 合并 F1 | `page_1.pdf` + `page_100.pdf` + `page_500.pdf` → **601 页**，`PAYONCEPDF_PAGE_NNNN` 顺序连续 |
| RD-G1-02 | 拆分 F2 | `page_100.pdf` 每 10 页拆 → 10 个文件、各 10 页，页序不乱 |
| RD-G1-03 | 压缩 F3 | `scan_big.pdf`（约 20MB）→ ≤5MB，文字层仍可选中 |
| RD-G1-04 | PDF→图片 F4 | `page_1.pdf` 300DPI → PNG，目视清晰；10 页 ≤30s（M3） |
| RD-G1-05 | 图片→PDF F4 | `images/page_01~03.png` → 3 页，顺序 01→02→03 |
| RD-G1-06 | OCR F5 | `scan_color.pdf` 用 `eng`、`chi_sim` 各跑一次；输出文字层非空，中文不成方框；随包两种语言**无需联网** |
| RD-G1-07 | 签名 / 表单 F6 | `page_1.pdf` 加图章，产物在阅读器可见 |
| RD-G1-08 | 批量 F7 | 文件夹含 `corrupted.pdf` + 正常 PDF → `report.csv` 表头 `filename,elapsed_ms,result,error_code,output`，坏文件记错误码且**队列不中断** |

### G2　F10–F14 编辑与导出（P0，本轮对标 PDFgear 的核心）

| ID | 用例 | 素材 / 预期 |
|---|---|---|
| RD-G2-01 | 就地改字（ASCII） | `page_1.pdf` 查找 `PAYONCEPDF_PAGE_0001` → 替换为自定义串；用阅读器读回确认**真的改了**（历史 bug：曾漏 `regenerate_content()`） |
| RD-G2-02 | 就地改字（中文 / 缺字形） | 换成原字体不含的新字形 → 明确报错 `E2004`，**绝不写 `?`、空格或方框** |
| RD-G2-03 | 改字不破坏原文件 | 处理前后 `sha256` 完全一致；输出另存新文件 |
| RD-G2-04 | 批注 F11 | 高亮 / 下划线 / 删除线 / 波浪线 / 便签各一次；Adobe Reader 或 PDFgear 里注解**可编辑可删除**（不是烫死的图片） |
| RD-G2-05 | 旋转页上的批注与导出 | 先旋转 90° 再批注 / 导出文字层，结果非空（历史 bug：`FPDFText_*` 在 `/Rotate != 0` 返回空） |
| RD-G2-06 | 页面管理 F12 | `rotate` / `delete` / `extract` / `insert` / `crop` 各一次；`crop` 只改 `/CropBox`、内容仍可改回 |
| RD-G2-07 | 水印 / 页码 / 属性 F13 | ASCII 水印角度与不透明度生效；**中文水印**（系统 CJK 字体嵌入）目视是中文不是方框；页码与标题写入属性 |
| RD-G2-08 | 中文页眉 / 页脚边界 | 输入中文 → 明确报 `E2004`，不写乱码（这是设计边界，**不是缺陷**） |
| RD-G2-09 | AES-256 加密 F13 | 设密码后错密码打不开、正确密码可开；打印/复制权限位生效 |
| RD-G2-10 | 导出文字层 F14 | TXT / HTML / DOCX / XLSX 各导出一次，分别用记事本 / 浏览器 / Word / Excel 打开；只含文字层，**不还原版式** |

### G3　F15 文档对象（P0）

| ID | 用例 | 素材 / 预期 |
|---|---|---|
| RD-G3-01 | 创建可填写表单 | `page_1.pdf` 按 `1\|buyer\|72\|700\|200\|24\|ACME\|0\|1` 建文本域；Adobe Reader 里可点选输入 |
| RD-G3-02 | 真脱敏 redaction | 脱敏后**复制粘贴也取不到原文**（与只盖黑框的假脱敏区分）；消息里报告删除词数/命中处数/黑框数 |
| RD-G3-03 | 书签管理 | 读取 → `set` / `add` / `rename` / `delete` 四种动作各一次，标题支持中文 |
| RD-G3-04 | 多级书签与边界 | spec 行首 `>` 建子级、`>>` 建孙级；set/rename/delete 后层级不压平；列表按层级缩进；层级跳跃报错；不支持拖放换父级、页面增删后不自动跟随页号（设计边界，非缺陷） |

### G4　F16 打印与扩展（P1）

| ID | 用例 | 素材 / 预期 |
|---|---|---|
| RD-G4-01 | 文本转 PDF（中文） | 含中文的 TXT → A4 竖版、45 行分页、中文不方框 |
| RD-G4-02 | CSV / HTML → PDF | HTML 只剥标签取文字，不执行脚本、不渲染 CSS；输出 `outputs` 是新 PDF |
| RD-G4-03 | 系统打印 | 唤起默认 PDF 阅读器的打印对话框；命令 `outputs` 为空数组 + 说明文字 |
| RD-G4-04 | 唤起邮件客户端 | 生成 `mailto:` 并唤起客户端；**附件需在客户端手动添加**（设计边界） |
| RD-G4-05 | OCR 可选语言包 | 面板列出 **42 种**；联网装 `jpn` 后 `installed=true`，断网点击应明确失败且**不破坏已有语言包** |

### G5　原生交互（P0）

| ID | 用例 | 素材 / 预期 |
|---|---|---|
| RD-G5-01 | 原生文件选择对话框 | 点「选择文件」能打开系统对话框并读入真实文件（预览模式做不到） |
| RD-G5-02 | 拖放 | 从资源管理器拖入多个 PDF，列表顺序、页数、大小显示正确，可排序/删除 |
| RD-G5-03 | 长任务进度与取消 | `page_5000.pdf` 跑长任务 → 进度条推进；点取消返回 `E2003`；原文件不变 |
| RD-G5-04 | 打开输出文件夹 | 点按钮能在资源管理器打开输出目录 |
| RD-G5-05 | 错误提示与错误码 | `corrupted.pdf` → 明确中文错误 + 错误码；`encrypted.pdf` 不给密码 → 明确拒绝；把实际码填进 CSV |

### G6　授权与试用（P1，逐条对应 `tests/license_test.md`）

| ID | 用例 | 要点 |
|---|---|---|
| RD-G6-01 | LT1 7 天全功能试用 | 首次启动、无邮箱、7 天、`canExport=true` |
| RD-G6-02 | LT2 断网可用 | 物理断网 / 防火墙阻出站，15 工具仍可用（激活与主动装语言包除外） |
| RD-G6-03 | LT3 首次联网激活 | mock 服务 + `PAYONCE-ABCD-EFGH-IJKL` → `active`；`BAD-KEY` / 旧 `ONCE-` 前缀被拒（`E2004`） |
| RD-G6-04 | LT4 离线缓存签名 | 断网冷启动仍 `active`；反复 3 次状态一致；无激活/更新请求 |
| RD-G6-05 | LT5 到期只限导出、不删文件 | 隔离虚拟机改系统时间到第 8 天 → `E2005`；用户文件 SHA-256 前后一致；激活后恢复 |
| RD-G6-06 | LT6 更新检查可关闭 | 关闭后抓包零更新请求；开启后可检测/升级/失败回滚（需真实端点，当前为占位 → 记 SKIP） |

> RD-G6-05 需改系统时间，**建议在隔离虚拟机做**，不要在当前工作机直接改。

### G7　隐私与网络（P0）

| ID | 用例 | 要点 |
|---|---|---|
| RD-G7-01 | 断网全功能 | 15 个工具逐个在断网下跑通；仅激活与主动装语言包允许失败 |
| RD-G7-02 | 抓包验证「不上传」 | Wireshark / mitmproxy：只出现三类可枚举请求；**请求体不含文件内容、文件名、路径、机器指纹**；验证口径见 `site/verify.html` §6 |
| RD-G7-03 | 日志默认关闭 | 开启后 `payoncepdf.log` 只含 时间 / 操作类型 / 错误码，不含文件名或路径 |

### G8　安装、卸载与性能（P1，仅 B 路径）

| ID | 用例 | 要点 |
|---|---|---|
| RD-G8-01 | 安装 | 安装目录、开始菜单、快捷方式、应用名均为 `PayOnce PDF`；记录实际安装路径 |
| RD-G8-02 | 卸载 | 卸载后用户文件仍在；激活缓存处理符合手册 §8.3 |
| RD-G8-03 | 冷启动 ≤3s | 从双击到可操作计时（PRD §8），记录 3 次取中位数 |
| RD-G8-04 | 体积 / 内存 / 无障碍 | 实测安装包体积、空闲内存、峰值内存；Tab 键可达性 |
| RD-G8-05 | SmartScreen 警告 | **预期会出现**（无 Authenticode/EV 签名），记录截图，不作为功能 FAIL |

### G9　产物兼容性（P1）

| ID | 用例 | 要点 |
|---|---|---|
| RD-G9-01 | Adobe Reader | 打开批注 / 表单 / 加密 / 脱敏 / 中文水印产物，全部正常 |
| RD-G9-02 | PDFgear | 同上；批注可再编辑 |
| RD-G9-03 | Word / Excel | 打开 F14 导出的 DOCX / XLSX，不报「文件已损坏」 |
| RD-G9-04 | macOS 预览 / `lp` / `open` | 本机 Windows，**本次 SKIP**；须在 macOS 实机执行，不得推断 |

### BLOCK 专项（P0，先复现再决定修不修）

| ID | 用例 | 要点 |
|---|---|---|
| RD-BLOCK-01 | **修复后**验证安装包 OCR 资源 | `bundle.resources` 已显式纳入 `resources/tessdata/*`，NSIS 已重建并重新验签；B 路径安装后组件自检 `tessdata` **预期 `ready=true`**、已装语言至少 `eng` + `chi_sim`，跑 OCR 应成功。若仍为 `ready=false`，记 FAIL 并附 `installer.nsi` 证据 |
| RD-BLOCK-02 | 回归记录与哈希核对 | 记录本次安装包 SHA-256（见 `docs/test-report.md` §21）与 `.sig` 验签结果；**任何重建都会改变哈希，必须重新签名与验签** |

---

## 5. 结果记录规范

- 文件：`tests/out/real-device-results.csv`（已预填所有用例 ID）。
- `result` 只允许 `PASS` / `FAIL` / `SKIP`；**SKIP 必须写原因**到 `notes`。
- `error_code` 填实际观察到的码（如 `E1001` / `E2003` / `E2004` / `E2005`），没有就留空。
- `evidence` 填 `tests/out/rd-evidence/<case_id>/` 下的文件名或截图名。
- 任何 FAIL 都请把 CSV 那一行 + 证据目录发回给我，我会定位到具体命令 / 引擎函数并修复，再补一条回归用例。
- **未填写的行一律视为「未测试」**；我会据此更新 `docs/test-report.md`，不会把空白推断成 PASS。

---

## 6. 已知边界与预期行为（不要当成 bug 报）

1. 中文**页眉/页脚**报 `E2004`（WinAnsi 字体限制）；中文**水印**与文本转 PDF 支持中文。
2. 改文字**不重排不回流**，文字变长可能压到右侧内容，需阅读器目视确认。
3. F14 只导出文字层，不还原版式/表格/图片；F16 文本转 PDF 是纯文本排版，不是 Office 高保真转换。
4. `mailto:` 不自动附附件；系统打印对话框由默认阅读器显示，app 内无打印预览。
5. 42 种语言包中仅 `eng` + `chi_sim` 随包，其余需**用户主动点击**才联网下载（全产品唯一运行时联网点）。
6. 安装包无 Authenticode/EV 签名 → SmartScreen 必然警告；更新端点是占位、minisign 是开发密钥。
7. **BLOCK（配置已修，待重建后回归）**：`bundle.resources` 已于 2026-11-20 改为 `["resources/*", "resources/tessdata/*"]` 并重建 NSIS；但**安装后的 B 路径仍未经真机验证**，`RD-BLOCK-01` / `RD-BLOCK-02` 必须照常执行，不得因配置已改而跳过。
   A 路径已手工补齐 `target/release/resources/tessdata/`，可正常 OCR。
8. macOS 产物未在 macOS 实机构建、签名或公证；`PayOnce PDF.app` / `.dmg` 命名仅由 CI 配置保证。
9. 截图 `tests/out/payoncepdf-*.png` 来自浏览器预览模式（`1.0.0 (preview)`），**不是真机窗口**，不能作为 G0 的证据。
10. 目录里旧品牌安装包仅供对照，不得分发；当前安装包也**不可对外分发**。

---

## 7. 现存自动化与真机的分工（别混为一谈）

| 已通过（自动化） | 尚未验证（真机） |
|---|---|
| `npm test` 59/59、`npm run build`、`cargo check`、`cargo test --lib` | 真机 Tauri GUI 端到端 |
| **应用内功能自检 F21**（环境 / 逻辑 / 真实引擎三层，随报告给出「仍需真机」清单） | 见 §9：自检 PASS 只减重复跑通，不替代人工用例 |
| `acceptance.py --smoke` 9 PASS / 14 `SKIP (manual)` | 原生对话框 / 拖放 / 进度事件 / 授权状态 |
| UI 交互冒烟 20/20（浏览器预览 + mock 后端） | F10–F16 在真实 app 内产出真实文件 |
| 两段 Ed25519 更新签名验签 + 负向自检 | 阅读器兼容性、断网抓包、安装/卸载、性能 |
| 真实 PDF 文件级 Rust 单测（旋转页 / 缺字形 / 脱敏） | macOS 分支与实机、Authenticode / 公证 |

`acceptance.py` 的 M1–M14 与 `tests/license_test.md` 的 LT1–LT6 是这些真机用例的**原始编号**，
本手册的 RD-* 是执行编号，两者映射见 §4 各组标题。

---

## 8. 开测后交回什么

把以下任一形式发回即可，我会接着做缺陷定位与回归：

1. 填好的 `tests/out/real-device-results.csv`；
2. 或按组口述（例如「G2 全 PASS，RD-G2-02 报 FAIL，错误码 E2004 但写了方框」）；
3. 加上 `tests/out/rd-evidence/` 下的截图 / 产物 / 日志。

我会同步更新 `docs/test-report.md` §16，从未执行 → 逐条记录 PASS/FAIL/SKIP，
并只把**有证据**的项写成 PASS。

---

## 9. 自检先行：把真机工作量压到「只能人工验」的部分（2026-11-24，F21）

参考 Grabber 的 `设置 → 运行自检`（逐条 `{case, ok, ms, detail}`、报告写 `selftest-*.json`、前端一键复制结果），
PayOnce PDF 增加应用内功能自检：**侧栏「信息 → 运行自检」**。目的是先把引擎层缺陷在真机 GUI 之外一次性暴露，
避免带着明知有问题的包跑完一整轮人工用例再返工。

### 9.1 自检能证明什么

| 层 | 用例 | 真机用例中可被「功能跑通」维度覆盖的项 |
|---|---|---|
| 环境 `env.*` | 报告目录可写、qpdf / pdfium / tesseract / tessdata 就绪、授权状态、界面与引擎版本同源 | RD-G0-02、RD-G0-03、RD-G0-04 的状态读取 |
| 纯逻辑 `logic.*` | 页码范围、多级书签层级、diff、0/90/180/270 旋转坐标往返、参数解析、原文件保护、错误码 | RD-G3-04 层级规则、RD-G2-03 原文件保护、参数边界类回归 |
| 引擎 `engine.*` | 内置样本 PDF 真跑：qpdf 页数/合并/切页/加密解密、PDFium 文字层/渲染、就地改字、页面删除、书签写回、文档比对、图片/文本转 PDF、压缩、原子提交 | RD-G1-01/02/05、RD-G2-01/06、RD-G3-03/04、RD-G4-01、F20 的**链路是否通** |
| 可选 `ocr.*` | tesseract + tessdata 都在时真跑一次 OCR；缺组件标「跳过」而非「通过」 | 只能证明 OCR 引擎可调起，**不证明**识别准确率 |

自检不联网、不占导出授权、不读用户文件；中间文件写在系统临时目录并清理，报告只写下载目录
`selftest-<时间戳>.json`（含 `schema` / `env` / `summary` / `cases` / `coverage` / `path`）。
`coverage.needs_real_device` 与面板「仍需真机验证」清单同源，共 12 项。

### 9.2 执行顺序（建议固定下来）

> 2026-11-24 本机实测：debug 真窗口无人值守自检 **31/31 通过、failed=0、耗时 4128ms**，并顺带定位修复了 OCR 语言包路径缺陷（见 `docs/test-report.md` §38）。这是**开发构建**上的结果，安装态与前端面板点击仍需按下面第 5 条人工确认。

1. 拿到包后**先跑一次自检**：`summary.failed === 0` 才进入人工用例；有 FAIL 先按 `case + detail + 错误码` 定位引擎/依赖，修完重跑，不把坏包带上真机。
2. 自检 PASS 的「功能跑通」维度，真机**抽验代表性用例 + 目视产物**即可（例如合并只看一次页数与顺序、改字只看一次结果 PDF 的文字与版式），不必 15 个功能逐个重复整条手工流程。
3. `skipped > 0`（多为 OCR 组件缺失）时，对应功能**必须**保留真机用例，不能按自检通过跳过。
4. **CSV 的 PASS 仍需人工填写**：自检结果不能自动写进 `tests/out/real-device-results.csv`；建议把 `selftest-*.json` 复制到 `tests/out/rd-evidence/<case_id>/` 作为辅助证据，但人工目视/交互判据照旧。
5. 下列组**一律照旧全跑**，自检不覆盖：`G5` 原生交互、`G6` 授权与改系统时间、`G7` 断网抓包、`G8` 安装卸载与性能、`G9` 第三方阅读器兼容性、`G0-09~G0-20` 鼠标/布局交互。

### 9.3 诚实边界

- 自检 PASS ≠ 真机 PASS。它证明的是「本机这些引擎函数能被真实调用并产出合法文件」，
  不能证明安装包签名、系统对话框、拖放、高 DPI、视觉质量、性能、第三方软件兼容性。
- 预览模式（浏览器 + mock 后端）自检不可用，面板明确拒绝并禁用按钮，不产出假报告。
- 自检样本是内置生成的 Helvetica 文本 PDF，覆盖不到中文内嵌字体、缺字形、扫描件、加密真实文档；
  这些仍是 `RD-G0-14~17` / `RD-G1-06` / `RD-G2-02` 真机用例的职责。
