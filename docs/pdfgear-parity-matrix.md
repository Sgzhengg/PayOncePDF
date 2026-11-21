# PayOnce PDF ↔ PDFgear 功能对标矩阵与范围变更

> **2026-11-20 提价说明**：本文写于 2026-11-16 定价决策当时，文中的 `$29 / ¥169`（早鸟 `$19 / ¥109`）是**历史价格**，论证与数字按原文保留。现行价格为 **$39 / ¥199**（首发 14 天早鸟 **$29 / ¥149**），一把钥匙最多 3 台设备；依据 `docs/updf-parity-decision.md` §1、`config/product.json` 与 PRD §22.10。

> 生成：2026-11-16（GMT+8），**同日第二轮更新**
> 触发：用户决策「PDFgear 能实现的 PayOnce PDF 也应该实现；功能只能比它强不能比它弱」；
> 第二轮指令「定价稍晚再说，高保真 Word/Excel、AI 问答、移动端暂时不要考虑，其它继续」
> 证据：联网检索 PDFgear 官网 / Mac 用户指南 / 官方《Statement on the Organised Spread of Disinformation》/ 第三方 2026 评测；`<repo>` 源码与构建结果
> 口径：**代码写完 ≠ 真机跑通 ≠ 商业验证**。本文对三者分别标注，未验证项一律不写成「已达标」。

---

## 0. 先纠正一个事实前提（重要，会影响定位）

用户判断「PDFgear 也是本地处理，并没有另调云上 API」——**核心编辑功能确实本地，但"完全没有云 API"不成立**。PDFgear 官方 2026-03-24 更新的声明原文：

| 事实 | 官方原文 / 来源 |
|---|---|
| 桌面端合并/拆分/压缩/转换本地 | 「*key features such as PDF conversion, merging, splitting, and compressing are all processed locally on the device, without uploading any data to servers*」 |
| **AI 与部分移动端任务走服务端** | 「*Advanced features such as AI and certain tasks on mobile apps still use secure server-side processing when high computing power or licensed SDKs are required*」 |
| **历史上部分压缩/转换也走过服务端** | 官方声明里专设一节「*Why does PDFgear utilise server-side processing for compression and conversion?*」，并称「*PDFgear is steadily transitioning toward local processing*」 |
| **AI 走 Microsoft Azure OpenAI，文件会离开本机** | 隐私政策经第三方核对：用户明确同意后，传输「您上传的 PDF 档案内容」「您在对话中输入的文字」及有限元数据至 Azure OpenAI Service |
| **"免费"是有期限的商业策略** | 「*In the future, we may add a few paid options for advanced tools, such as AI-powered features, high-accuracy OCR, and complex PDF conversions*」 |
| 使用成熟商业 SDK，非自研 | 「*As a startup, we use well-established SDKs … rather than rebuilding common technical foundations from scratch*」 |

**结论**：用户「功能面对标」的战略判断成立。但「文件内容永不上传」不是双方相同的前提，而是我们**可以真正超过它**的地方（措辞注意：见下，不再用「零云调用 / 零网络请求」这类绝对表述）。

> ⚠️ **第二轮诚实修正**：PayOnce PDF 现在有**一个**运行时联网点——用户主动点击「安装 OCR 语言包」时下载静态字典（详见 §2.3）。因此官网/文档里「任何功能都不联网」的绝对表述必须改成
> 「**任何文件处理功能都不联网；文件与文件名永不上传**」，并写进抓包验证清单。PDFgear 的 AI 是把**文件内容**发给 Azure OpenAI，两者性质完全不同，但「零网络请求」这句话已不再严格成立，不能继续这么写。

---

## 1. 对标矩阵（2026-11-16 第二轮后状态）

图例：`✅ 已有` · `🆕 第一轮新增` · `🆕 第二轮新增` · `⚠️ 部分/降级` · `❌ 缺失` · `⏸ 用户明确暂缓`

### 1.1 阅读、编辑、批注

| 能力 | PDFgear 桌面版 | PayOnce PDF（第二轮后） | 判定 |
|---|---|---|---|
| 就地替换现有文字（不转 Word） | ✅ 免费 | **F10** PDFium `FPDFText_SetText` + 内容流显式提交 + 写后读回校验；支持中文（需嵌入 CJK 字体）、跨文本对象合并、缺字形明确报错 | **补齐，已真机文件级验证** |
| 所见即所得编辑器（光标/段落回流/拖拽对象） | ✅ 类 Word | ❌ 仍不做 | **明确落后**，需单独立项 |
| 编辑图片 / 形状 / 超链接 / 文本框 | ✅ | ❌ | **明确落后** |
| 高亮 / 下划线 / 删除线 / 波浪线 | ✅ | **F11** 标准注解字典 + QuadPoints | 补齐 |
| 便签 / 评论 / 图章 / 绘图 / 自定义形状 | ✅ | ⚠️ 仅「便签备注」一种 | 部分补齐 |
| 书签查看 / 新建 / 重命名 / 删除 | ✅ | **F15** `/Outlines` 重建，顶层书签 set/add/rename/delete + 只读列表 | 补齐（无嵌套层级） |
| 创建可填写表单字段 | ✅ | **F15** AcroForm 文本域（单行/多行/必填，保留既有表单） | **部分补齐**：无复选框/单选/下拉/计算脚本/签名域 |
| 填写表单 / 扁平化 | ✅ | ✅ F6 | 持平 |
| 电子签名（鼠标/键盘/图片） | ✅ | ⚠️ 仅图片叠加 | 部分持平 |
| 密文标记 / 脱敏（redaction） | ✅ | **F15** 真删除文字对象 + 黑框覆盖（复制粘贴也取不到原文） | 补齐，已真机文件级验证 |

### 1.2 转换、OCR、页面

| 能力 | PDFgear | PayOnce PDF（第二轮后） | 判定 |
|---|---|---|---|
| PDF → Word / Excel / PPT（保版式） | ✅ 商业 SDK | ⚠️ **F14** 仅文字层 TXT/HTML/DOCX/XLSX | **明确落后**；用户明确暂缓高保真路线 |
| Word/Excel/PPT/RTF/TXT → PDF | ✅ | **F16** TXT/CSV/HTML → PDF（中文用系统 CJK 字体嵌入） | **部分补齐**：纯文本排版，不还原 Office 版式 |
| 批量转换 | ✅ | ✅ F7（批量 9 种操作） | **我方更强** |
| OCR 30+ 语言 | ✅ 随包 30+ | **F16** 42 种语言清单（`sidecar.rs::AVAILABLE_LANGS` 实数），随包 `eng`+`chi_sim`，其余按需安装到用户目录 | 基本补齐（需用户主动安装一次） |
| OCR → 可搜索 PDF / TXT | ✅ | ✅ F5 | 持平 |
| 合并 / 拆分 / 压缩 | ✅ | ✅ F1/F2/F3 | 持平 |
| 旋转 / 删除 / 抽取 / 插入空白页 / 裁剪 | ✅ | **F12**（除"拖拽重排"外齐全）+ 旋转页文字提取兼容层 | 基本补齐 |
| 水印 / 页眉页脚 / 页码 | ✅ | **F13** + **F16** 中文水印（系统 CJK 字体嵌入） | 补齐；中文水印为水平灰字，不支持旋转/独立透明度 |
| 文档属性编辑 | ✅ | **F13** /Info | 补齐 |
| 密码保护 / 权限位 | ✅ | **F13** qpdf AES-256 | 补齐 |
| 打印 / 打印设置 | ✅ | **F16** 系统打印（Windows `Start-Process -Verb Print` / macOS `lp`） | 补齐（无应用内打印预览） |
| 邮件 / 链接分享 | ✅ | **F16** `mailto:` 预填主题与正文 | 部分补齐（附件需客户端手动添加） |
| 创建空白 PDF / 扫描仪导入 | ✅ | ⚠️ F12 可插入空白页；F16 可从文本新建 | 部分 |

### 1.3 平台、AI、商业模式

| 维度 | PDFgear | PayOnce PDF | 判定 |
|---|---|---|---|
| Windows / macOS | ✅ | ⚠️ Windows 包已产出（未签名）；macOS 未构建/未公证 | 落后 |
| iOS / Android | ✅ | ❌ §19 明确不做 | ⏸ 用户明确暂缓 |
| 网页版（浏览器内本地处理） | ✅ 数十种工具 | ⚠️ 本地 `dist/` 预览为 mock，不处理文件 | 落后（需单独立项） |
| AI 文档问答 / 摘要 / 翻译 | ✅（**云端 Azure OpenAI**） | ❌ 明确不做 | ⏸ 用户明确暂缓；转为隐私卖点 |
| 无账号 / 无广告 / 无水印 | ✅ | ✅ | 持平 |
| 遥测 | 官网称无；隐私政策涉及 Google Analytics / 广告技术 | ✅ 无遥测、日志默认关闭；语言包安装无用户标识 | **我方更强（可审计）** |
| **文件内容是否离开本机** | ❌ AI 上传 PDF 内容至 Azure OpenAI | ✅ **永不上传**；语言包下载请求只含语言代码 | **我方更强（抓包可验证）** |
| 网络请求总数 | 激活/更新/AI/部分服务端处理 | 一次性激活、可选更新检查、**用户主动的语言包下载** | 需如实披露，见 §2.3 |
| 收费确定性 | ⚠️ 官方自承未来可能对 AI / 高精度 OCR / 复杂转换收费 | ✅ **$29 买断**（首发 14 天早鸟 $19）+ 12 个月 1.x 更新边界写死，**2026-11-16 已定价**（PRD §22.8 / `docs/pricing-decision.md`） | **我方更强** |
| 开源 / 可审计源码 | ❌ 闭源 | ❌ 也闭源 | **都不强**；Stirling PDF 在这一点上胜过两家 |
| 批处理 + 结构化失败台账 `report.csv` | ⚠️ XDA 2026 评测明确列「批量处理及进阶自动化仍有加强空间」 | ✅ 单文件失败不中断、带错误码、不覆盖输入 | **我方更强** |

---

## 2. 两轮实际做了什么（可核验）

### 2.1 第一轮：F10–F14

| 项 | 文件 | 状态 |
|---|---|---|
| F10 就地替换文字 | `engine/extras.rs::replace_text`、`commands/extras.rs::replace_text` | 代码完成；**第二轮真机验证发现并修好两个致命 bug** |
| F11 批注（5 种） | `extras.rs::annotate`、`annotate_pdf` | 同上 |
| F12 页面管理（5 种） | `extras.rs::page_ops`、`manage_pages` | 同上 |
| F13 水印/页码/页眉页脚/元数据/AES-256 | `extras.rs::decorate`、`qpdf.rs::encrypt` | 同上 |
| F14 导出 TXT/HTML/DOCX/XLSX | `extras.rs::export_text` + 自写 ZIP/OOXML（**无新增依赖**） | 同上 |
| 批量扩展到 9 种操作 | `commands/jobs.rs::run_batch_one` | 代码完成 |
| 前端 12 个功能入口 | `panels/ExtrasPanels.tsx` 等 | 构建通过 |

### 2.2 第二轮：实证验证 + F15/F16

**A. 真机文件级端到端验证（本轮最高价值）**

把引擎与 `AppHandle` 解耦（`JobCtx.app` 改为 `Option<&AppHandle>`、PDFium 实例可注入），写了 **28 项 Rust 单测**，其中 11 项是真实 PDF 文件 + 真实 PDFium DLL + `qpdf --check` 的端到端测试：

| 测试 | 验证内容 |
|---|---|
| `e2e_edit_ascii_real_pdf` | 改字后重新打开，文本层确为 "Hello PayOnce PDF"；原文件字节未变；qpdf 通过 |
| `e2e_edit_cjk_real_pdf` | 用系统 `simhei.ttf` 生成真嵌入 CJK 字体 PDF，改数字与**改汉字**都成功 |
| `e2e_edit_missing_glyph_fails_closed` | Helvetica 写中文**明确报错且不写半成品** |
| `e2e_annotate_real_pdf` / `..._rotated_page` | 标准 `/Highlight` + `/QuadPoints`，旋转页兼容 |
| `e2e_redact_really_removes_text` | 脱敏后文本层读不到敏感词，黑框内容流存在 |
| `e2e_export_real_pdf_all_formats` | 四种格式从真实文字层导出，DOCX/XLSX 为合法 ZIP |
| `e2e_pipeline_...` | 改字 → 批注 → 旋转 → 导出 全链路 |
| `e2e_text_to_pdf_with_cjk` / `e2e_cjk_watermark_real_pdf` | 中文文本转 PDF / 中文水印真的写进文本层 |
| `form_fields_are_real_acroform_widgets` / `bookmark_...roundtrip` | AcroForm Widget 与 /Outlines 结构 |

**B. 实证抓出并修复的两个真 bug（第一轮「编译通过」掩盖的问题）**

| # | 症状 | 根因 | 修复 |
|---|---|---|---|
| 1 | **F10 改字返回成功，但打开 PDF 文字仍是原文**；中文缺字形保护也没触发 | PDFium 规定保存前必须 `FPDFPage_GenerateContent`，对象级修改否则全部丢失；`FPDFText_SetText` 对缺字形字符会**静默写空格**而非返回错误 | 每页修改后 `page.regenerate_content()`；非 ASCII 替换先查 `font.is_built_in()` / `is_embedded()` 拒绝；**写后读回校验**（重新打开产物，读不到新文字就删除输出并报错） |
| 2 | 旋转 90° 后 F11 找不到文字、F14 导出空白 | 随包 PDFium 在 `/Rotate ≠ 0` 的页面上 `FPDFText_*` 返回空（内容流与字体资源完好） | `with_unrotated()`：查找/提取时生成只把 `/Rotate` 置 0 的临时副本，正式输出仍保留原旋转；改字校验改用文本对象拼接，绕开该 API |

> 这两条证明：**`cargo check` + 纯逻辑单测完全不足以宣称功能可用**。本轮之前文档里「F10 代码完成，真机未验证」的措辞是对的，但「支持中文」属于未经验证的承诺，现已由测试证实或明确拦截。

**C. 新功能**

| 项 | 文件 | 诚实边界 |
|---|---|---|
| F15 表单创建 | `engine/objects.rs::create_form` / `commands/objects.rs::create_form_pdf` | 仅文本域；`/NeedAppearances true` 由阅读器生成外观；不覆盖既有表单 |
| F15 脱敏 | `objects.rs::redact` / `redact_pdf` | 真删除文字对象 + 黑框；只处理可选中文本；扫描件需先 OCR |
| F15 书签 | `objects.rs::{list_bookmarks,manage_bookmarks}` / 2 个命令 | 顶层书签；页面增删后不自动跟随页号 |
| F16 文本转 PDF | `engine/more.rs::text_to_pdf` / `commands/more.rs::text_to_pdf` | TXT/CSV/HTML 纯文本排版；中文用系统 CJK 字体；**不是 Office 高保真转换** |
| F16 中文水印 | `more.rs::cjk_watermark` + `decorate_pdf` 分支 | 水平居中灰字，无旋转/独立透明度；页眉页脚仍限 WinAnsi |
| F16 打印/邮件 | `commands/more.rs::share_file` | 系统默认程序；mailto 附件需手动添加 |
| F16 OCR 语言包 | `engine/sidecar.rs::{list_languages,install_language}` + 2 个命令 | 42 种；白名单校验；先临时文件再替换；见 §2.3 |
| 前端 15 个功能入口 | `panels/MorePanels.tsx`、`lib/{types,params,backend}.ts`、`App.tsx`、`ParamPanel.tsx`、`styles.css` | `npm run build` 61 modules 通过；UI 冒烟 20 PASS / 0 FAIL |

**全部导出类命令复用既有 `execute()` 外壳**：授权校验、取消、临时目录、进度/结束事件、原子提交、错误码、本地日志、`assert_not_overwriting_inputs` —— 没有绕过任何一条既有安全约束。

### 2.3 唯一运行时联网点（必须写进隐私与抓包验证）

`install_ocr_language(lang)` 用已在包内的 `ureq` 下载 `https://raw.githubusercontent.com/tesseract-ocr/tessdata_fast/main/<lang>.traineddata` 到 `app_data_dir/tessdata/`。三条硬约束：

1. 只在用户主动点击「安装语言包」时发生；
2. URL 只含**语言代码**，不含文件名、路径、机器指纹或任何用户数据；
3. 下载的是公开静态字典，不调用云端处理或 AI 接口；识别全程仍在本机。

下载前会把随包 `eng`/`chi_sim` 复制到用户目录，使其成为完整 tessdata；`find_tessdata()` 之后优先使用该目录。语言代码有白名单，超小响应会被丢弃且不改动现有语言包。

**因此官网/隐私页必须把「零网络请求」改为「文件永不上传 + 三类请求可枚举」**：一次性激活、可选更新检查、用户主动的语言包下载。

---

## 3. 「只能比它强不能比它弱」拆成三层

### A. 功能面对标（必须补齐）→ 两轮后剩余缺口
仍有 ❌：WYSIWYG 编辑器、图片/形状/链接/文本框编辑、图章与绘图、复选框/单选/下拉表单域、应用内打印预览、邮件自动附加附件、网页版、macOS 构建与签名。**按字面「不能比它弱」，这些仍需单独立项，不是一个迭代能完成。**

### B. 我们能真正「更强」的维度（可防守）
1. **文件内容永不上传**：对方官方承认 AI 走 Azure OpenAI 且传输 PDF 内容；我们连文件名都不发。
2. **收费确定性**：对方官方写明未来可能对 AI / 高精度 OCR / 复杂转换收费；我们一次买断、更新边界写死（**定价已定 $29 / ¥169，见 PRD §22.8**）。
3. **批处理 + 可审计失败台账**：对方批量与自动化被第三方评测点名；我方 `report.csv` 带 `error_code`、单文件失败不中断、不覆盖输入。
4. **不损坏原文件 + 写后读回校验**：第二轮新增的「改字结果必须能被重新读回，否则删除输出并报错」是对低价编辑器常见的「假成功」的硬防御。
5. **可开源核心**（待决策）：两家都闭源；开源引擎可把隐私承诺从抓包验证升级为源码审计。

### C. 结构性冲突与用户明确暂缓
| 冲突 | 原因 | 状态 |
|---|---|---|
| Word/Excel/PPT 高保真转换 | 需商业 SDK 或 ~200MB LibreOffice（撞 §8 100MB 上限） | **用户明确暂缓**；保持 F14 文字层降级 |
| AI 文档问答 | 云端大模型上传文件 / 本地 LLM 数 GB | **用户明确暂缓**；转为隐私卖点 |
| 移动端 / 网页版 | §19 与六周排期 | **移动端暂缓**；网页版需单独立项 |

### D. 成本结构冲突（**2026-11-16 已决策**，见 PRD §22.8）
PRD §18 定「支持邮件 ≤0.1 封/单」、无客服团队。编辑器是投诉密度最高的功能。原三选一「提价 / 严格收窄边界 / 开源核心+社区支持」当时挂起，**现决议：提价（$19 → $29）+ 严格收窄边界（不做 WYSIWYG、高保真 Office 保持文字层降级）同时执行，不开源**。依据：Gumroad 直连净收入 $29 为 $24.46 vs $10 的 $7.91；支持 0.3 封/单时 $10 被吃掉 47.4%、$29 仅 15.3%。完整单位经济见 `docs/pricing-decision.md` §3。

> 本文其余处出现的 `$19` 是**决策前的历史基线**，不代表当前定价；当前为 **$29 / ¥169**。

---

## 4. 仍待办（按优先级）

| # | 缺口 | 状态 |
|---|---|---|
| 1 | 真机 Tauri 端到端（GUI 内跑 F10–F16、原生对话框、授权） | **未做**；本轮完成的是「引擎级文件级端到端」，不是 Tauri 真机 |
| 2 | Adobe Reader / Word / Excel 打开产物的兼容性 | **未做**（qpdf 结构校验已过，阅读器显示未验证） |
| 3 | WYSIWYG 编辑器、图片/形状/链接编辑、图章绘图 | 未做，需单独立项 |
| 4 | 表单域类型扩展（复选框/单选/下拉） | 未做 |
| 5 | 应用内打印预览、邮件自动附加附件 | 未做 |
| 6 | 中文水印的旋转/透明度 | 未做（已诚实标注为水平灰字） |
| 7 | macOS 构建/公证、Windows Authenticode 签名 | 未做；安装包仍不可对外分发 |
| 8 | 预购 A/B 验证（PRD §1.4 三假设） | 未做；用户明确定价稍后再说 |
| 9 | 官网/隐私/验证页网络口径修订 | **已完成**：6 页全部同步 15 工具与三类请求；标签闭合/断链/1440px 渲染校验通过（`docs/test-report.md` §14 V-14.6） |
| 10 | 重建 Windows NSIS 安装包（含本轮全部新命令） | **已完成**：27.08 MiB + `.sig`，8/8 新命令字符串命中，两段 Ed25519 签名用配置公钥验签通过；见 `docs/test-report.md` §14。**仍未做 Authenticode 签名与真机安装/升级实测** |

---

## 5. 结论

1. **方向认同，已按用户授权放宽 PRD §19 并落地 F10–F16**：从 7 个功能扩到 15 个入口，功能面从「七个批量工具」变成覆盖改字、批注、页面、水印、导出、表单、脱敏、书签、文本转 PDF、打印/邮件、语言包。
2. **第二轮最大收获不是新功能，而是实证推翻了两个「编译通过」的假象**：F10 原本根本没生效，旋转页原本会丢文字层。现在有 11 个真实 PDF 文件级测试锁住这些行为。
3. **「零云调用 / 零网络请求」口径必须修正**：新增语言包下载后，正确的承诺是「文件内容永不上传、所有网络请求可枚举且可抓包验证」。官网、隐私页、验证页、用户手册、FAQ、契约、PRD 已按此口径统一（第三轮同步完成）。
4. **真正能赢的不是"功能更多"，而是"同等功能 + 三条它给不了的硬承诺"**：文件内容不上传、收费确定性、批处理可审计；外加写后读回校验这类工程质量承诺。
5. **当前状态**：代码与引擎级验证完成；**真机 Tauri 端到端、双平台安装包签名、阅读器兼容性、商业验证全部未完成**。发布前必须补 §4 第 1/2/7 项，并把 §1 里所有 ❌ 写进官网「不支持清单」。

---

## 附录：可复现命令

```bash
cd <repo> && npm run build                 # 前端类型与构建
cd <repo>\src-tauri && cargo check --lib  # 后端编译
cargo test --lib -- --test-threads=1           # 28 项单测（含 11 项真实 PDF 端到端）
python tests/acceptance.py --smoke             # 既有验收
node tests/ui-interaction-smoke.mjs            # UI 冒烟（需 dev server + CDP 浏览器）
```

## 附录：引用来源（2026-11-16 检索，需按期复核）

| 材料 | URL |
|---|---|
| PDFgear 官网功能与「The FIRST AI-Powered PDF Software」 | `https://www.pdfgear.com/` |
| PDFgear 官方关于数据处理与免费模式的声明（2026-03-24 更新） | `https://www.pdfgear.com/reddit-disinformation-statement/` |
| PDFgear Mac 用户指南目录（水印/页眉页脚/页码/表单创建/密文/打印/保护） | `https://www.pdfgear.com/zh/mac-user-guide/introduction-to-pdfgear-on-mac.htm` |
| PDFgear 在线编辑器功能边界（桌面版才能改现有文字） | `https://www.pdfgear.com/edit-pdf/` |
| 2026 免费 PDF 编辑器横评（XDA 转述：批量与自动化偏弱） | `https://ezone.hk/article/20116128` |
| 隐私政策核对（Azure OpenAI Service / 显式同意 / Google Analytics） | `https://toolso.ai/tw/tool/pdfgear`、`https://sekin.in/pdfgear-review-a-free-ai-assisted-pdf-editor-with-important-privacy-caveats` |
| PainRadar 痛点语料 | `E:\PainRadar\var\pdf.db`、`<repo>\docs\pain-and-competitive-analysis.md` |
