# PayOnce PDF 测试报告（PRD §14）

> 报告日期：2026-11-16（Windows 11 x64 开发机，本次执行实测）
> **诚实声明**：本报告只把「本机真实跑过并留下证据」的项目标为通过。凡需要真机安装包、双平台、签名/公证、抓包或标准 OCR 测试集的项目一律标注「待测 / 待外部条件」，**不得据此对外宣称测试通过**。
> **当前状态以文末为准**：§1–§11 是首轮（F1–F9、7 入口）历史记录；§12 是第一轮对标（F10–F14、12 入口），其「功能可用」结论已被 §13 实证推翻；**§13 是第二轮真实 PDF 文件级验证 + F15/F16（15 入口），§14 是全量验证与安装包重建**。阅读当前能力请直接看 §13–§14 与 `docs/pdfgear-parity-matrix.md`。

状态取值：`通过`（本机实测）/ `失败` / `待测`（需真实 app 或真机）/ `待外部条件`（需证书、账号、macOS 硬件）。

---

## 1. 本次实测结果汇总

| # | 验证项 | 命令 / 方法 | 结果 |
|---|---|---|---|
| V-01 | 前端类型检查与生产构建 | `npm run build`（`tsc --noEmit` 严格模式 + Vite 7） | **通过**（0 错误；产物 `dist/`，含 F9 新增代码） |
| V-02 | Rust 全量编译检查 | `cargo check`（Tauri 2.12 / 16 个 `.rs` 模块） | **通过**（`Finished dev profile`，0 error / 0 warning） |
| V-03 | 三平台 sidecar 供给（Windows 路） | `node scripts/provision-sidecars.mjs --target x86_64-pc-windows-msvc` | **通过**（5 个文件，sha256 + PE Machine=0x8664 校验；`sidecar-manifest.json`） |
| V-04 | sidecar 实际可运行 | `qpdf --version` / `tesseract --version` / tessdata eng+chi_sim / PDFium `pdfium.dll` | **通过**（qpdf 11.9.1、tesseract 5.5.0、7.5MB PDFium） |
| V-05 | fixtures 生成 | `python scripts/generate-fixtures.py --huge --big` | **通过**（14 个样本，含 5000 页 / 20MB 扫描件 / 截断损坏件 / 真实加密件） |
| V-06 | PRD §14 验收 smoke | `python tests/acceptance.py --smoke`（真实 qpdf 11.9.1） | **通过 9 / 失败 0 / SKIP(manual) 14** |
| V-07 | 授权密钥与离线验签 | `gen-license-keys.mjs` + `license-server.mjs` + Node crypto 验签 | **通过**（32B 公钥 / 64B 签名，`SIGNATURE_OK`，payload 仅含 keyHash、fingerprint、时间） |
| V-08 | UI 布局（PRD §16） | Edge 无头截图 `tests/out/ui-preview.png`（浏览器预览模式） | **通过**（左七功能、中拖拽+列表、右参数、底进度、标语与预览横幅齐备） |
| V-09 | 官网页面与站内链接 | Edge 截图 + 全量 `href` 存在性检查 | **通过**（无断链、无外部 CDN、无追踪脚本） |
| V-10 | Windows NSIS 安装包 + 更新签名（**首轮历史产物，已被 §14 替代**） | `npx tauri build --bundles nsis`（带开发签名密钥） | **通过**（当时 `PayOnce-PDF-Setup-1.0.0.exe` 26.98 MiB + `.sig` 436B；release 1m39s；日志 `tests/out/build.log`）。该包不含 F10–F16；当前产物见 §14。**安装包本身仍无 EV/Authenticode 签名，不得对外分发** |

V-06 的 9 个通过项：`A1` fixtures 页数/顺序、`A2` qpdf 真实合并 `101 = 100 + 1`、`A3` 500/10→50 推导、`A4` qpdf 真实拆分 50 文件合计 500 页、`A5` `report.csv` 字段、`A6` 12 个错误码枚举、`A7a/b/c` 验证页/错误码表/测试报告存在性。

---

## 2. 构建记录（PRD §15 M7）

| 平台 | 目标 triple | 状态 | 说明 |
|---|---|---|---|
| Windows x64 | `x86_64-pc-windows-msvc` | **已产出（UNSIGNED Authenticode）** | 首轮 26.98 MiB（**现为 §14 的 27.08 MiB**）；更新包签名 `.sig` 已用**开发密钥**生成（私钥仅存于 gitignored `.toolchain/`）。无 EV/Azure 证书，仅内部验证用 |
| macOS arm64 | `aarch64-apple-darwin` | **待外部条件** | CI `macos-14` 矩阵已配置，本机为 Windows 无法编译/公证 |
| macOS x64 | `x86_64-apple-darwin` | **待外部条件** | CI `macos-13` 矩阵已配置 |
| universal binary | arm64 + x86_64 | **待外部条件** | CI `universal` job 用 `lipo` 合成主可执行文件；sidecar/动态库仍单架构（已在 workflow 注明） |

> 首次 release 构建曾在两个 rustc 进程上挂起 15 分钟无产物增长；已终止并改用 `CARGO_BUILD_JOBS=1` 串行重试，修复记录见 `tests/out/build.log`。此问题只影响本机构建速度，不影响源码正确性。

---

## 3. 测试环境

| 项 | 计划（PRD §2.3 / §15） | 当前状态 |
|---|---|---|
| Windows 10/11 x64 | 本机 | **已建立**（Windows 11 x64，Node 22.22 / Rust stable / VS 2022 / WebView2 157） |
| macOS 11+ arm64 / x64 | 真机或 CI | **待外部条件**（CI 矩阵已写好，未执行） |
| CI 矩阵 | `windows-latest` + `macos-14` + `macos-13` | 配置就绪（`.github/workflows/build.yml`），secrets 未配置 |
| 签名产物 | Windows EV/Azure + macOS Developer ID/公证 | **待外部条件**（18 个 secrets 清单见 workflow 末尾） |

---

## 4. Fixtures 清单（PRD §14.1）

| # | Fixture | 实际文件 | 用途 | 状态 |
|---|---|---|---|---|
| F-01 | 1 页 PDF | `tests/fixtures/page_1.pdf` | 基础合并/拆分/转换 | **已生成** |
| F-02 | 100 页 PDF | `page_100.pdf` | 拆分、页数校验 | **已生成** |
| F-03 | 500 页 / 5000 页 | `page_500.pdf` / `page_5000.pdf` | 拆分、`E1005` 边界 | **已生成** |
| F-04 | 加密 PDF（已知密码 `payoncepdf`） | `encrypted.pdf`（qpdf 11.9.1 生成，AES-256） | `E1002` / `E1003` | **已生成** |
| F-05 | 扫描件 300DPI 含表格 | `scan_gray.pdf` / `scan_color.pdf` | OCR、压缩画质对比 | **已生成**（合成图像，非真实扫描） |
| F-06 | 中文/特殊符号 PDF | `text_cjk_special.pdf` | 字体兼容 | **已生成**（未嵌 CJK 字体，可能显示方框） |
| F-07 | 损坏文件（截断） | `corrupted.pdf` | `E1001` 失败恢复 | **已生成** |
| F-08 | 超大单页图片 | `scan_big.pdf`（约 20MB） | 大文件稳定性；50MB 需调大参数 | **部分**（20MB，非 50MB） |
| F-09 | 100 文件合计 500MB | — | F1 性能验收 | **待测**（未生成，避免占用磁盘） |
| F-10 | 20MB 扫描件 | `scan_big.pdf` | F3 压缩到 ≤5MB | **已生成** |
| F-11 | 10 页清晰扫描 + 标准测试集 | — | OCR 准确率英 ≥95% / 中 ≥90% | **待测**（需真实扫描标注集） |
| F-12 | 200 个文件的文件夹 | — | F7 批量与 `report.csv` | **待测**（脚本可按目录批量生成） |
| F-13 | 文字层/书签断言样本 | `page_*.pdf`（含 `PAYONCEPDF_PAGE_NNNN` 标记） | 压缩/OCR 不破坏文字层 | **部分**（页序标记已就绪，书签样本待测） |
| F-14 | Canary 文件（唯一字符串） | — | 抓包验证无文件外发 | **待测**（步骤见 `site/verify.html`） |
| — | 图片序列 | `images/page_01..03.png` | 图片 → PDF | **已生成**（PNG；JPG 缺纯 stdlib 编码器，待测） |

---

## 5. 必测项（PRD §14.2，逐项）

| # | 测试项 | 方法 | 判定标准 | 当前状态 |
|---|---|---|---|---|
| T-01 | 合并/拆分正确性 | 真实 qpdf 合并 100+1 页、500 页按每 10 页拆 50 份 | 页数、顺序一致 | **部分通过**（页数/顺序通过；书签合并当时待测，已于 §31 补齐并加 e2e；三种拆分模式待测） |
| T-02 | 压缩质量 | F-10 → 目标 5MB，检查文字可选与前后对比图 | 目标 ±10%、文字层仍在、无明显模糊 | **待测**（需真实 app 的压缩引擎） |
| T-03 | OCR 准确率 | F-11 标准测试集 | 英 ≥95%、中 ≥90% | **待测** |
| T-04 | 大文件稳定性 | F-08/F-09，监控内存 | 500MB 不崩溃、内存 ≤1.5GB | **待测** |
| T-05 | 离线可用 | 激活一次后断网逐项执行 | 除首次激活外全功能可用 | **待测**（步骤见 `tests/license_test.md`） |
| T-06 | 无上传 | Wireshark/Fiddler、Little Snitch + `site/verify.html` | 抓包无文件外发 | **待测** |
| T-07 | 失败恢复 | F-07 损坏文件、取消、磁盘不足 | 原文件不改动、有错误码、队列不中断 | **待测**（代码层有断言保护，未端到端执行） |

---

## 6. 功能验收（PRD §7，F1–F9）

| 编号 | 功能 | 代码状态 | 验收状态 |
|---|---|---|---|
| F1 | 合并 PDF | 已实现（qpdf + 页数校验 + 进度 + 取消） | **部分**（页数之和通过；100 文件 ≤60s / 内存峰值待测；书签合并原为已知限制，已按 §31 补齐，真实 qpdf e2e 通过） |
| F2 | 拆分 PDF | 已实现（everyN / ranges / bookmarks） | **部分**（每 10 页 → 50 文件通过；另两模式待测） |
| F3 | 压缩到指定大小 | 已实现（lopdf 图像 XObject 重编码 + 二分搜索目标大小） | **待测**（效果、文字层、前后对比图未实测） |
| F4 | PDF ⇄ 图片 | 已实现（PDFium 动态加载 + `image` 编码；lopdf 生成 PDF） | **待测**（300DPI 10 页 ≤30s、页序未实测） |
| F5 | 本地 OCR | 已实现（Tesseract sidecar，eng+chi_sim，可搜索 PDF/TXT） | **待测**（准确率与耗时未实测） |
| F6 | 签名与表单 | 已实现（签名图章 XObject、字段 `/V`、扁平化） | **待测**（Adobe Reader 打开、字段不可编辑未实测） |
| F7 | 批量处理 | 已实现（错误隔离 + `report.csv`） | **部分**（schema 通过；200 文件真机待测） |
| F8 | 授权与试用 | 已实现（7 天试用、Ed25519 离线缓存、导出限制 E2005） | **部分**（密钥/验签通过；断网与到期场景待测） |
| F9 | 自动更新 | 已实现（前端 Tauri Updater check/downloadAndInstall、开关、签名校验） | **部分**（开发公钥与 `.sig` 已生成并验签；endpoint 仍为占位，升级/回滚未实测） |

---

## 7. 非功能项（PRD §8）

| 项 | 判定标准 | 当前状态 |
|---|---|---|
| 安装包体积 | Win ≤60MB（不含语言包）/ 含语言包 ≤100MB；mac dmg ≤80MB | **通过（Windows）**：§14 重建后 NSIS 27.08 MiB（首轮 26.98 MiB），已含 eng+chi_sim 语言包与全部依赖 DLL；mac dmg **待测** |
| 冷启动 ≤3s / 空闲内存 ≤200MB | 真机测量 | **待测** |
| 处理 500MB 文件内存 ≤1.5GB | 真机测量 | **待测** |
| 离线（除激活外全程可用） | 断网测试 | **待测**（代码无上传路径） |
| 隐私：无遥测、无上传 | 抓包 | **待测**（说明页 `site/verify.html` 已交付） |
| 兼容：Windows 10 1809+、PDF 1.4–2.0、已知密码加密件 | 真机矩阵 | **待测** |
| 崩溃安全：失败不损坏原文件、临时文件可清理 | 代码层临时目录 + 原子替换 | **待测** |
| macOS 签名 / notarize / Gatekeeper | Developer ID + notarytool | **待外部条件** |
| 架构：Windows x64 + macOS universal | CI 矩阵 + lipo | **待外部条件** |
| 无障碍：键盘可完成全部操作、高对比度 | 已实现（`prefers-contrast`、可见 focus、Esc 取消） | **部分**（代码与截图检查通过，无障碍审计待测） |

---

## 8. 复现命令（本次实际执行）

```bash
# 依赖
npm install
# Rust 工具链（项目内隔离，不污染系统）
#   rustup-init -y --no-modify-path --profile minimal --default-toolchain stable-x86_64-pc-windows-msvc
#   CARGO_HOME=<repo>/.toolchain/cargo RUSTUP_HOME=<repo>/.toolchain/rustup

npm run build
cargo check --manifest-path src-tauri/Cargo.toml
node scripts/provision-sidecars.mjs --target x86_64-pc-windows-msvc
python scripts/generate-fixtures.py --huge --big
python tests/acceptance.py --smoke            # qpdf 在 PATH 时 A2/A4 为真实调用
node scripts/gen-license-keys.mjs --check
node scripts/license-server.mjs               # 另开终端，POST /api/activate 验签
# createUpdaterArtifacts: true，构建时必须提供 minisign 私钥（CI 走 secrets）
#   TAURI_SIGNING_PRIVATE_KEY=<path-or-key>  TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<password>
npx tauri build --bundles nsis               # Windows 未做 Authenticode 签名的包 + updater .sig

# 双平台（CI 或 macOS 真机）
npm run tauri build -- --target aarch64-apple-darwin
npm run tauri build -- --target x86_64-apple-darwin
```

证据文件：`tests/out/acceptance-report.md`、`tests/out/build.log`、`tests/out/ui-preview.png`、`tests/out/site-preview.png`、`src-tauri/bin/sidecar-manifest.json`。

---

## 9. 待外部条件清单

- [ ] Windows EV 证书 / Azure Trusted Signing；macOS Apple Developer ID（$99/年）+ notarytool 公证 + staple
- [ ] CI secrets（18 项，见 `.github/workflows/build.yml` 末尾）+ 真实更新端点与**生产** minisign 密钥对（替换当前开发公钥）
- [ ] Gumroad / 爱发电 商品、密钥发放服务与真实激活接口（当前为本地 mock）
- [ ] macOS arm64 / x64 真机构建与 universal dmg
- [ ] 标准 OCR 测试集、真实扫描件、500MB 大样本、抓包工具与 canary 文件
- [ ] 双平台真机端到端执行 §5/§6/§7 全部待测项并留存截图、日志、`report.csv`、抓包片段

---

## 10. 当前结论

| 统计 | 数量 |
|---|---|
| 本机实测通过 | **10 项（V-01…V-10）** |
| 失败 | **0** |
| 验收 smoke | **9 PASS / 0 FAIL / 14 SKIP(manual)** |
| 待测 / 待外部条件 | PRD §14.2 全部性能、准确率、隐私抓包、双平台与签名项 |

**结论**：代码层、工程化与文档层已按 PRD 落地并通过本机静态/编译/供给/验收 smoke 验证；**尚未完成真机双平台端到端测试、签名与公证，不得对外宣称"测试通过"或发布未签名安装包**。补齐 §9 后再逐项把「待测」更新为实测结果并附证据。

---

## 11. 附录：项目迁移与复验（2026-11-16）

项目根目录由 `<old-repo>` 整体迁至 `<repo>`（同盘符重命名，工具链、构建缓存、sidecar、安装包一并保留；需求文档副本纳入 `docs/requirements/`）。迁移详情见 `tests/out/MIGRATION.md`。

迁移后原地复验，命令中的 `CARGO_HOME` / `RUSTUP_HOME` / qpdf `PATH` 均改为新路径：

| 检查 | 结果 |
|---|---|
| `npm run build`（`tsc --noEmit` + `vite build`） | 通过，59 modules |
| `cargo check`（新工具链路径） | 通过，`EXIT=0`，48.70s；日志 `tests/out/cargo-check.log` |
| `python tests/acceptance.py --smoke` | 9 PASS / 0 FAIL / 14 SKIP(manual)，报告路径自动更新为 `<repo>` |
| `node scripts/provision-sidecars.mjs --dry-run` | 通过 |
| `npm run dev` → `http://localhost:1420` | HTTP 200，见 `tests/out/dev-server.log` |
| UI 截图核验 | `tests/out/ui-preview.png`：四区布局、预览横幅、七个入口、参数面板、底部进度栏均正常，无错位/遮挡 |
| UI 交互冒烟（CDP，浏览器预览模式） | `node tests/ui-interaction-smoke.mjs`：**11 PASS / 0 FAIL**。覆盖首屏标语、预览横幅、七入口、拖放入列表、导出按钮可用性、进度事件与取消按钮、完成不伪造输出、面板切换、关于/授权对话框、Esc 关闭对话框、Esc 取消任务；全新实例与刷新后各跑一次结果一致 |

> 迁移只改变路径，不改变功能与验收状态：§5–§7 的真机/双平台/签名/公证测试仍为「待测」，`PayOnce-PDF-Setup-1.0.0.exe` 仍是**未做 Authenticode 签名的内部验证包**。

---

## 12. F10–F14 范围变更复验（2026-11-16）

用户批准功能面对标 PDFgear 后，新增 5 个命令与 12 个前端入口。本节只记录**本次真实执行过**的检查，未跑的一律标注待测。

| 检查 | 命令 / 动作 | 结果 |
|---|---|---|
| 前端类型与构建 | `npm run build`（`tsc --noEmit` + `vite build`） | **通过**，60 modules，`dist/` 已更新 |
| Rust 编译 | `cargo check --lib` | **通过**，0 warning / 0 error |
| 引擎单元测试 | `cargo test --lib engine::extras::tests` | **11 PASS / 0 FAIL** |
| 真实 PDF 文件级测试 | `decorate_writes_real_pdf`、`page_ops_writes_real_pdf` | **通过**：写入水印/页码/`/Info` 后重新载入校验；旋转/删除/插入/裁剪后页数与字典校验；中文水印被明确拒绝 |
| OOXML 可解析性 | Python `zipfile` + `xml.dom.minidom` 解析 `sample.docx` / `sample.xlsx` | **通过**：ZIP CRC 全对、必需条目齐全、全部 XML well-formed、中文与 XML 转义正确 |
| 既有验收 smoke | `python tests/acceptance.py --smoke` | **重跑通过**：9 PASS / 0 FAIL / 14 SKIP(manual)，报告 `tests/out/acceptance-report.md` |
| UI 交互冒烟（浏览器预览模式） | `node tests/ui-interaction-smoke.mjs`（全新 Edge CDP 实例，1440×900） | **15 PASS / 0 FAIL**：12 入口齐备、侧栏可滚动、F10 面板含「查找/替换/不重排」、F14 明示「不还原版式」、拖放/进度/取消/对话框链路 |
| UI 布局截图 | `tests/out/ui-preview-12tools.png` | **通过**：1440×900 下 12 个入口与底部「关于/授权/网络活动」全部可见，四区无错位/遮挡 |

**全部待测（不得写成通过）**

- F10 真机：PDFium `FPDFText_SetText` 对真实 Word/Chrome 生成 PDF 的替换效果、中文 CJK 字形、跨文本对象命中、替换变长的视觉重叠、Adobe Reader 打开无损坏。
- F11 真机：`/QuadPoints` 与命中文字是否重合、注解在 Adobe Reader / PDFgear / 预览 中的显示与可编辑性。
- F13 真机：AES-256 密码与权限位（qpdf `--show-encryption`）、水印不遮挡正文。
- F14 真机：Word / Excel 实际打开 DOCX / XLSX（当前只验证 ZIP/XML 结构合法）。
- 全部新命令的 Tauri 端到端（含授权、取消、进度事件、`report.csv` 批量新操作）。
- 安装包：新增代码后 **NSIS 包未重新构建**，既有 26.98 MiB 包不含 F10–F14。（**此条已在 §14 解决**：重建为 27.08 MiB 并验签。）

**结论**：本次只达成「代码编译通过 + 纯逻辑与文件级单测通过」；产品层「已对标 PDFgear」尚未成立，发布前必须补 §12 待测清单。详见 `docs/pdfgear-parity-matrix.md` §4。

> ⚠️ **§12 的「11 项单测通过」结论已被 §13 实证推翻**：当时 F10 改字在真实 PDF 上根本没有生效。§12 保留原文是为了记录「编译通过 ≠ 功能可用」这一教训，不作为功能验收依据。

---

## 13. 第二轮：真实 PDF 文件级验证 + F15/F16（2026-11-16 同日）

用户指令「定价稍晚再说，高保真 Word/Excel、AI 问答和移动端暂时不要考虑，其它继续」后执行。本节只记录**真实跑过**的命令，未跑的明确标注待测。

### 13.1 解耦与测试基础设施

为让引擎级测试不依赖 Tauri runtime，做了两处结构性改造：

- `engine/mod.rs::JobCtx.app` 由 `&AppHandle` 改为 `Option<&AppHandle>`，新增 `ctx.app()`（离线模式返回 E2004）；`engine/ocr.rs` 全部改用局部变量 `app`。
- 命令层负责 `render::load_pdfium(app)?`，`engine/extras.rs` 的 `replace_text` / `annotate` / `export_text` 改为接收 `&Pdfium`，测试可直接注入随包 PDFium DLL。

测试辅助：`TEST_PDFIUM`、`test_pdfium()`、`ascii_pdf_bytes()`、`cjk_pdf()`（用系统 `simhei.ttf` 生成真嵌入 CJK 字体 PDF）、`page_text()`、`qpdf_check()`、`offline_ctx()`。

### 13.2 实证抓出并修复的两个真 bug

| # | 症状（首次跑测试时） | 根因 | 修复 |
|---|---|---|---|
| 1 | `e2e_edit_ascii_real_pdf` 读回仍是 `"Hello World"`；`e2e_edit_cjk_real_pdf` 替换数字失败；`e2e_edit_missing_glyph_fails_closed` 期望报错却成功；组合链路导出为空 | PDFium 修改页面对象后保存前必须 `FPDFPage_GenerateContent`，否则对象级修改全部丢失；`FPDFText_SetText` 对字体缺字形字符会**静默写空格/问号**且不返回错误 | 每页 `page.regenerate_content()`；非 ASCII 替换前检查 `font.is_built_in()` / `is_embedded()` 拒绝；新增 `verify_edit()` **写后读回校验**（重新打开产物，读不到新文字就删除输出并报错） |
| 2 | 旋转 90° 后 F11 找不到文字、F14 导出空白（内容流与字体资源完好） | 随包 PDFium 在 `/Rotate ≠ 0` 页面上 `FPDFText_*` 返回空文本 | `unrotated_copy()` + `with_unrotated<T>()`：查找/导出时生成只把 `/Rotate` 置 0 的临时副本，用完删除，正式产物保留旋转；改字校验改用 `collect_runs()` 拼文本对象绕开该 API |

> 这一节的工程价值高于新功能：证明 §12 的「编译通过 + 纯逻辑单测」完全不足以宣称功能可用。

### 13.3 本轮真实执行过的检查

| 检查 | 命令 / 动作 | 结果 |
|---|---|---|
| Rust 编译 | `cargo check --lib` | **通过**，0 warning / 0 error |
| Rust 全量单测 | `cargo test --lib -- --test-threads=1` | **28 PASS / 0 FAIL**（含 11 项真实 PDF 文件级） |
| 真实 PDF 文件级端到端 | `engine::extras::tests::e2e_*` + `engine::objects::tests::*` | **通过**：改字 ASCII / 改字中文 / 缺字形 fail-closed / 批注 / 旋转页批注 / 四格式导出 / 全链路（改字→批注→旋转→导出）/ 真脱敏 / 中文文本转 PDF / 中文水印 / AcroForm Widget / 书签往返；产物均经 `qpdf --check` |
| 前端类型与构建 | `npm run build`（`tsc --noEmit` + `vite build`） | **通过**，61 modules，`dist/` 已更新 |
| UI 交互冒烟（浏览器预览 / CDP） | `node tests/ui-interaction-smoke.mjs`（1440×900，强制 `Page.reload({ignoreCache:true})`） | **此前连续两次 18 PASS / 0 FAIL；本轮补 2 项文案断言后 20 PASS / 0 FAIL**：15 个入口、侧栏可滚动、F15/F16 面板文案、拖放/进度/取消/对话框链路 |
| UI 布局截图 | `tests/out/ui-preview-15tools.png` | **通过**：15 入口与底部「关于/授权/网络活动」均可见，四区无错位/遮挡 |
| 既有验收 smoke | `python tests/acceptance.py --smoke` | **重跑通过**：9 PASS / 0 FAIL / 14 SKIP(manual) |

期间修掉的编译/断言问题：`AtomicBool` 未导入、`page.text().all()` 生命周期 E0597、`FormField` 缺 `Serialize`、`let mut w` 未使用、`pdf_error` dead_code、`decorate_pdf` 中文水印分支、Windows `rename` 覆盖前先删除目标、UI 冒烟 F15 断言误用「顶层书签」与 CDP 页面状态串扰。

### 13.4 本轮仍未做（不得写成通过）

- **真机 Tauri GUI 端到端**：GUI 内原生文件对话框、授权状态、命令链路、进度/结束事件，F10–F16 一个都没在真实 app 里跑过。本节的「端到端」是**引擎级文件级**，不是 Tauri 真机。
- **阅读器 / Office 兼容性**：Adobe Reader 显示注解与填写 AcroForm、PDFgear 打开、Word/Excel 打开 DOCX/XLSX、macOS 预览。
- **F13 AES-256** 阅读器实测（qpdf 结构校验已过）、水印不遮挡正文的目视复核。
- **F16 系统打印 / 邮件** 在真机唤起；`install_ocr_language` 真机联网下载与断网降级；macOS 分支（`lp` / `open`）未编译验证。
- **安装包**：§12 末尾所述「新增代码后 NSIS 包未重新构建」已在 **§14 重建完成**（27.08 MiB + `.sig`）；但**仍未做 Authenticode 签名、真机安装与升级实测**。
- 双平台端到端、抓包验证、性能/内存/冷启动、Windows Authenticode 签名、macOS 公证、预购 A/B。

### 13.5 结论

本轮达成「**引擎级真实 PDF 文件级验证 + F15/F16 代码完成 + 前端 15 入口同步**」，并修好两个会导致功能实际不可用的真 bug。产品层「已对标 PDFgear」**仍不成立**：真机 GUI 端到端、阅读器兼容、双平台签名分发全部未完成。详见 `docs/pdfgear-parity-matrix.md` §4/§5。

---

## 14. 全量验证与 Windows 安装包重建（2026-11-16 收尾）

用户「其它，请继续」的最后一步：在完成 §13 与全部文档/官网同步后，重跑全量验证并重建**包含全部新命令**的 Windows 安装包。以下为本机真实执行结果。

### 14.1 全量验证命令与结果

| # | 检查 | 命令 | 结果 |
|---|---|---|---|
| V-14.1 | 前端类型检查与生产构建 | `npm run build`（`tsc --noEmit` + `vite build`） | **通过**，0 错误，61 modules；`dist/` 已更新（CSS 10.18 kB、主 JS 289.85 kB） |
| V-14.2 | Rust 编译 | `cargo check --lib`（项目内隔离工具链，`CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0`） | **通过**，`Finished dev profile in 3.06s`，0 error / 0 warning |
| V-14.3 | Rust 全量单测 | `cargo test --lib -- --test-threads=1` | **28 PASS / 0 FAIL**（含 11 项真实 PDF 文件 + PDFium DLL + `qpdf --check` 文件级端到端），1.45s |
| V-14.4 | 既有验收 smoke | `python tests/acceptance.py --smoke` | **9 PASS / 0 FAIL / 14 SKIP(manual)**；真实 qpdf 合并 `101=100+1`、拆分 50 文件合计 500 页；报告 `tests/out/acceptance-report.md` |
| V-14.5 | UI 交互冒烟（浏览器预览 / CDP） | `node tests/ui-interaction-smoke.mjs` | **20 PASS / 0 FAIL**：15 入口、侧栏滚动、F10/F14/F15/F16 诚实边界、**新增「中文水印与页眉页脚边界已区分」「语言包清单 42 种」两项断言**、拖放/进度/取消/对话框/Esc 链路 |
| V-14.6 | 官网结构校验 | Python `HTMLParser` 标签闭合 + 站内 `href` 存在性；CDP 1440px 实际渲染 | **6 页全部标签闭合、0 断链**；中英首页各 15 张功能卡（`grid-3` = 3 列 × 5 行，`overflow=0`）、FAQ 19 条、隐私页 3 类请求表；截图 `tests/out/site-{en,zh,faq,privacy}-round2.png` |
| V-14.7 | Windows NSIS 安装包重建 | `npx tauri build --bundles nsis`（带开发 minisign 密钥） | **通过**：`PayOnce-PDF-Setup-1.0.0.exe` **27.08 MiB（28,392,755 B）** + `.sig` 436 B；release profile 10m07s，日志 `tests/out/build-round2.log` |
| V-14.8 | 新命令确实进入发行二进制 | 对 `src-tauri/target/release/payoncepdf.exe` 做字符串检查 | **8/8 全部命中**：`create_form_pdf` / `redact_pdf` / `list_bookmarks` / `manage_bookmarks` / `text_to_pdf` / `share_file` / `list_ocr_languages` / `install_ocr_language` |
| V-14.9 | 更新签名密码学校验 | `python tests/verify-updater-signature.py`（`cryptography` 按 `minisign-verify 0.3.0` 算法复现） | **通过**：key id 与 `tauri.conf.json` 公钥一致（`654e340d1fe4b8e5`）；① 文件签名 = Ed25519 over BLAKE2b-512(安装包) 验签 OK；② 全局签名覆盖 trusted comment 验签 OK；③ 负向自检（改 1 bit）确实失败 |
| V-14.10 | 验签可复现性落地 | 脚本入库 + `site/verify.html` 增加「Verify the update signature yourself」小节 + README / site README 脚本清单同步 | **通过**：脚本退出码 0；官网 6 页 HTML 标签闭合、0 断链复核通过 |
| V-14.11 | 定价拍板后官网口径复核 | Python `HTMLParser` 标签闭合 + 站内 `href` 存在性（官网 6 页） | **通过 6/6**：0 未闭合、0 断链；中英首页 / 定价页 / FAQ 统一为 **$29 / ¥169**（首发 14 天早鸟 $19 / ¥109），定价页补「为什么不是 $10 / 单档 / 国内渠道 / 未签名非实时报价」，FAQ Release status 改为「定价已定但未上架」；依据 `docs/pricing-decision.md` §8、PRD §22.8 |

> V-14.9 是本轮新增的硬校验：以前只确认「生成了 .sig」，这次实际用 `tauri.conf.json` 里的公钥把两段签名都验了一遍，确认签名覆盖的是**当前这个安装包**而非旧产物。校验逻辑已固化为 `tests/verify-updater-signature.py`，任何人不需要私钥即可重跑；`site/verify.html` 也给出了对应命令。
>
> V-14.8 注意：字符串检查必须针对**未压缩的** `src-tauri/target/release/payoncepdf.exe`。NSIS 安装包是 LZMA 压缩容器，直接对 `.exe` 安装包 grep 命令名会全部落空（8/8 MISS），这是压缩导致的假阴性，不代表新命令没进包；构建日志 `tests/out/build-round2.log` 显示 `makensis` 是在 `payoncepdf.exe` 编译并 patch 完成后才执行的。

### 14.2 重新构建的产物（§14 历史记录；当前产物见 §15）

> 下表是 2026-11-16 14:49 第二轮构建的**历史产物**。当时沿用 Tauri 默认命名与旧品牌名（旧值见 `docs/RENAME-MIGRATION.md`）；表中文件名在品牌改名时被同步替换，**不代表当时磁盘上的实际文件名**。当前 18:14 重建产物见 §15。

| 文件 | 大小 | 时间 | 说明 |
|---|---|---|---|
| `src-tauri/target/release/payoncepdf.exe` | — | 2026-11-16 14:49 | 含 F1–F7 + F10–F16 全部命令 |
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | 27.08 MiB | 2026-11-16 14:49 | 替代旧 26.98 MiB 包（旧包**不含** F10–F16） |
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe.sig` | 436 B | 2026-11-16 14:49 | 开发密钥签名，密码学校验通过 |
| `tests/out/build-round2.log` | — | — | 本轮完整构建日志 |

### 14.3 诚实边界（不得写成已完成）

- 安装包**仍无 EV / Authenticode 签名**，Windows SmartScreen 会警告；**仅供内部验证，不可对外分发**。
- 更新签名的密钥是 `npx tauri signer generate --ci` 生成的**开发密钥**（历史开发口令与私钥路径见 `docs/RENAME-MIGRATION.md`；私钥文件已改名为 `.toolchain/payoncepdf-updater.key`，口令未随品牌改名变更），发布前必须换成生产密钥对并替换 `tauri.conf.json` 的 `pubkey`、在 CI 配好 `TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD`。
- 更新端点 `https://payoncepdf.com/api/update/...` 仍是占位；没有真实更新服务器。
- **真机安装、冷启动、升级/回滚、抓包验证、F10–F16 在 GUI 内跑通全部未做**（§13.4 清单不变）。
- macOS arm64/x64 构建、Developer ID 签名、notarize、staple、universal dmg 全部未做；CI 18 个 secrets 未配置。
- 定价已于 2026-11-16 拍板为 **$29 / ¥169**（首发 14 天早鸟 $19 / ¥109），见 `docs/pricing-decision.md` 与 PRD §22.8；高保真 Word/Excel、AI 问答、移动端按用户指示**保持暂缓**；**预购 A/B 仍未执行**。定价决策未改变本节的发布就绪度结论。

### 14.4 本节结论

代码、引擎级文件级验证、前端契约、全部文档与官网 6 页口径已同步到 15 工具 / F15 / F16 / 三类可枚举网络请求；Windows 内部安装包已重建并验签。**发布就绪度仍为「否」**：缺真机 GUI 端到端、阅读器兼容性、Authenticode 签名与 macOS 分发链。

---

## 15. 品牌改名复验（2026-11-16 18:14，PayOnce PDF）

> **本节是当前最新状态**。§14 及以前的产物命名、密钥口令、路径均为改名前的历史记录；完整旧值映射见 `docs/RENAME-MIGRATION.md`，价格四字段与裁决记录见 `config/product.json`。

### 15.1 验证结果

| 项 | 命令 | 结果 |
|---|---|---|
| V-15.1 | 旧名残留扫描（带词边界，正则全文见 §15.3） | **0 行真旧名**（源码 / 配置 / 文档）；详见 §15.3 的正则说明 |
| V-15.2 | `npm test`（`node --test tests/rename-contract.test.mjs`） | **8 PASS / 0 FAIL**：Tauri 配置、包名、许可证前缀长度与正则、事件名、用户可见文案、价格集中、产物脚本 |
| V-15.3 | `npm run build` | **通过**：`tsc --noEmit` + Vite 61 modules，0 error；`dist/index.html` 标题为 PayOnce PDF |
| V-15.4 | `cargo check --lib` | **CHECK_OK**：`Compiling payoncepdf v1.0.0`，0 error / 0 warning |
| V-15.5 | `cargo test --lib -- --test-threads=1` | **31 PASS / 0 FAIL**（原 28 项 + 新增 3 项许可证前缀测试：`new_prefix_is_accepted` / `legacy_prefix_is_rejected` / `malformed_keys_are_rejected`） |
| V-15.6 | `python tests/acceptance.py --smoke` | **9 PASS / 0 FAIL / 14 SKIP(manual)**；fixtures 按 `PAYONCEPDF_PAGE_` 重新生成，真加密样本口令同步为 `payoncepdf`，`qpdf --check` 通过 |
| V-15.7 | `node tests/ui-interaction-smoke.mjs` | **20 PASS / 0 FAIL**（15 入口、中文水印边界、42 种语言包）。首次与 release 构建并发时 2 项进度断言因计时器被抢占失败，CPU 空闲后复跑全绿 |
| V-15.8 | 官网 6 页 HTML 标签闭合 + 站内链接 | **6/6 PASS**，0 未闭合、0 断链 |
| V-15.9 | `npx tauri build --bundles nsis` | **通过**：主程序 `src-tauri/target/release/payoncepdf.exe`；Tauri 默认名 `PayOnce PDF_1.0.0_x64-setup.exe` 27.08 MiB |
| V-15.10 | `node scripts/rename-bundles.mjs` | **通过**：2 个文件重命名为 `PayOnce-PDF-Setup-1.0.0.exe` / `.exe.sig` |
| V-15.11 | `python tests/verify-updater-signature.py` | **通过**：key id `654e340d1fe4b8e5` 匹配；① 文件签名（Ed25519 over BLAKE2b-512）OK；② 全局签名 OK；③ 篡改 1 bit 负向自检 OK |
| V-15.12 | 发行主程序字符串检查 | **8/8 命中** F15/F16 新命令（须查未压缩的 `payoncepdf.exe`，不能查 LZMA 压缩的安装包） |
| V-15.13 | CDP 截图 | `tests/out/payoncepdf-home.png`（侧栏 PayOnce PDF / 一次付费 PDF、15 工具）、`tests/out/payoncepdf-about.png`（对话框标题「关于 PayOnce PDF」） |
| V-15.14 | 项目根目录重命名（21:35，旧工作区目录 → `E:\PayOncePDF`） | **通过**：停 2 个 vite + esbuild 子进程 → 同盘符原子移动 → 新路径重启 1420（HTTP 200）。复验 `npm test` 8/8、`npm run build` 1.34s、`cargo check --lib` 全量重编译 5m17s 后 CHECK_DONE、`cargo test --lib` 31 PASS / 0 FAIL、UI 冒烟 20/0、acceptance smoke 9 PASS / 0 FAIL / 14 SKIP(manual)、官网 6/6 PASS、验签三段 OK、安装包 SHA-256 未变、边界残留 0。源码/配置/官网/参考资料无硬编码项目根路径；仅 `docs/RENAME-MIGRATION.md` 的历史路径链、`tests/out/*.log` 与 `src-tauri/target/**/*.d` 保留旧值（历史生成物） |
| V-15.15 | 价格冲突裁决与参考资料同步（22:13） | **通过**：用户确认 `$19 / ¥149` → `$29 / ¥169`；`E:\PainRadar\docs\pdf-launch\` 5 个文件共 27 处（26 处批量精确替换，每条规则断言命中 1 次；另 1 处为 `posts.md` 顶部状态注记）；`config/product.json` 的 `pending_confirmation` 置 `false` 并新增 `_resolution`；`tests/rename-contract.test.mjs` 同步断言并新增「参考资料价格一致性」用例；`npm test` 9 PASS / 0 FAIL；官网 6 页 6/6 PASS；参考资料 2 个 HTML 标签闭合通过；决策前正式价残留 0（历史任务说明除外）。定金 / 早鸟 / A/B / 竞品价 / 历史基线未改；`npm run build` 与验签复跑通过，安装包字节不变 |

### 15.2 当前安装包与截图

| 产物 | 路径 | 大小 / 校验 |
|---|---|---|
| 项目根目录 | `E:\PayOncePDF`（21:35 由旧工作区目录重命名；旧路径已不存在） | 所有下表路径均相对此根目录 |
| Windows NSIS 安装包 | `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | 28,396,590 B（27.08 MiB），SHA-256 `0b41dbd1593ea3d75c7a030cec68b0a112911b4debd330506540c6b44e32e736` |
| 更新签名 | `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe.sig` | 440 B，两段 Ed25519 验签通过 |
| 主可执行文件 | `src-tauri/target/release/payoncepdf.exe` | 7,110,144 B |
| 应用首页截图 | `tests/out/payoncepdf-home.png` | 88,631 B |
| 关于对话框截图 | `tests/out/payoncepdf-about.png` | 121,580 B |
| 完整构建日志 | `tests/out/rename-build.log` | check / test / build / rename / 验签 五步全通过 |

### 15.3 grep 口径说明（重要）

任务给出的字面扫描正则（原文见 `docs/RENAME-MIGRATION.md` §4）会命中**新名称本身**：新英文名与小写包名都包含该正则描述的子串，连新的测试样本标记也含旧标记子串。所以「改后字面 0 命中」在逻辑上不可能成立。本仓库实际使用带词边界的残留正则：

```bash
rg -n -i '\boncepdf|\bonce[ _-]?pdf\b|\bONCE-' \
  --glob '!node_modules/**' --glob '!src-tauri/target/**' --glob '!.toolchain/**' \
  --glob '!dist/**' --glob '!.sidecar-cache/**' --glob '!tests/out/**' --glob '!tests/fixtures/**' .
```

结果为 0。`docs/RENAME-MIGRATION.md`（迁移说明）与 `E:\PainRadar\docs\pdf-launch\workbuddy-rename-prompt.md`（改名任务原始说明）是两处按验收标准**有意保留**旧值的文档。生成的 `dist/`、`src-tauri/target/`、`tests/fixtures/`、`.sidecar-cache/` 属于构建产物/缓存，不在「源码 / 配置 / 文档」范围内；其中仍可有历史字符串或新名称自匹配。

### 15.4 诚实边界（不得写成已完成）

- 截图来自**浏览器预览模式**（Vite dev server），不是真机 Tauri 窗口；`关于` 对话框显示 `1.0.0 (preview)` 与 `browser / n/a`。真机 GUI 端到端、原生对话框、F10–F16 在 GUI 内跑通仍未做。
- **macOS 产物未构建**：本机为 Windows，`PayOnce PDF.app` / `PayOnce-PDF-1.0.0.dmg` 的命名由 `.github/workflows/build.yml` 配置保证，未在 macOS 实机生成或验证。
- 安装包**仍无 Authenticode / EV 签名**，Windows SmartScreen 会警告；开发 minisign 密钥与占位更新端点未替换。**不可对外分发**。
- `.sig` 的 trusted comment 记录的是 Tauri 打包时的默认文件名 `PayOnce PDF_1.0.0_x64-setup.exe`（含空格），随后被 `rename-bundles.mjs` 改成连字符发行名；签名覆盖的是**文件字节**，验签仍通过，但文件名与 trusted comment 不一致，上线前建议统一。
- 目录内仍保留改名前的历史安装包与其 `.sig`（14:49，旧品牌名命名，旧值见 `docs/RENAME-MIGRATION.md`），仅供对照，**不得分发**。
- **价格冲突已裁决（22:13）**：用户确认参考资料正式价由 `$19 / ¥149` 改为 `$29 / ¥169`；5 个文件已同步，`config/product.json` 的 `pending_confirmation` 为 `false`。仍**未验证**的是商业侧：预购价格 A/B、PRD §1.4 三假设、竞品官网当前价格复核。
- DNS、证书、Gumroad 商品名、Apple/Windows 签名、商标检索均不在本次代码范围内。

---

## 16. 真机测试准备（2026-11-16 22:35，用例全部未执行）

> 本节只记录**准备工作**。真机 GUI 用例一条都还没跑，状态一律为「未执行」，
> 不得写成 PASS。执行结果由人工填入 `tests/out/real-device-results.csv` 后再回写本节。

### 16.1 本轮新增 / 改动

| 文件 | 作用 |
|---|---|
| `docs/real-device-test-plan.md` | 真机测试手册：三种启动方式、干净状态、激活 mock、G0–G9 + BLOCK 共 10 组用例、判定标准、已知边界与预期 FAIL、取证清单 |
| `tests/out/real-device-results.csv` | 预填用例 ID / 分组 / 优先级 / 素材 / 预期（2026-11-20 F18 新增 `RD-G0-09/10/11`；2026-11-21 F19 新增 `RD-G0-12/13`，当前共 **65 条**），`result` 等 6 列留空待人工填写（UTF-8 BOM，Excel 可直接打开） |
| `scripts/preflight-real-device.py` | 严格只读的一键预检：产物与哈希、运行时引擎、配置、素材完整性、端口、试用状态、安装包资源缺口；输出 `tests/out/preflight-real-device.json` |
| `src-tauri/target/release/resources/tessdata/` | **补齐运行时 OCR 语言数据**（`eng` + `chi_sim`），使未打包 release 主程序可跑 OCR；与源目录逐字节哈希一致 |

未改动任何业务逻辑、未重建安装包、未修改 `tauri.conf.json`；交付物 SHA-256 保持不变。

### 16.2 预检结果（本次实跑）

| 命令 | 结果 |
|---|---|
| `python scripts/preflight-real-device.py` | **PASS 25 / WARN 3 / BLOCK 2 / FAIL 0**，退出码 0 |
| 安装包 SHA-256 复核 | `0b41dbd…e736`，与 §15.2 记录一致 |
| fixtures 完整性 | 按 `manifest.json` 校验 14 项素材的 SHA-256 与大小，**全部匹配**；真实加密样本 `encrypted.pdf` 存在 |
| 运行时引擎 | `pdfium.dll` 7.8MB、`qpdf.exe`、`tesseract.exe`、两份 traineddata 全部就位 |
| 配置 | `productName` / 窗口标题 / identifier / updater pubkey / 价格四字段（`pending_confirmation=false`）全部通过 |
| 环境 | 1420 预览 200、9222 CDP 在监听；8787 mock 未启动（按需）；`%APPDATA%\app.payoncepdf.desktop` 不存在 → 适合测 LT1 首次试用 |

WARN 3：mock 服务未启动（按需）、`installer.nsi` 仍含旧工作区绝对路径（历史生成物）、目录内残留旧品牌安装包（不得分发）。

### 16.3 开测前发现的阻断项（BLOCK 2，同源缺陷）

`src-tauri/tauri.conf.json` 的 `bundle.resources` 为 `["resources/*"]`；该 glob **只展开文件、不展开 `tessdata/` 目录**。据此：

- `src-tauri/target/release/resources/` 下原本没有 `tessdata/`（本轮已手工补齐，A 路径可测）；
- 生成的 `src-tauri/target/release/nsis/x64/installer.nsi` 共 63 条 `File /a` 指令，**含 `.traineddata` 的 0 条** → 已生成的 NSIS 安装包不含 OCR 语言数据，B 路径安装后 `tessdata.ready` 预期为 `false`、OCR 预期失败；
- 对 NSIS 压缩包直接 grep 文件名会假阴性（LZMA 容器），上述结论来自可读的 `installer.nsi` 生成脚本，不是对安装包的字节猜测。

修复需改 `bundle.resources` 显式纳入 `resources/tessdata/*` 并重建 NSIS，**会改变安装包字节与 SHA-256，需重新签名与验签**；本轮为不推翻刚交付的产物，未擅自修改，列为 `RD-BLOCK-01`（复现）/ `RD-BLOCK-02`（修复后回归）交用户决策。

> **2026-11-20 更新（§18）**：用户授权对标 UPDF 后已按上述方案修改配置并重建 NSIS，`RD-BLOCK-01` 的复现前提不再成立；但 `RD-BLOCK-02`（安装态 OCR 回归）**仍必须真机执行**，不得因配置已改而跳过。本节保留原始缺陷记录以便追溯。

### 16.4 用例状态统计

| 分组 | 条数 | 优先级 | 状态 |
|---|---|---|---|
| G0 启动自检（含 F17 两条）/ G1 基础七工具 / G2 编辑与导出 / G3 文档对象 / G5 原生交互 / G7 隐私网络 / BLOCK 专项 | 40 | P0 | 未执行 |
| G4 打印与扩展 / G6 授权试用 / G8 安装卸载性能 / G9 产物兼容性 | 20 | P1 | 未执行 |
| 合计 | **60** | — | **0 已执行** |

`acceptance.py` 的 M1–M14 与 `tests/license_test.md` 的 LT1–LT6 仍为 `SKIP (manual)`，本轮未改动其判定。

### 16.5 诚实边界

- 真机 Tauri GUI **尚未启动**；本节所有结论来自静态检查与只读预检，不是 GUI 实测。
- `http://localhost:1420/` 是浏览器预览 + mock 后端，**不算真机**；`tests/out/payoncepdf-*.png` 截图也不是真机证据。
- 补齐的 `target/release/resources/tessdata/` 属于**构建产物目录**；`bundle.resources` 已于 2026-11-20 改为 `["resources/*", "resources/tessdata/*"]` 并重建 NSIS，但**安装后的 B 路径仍需真机回归**，见 §18。
- 安装包仍无 Authenticode/EV 签名（SmartScreen 必警告）、更新端点占位、开发 minisign 密钥；macOS 未实机构建、签名、公证。**发布就绪度仍为「否」，安装包不可对外分发。**

---

## 17. 定位 v2 文案同步（2026-11-20，按修正版全量落地）

> 触发：用户 @ 附件 `payoncepdf 定位 v2：用手册结论重写.md`，并在核对后选择「按修正版全量落地」。
> 本节只记录**文案与文档改动**：未改业务逻辑、未重建安装包、未改价格、未改任何测试断言。

### 17.1 数据核对（逐条查 `E:\PainRadar` 原始依据）

| 附件说法 | 核对结果 |
|---|---|
| 付费与订阅 40% / 3.8、编辑与表单 11% / 3.6、配置门槛 9%、弹窗广告 6% / 3.8 | ✅ 与 `pdf-tools-actions.md`（300 样本 / 81 条有效痛点，2026-11-20）一致 |
| 五问全过 / 七品类唯一全过 | ✅ 与 `category-opportunity-playbook.md` 一致 |
| 「27% 提到弹窗/广告」 | ❌ **误引**：27% 出自旧 `pdf-deepdive.md` 的「功能类痛点合计」，新语料弹窗只有 6%。已删 |
| 「37% 差评与收费有关」保留 | ⚠️ 旧语料 123/333（单项「试用扣费」），与新语料 40%（32/81）**分母不同，不得并列** |
| 「性能是隐藏风险，崩一次就退款」 | ⚠️ 补验 §8 已修正：「性能没有扩大，4.0 来自少数重度用户」，P0 改为「OCR 优先级上升、性能维持基线」 |
| 「OCR 占 24%，是重点投入项」 | ⚠️ 24% 数字对，但强度回落到 3.5，结论是「常见但可忍」，**优先级读反** |
| 「迁移 4%」 | ⚠️ 引用文件里未核到该数值，保留为定性判断，不对外引用数字 |

**语料口径**：旧语料 723 评论 / 333 痛点原子（11-16）；新语料 300 样本 / 81 条有效痛点（11-20）。
对外文案统一用新语料的 40% / 6% / 11%；旧 37% 只作历史引文。

### 17.2 五处修正（完整依据见 `docs/positioning-v2.md` §0）

1. **不宣称「没有免费替代」**：改为「免费替代最弱、又被订阅伤害最深」，以「功能不弱于免费竞品 + 文件永不出门」兜底（用户此前已指出 PDFgear 免费可改字；playbook 原文为「几乎无」）。
2. **功能层级回到 15 个工具**：主视觉为「就地改文字」，附件 §3 的「四件事」退回旧 PRD 口径，已改。
3. **`Works offline` 加三类请求限定**：核心工具断网可用，仅激活 / 可选更新检查 / 用户主动装语言包联网，不回到「零网络请求」。
4. **`license never expires` 与「12 个月 1.x 更新」同时出现**：授权永久有效 ≠ 永久免费更新；「服务器下线不影响」依赖 LT4 离线缓存，真机未验证。
5. **`no pop-ups` 改为「无广告 / 无评分弹窗 / 无升级骚扰」**：原生文件与打印对话框是功能必需；不注册 / 无每日次数限制移到第二屏。

附带：删除「OCR 是重点投入项」；`posts.md` EN 帖里 `convert (Word, Excel, images)` 的过度承诺改为「图片互转 + 文字层导出（仅文字，不还原版式）」；FAQ / pricing 补「授权不过期 ≠ lifetime updates」。

### 17.3 改动清单

| 组 | 文件 | 方式 |
|---|---|---|
| 归档 | `docs/positioning-v2.md`（新建） | §0 核对结论 + §A 附件原文逐字保留 |
| 文案板 | `E:\PainRadar\docs\pdf-launch\copy.md`、`docs/requirements/copy.md` | 整份重写为修正版，两份正文逐行一致（仅顶部注记不同） |
| 官网 | `site/index.html`、`index.zh.html`、`faq.html`、`pricing.html` | 45 条批量精确替换中的 21 条 + 2 处补改（中文 title、措辞） |
| 落地页 | PainRadar 与项目内 `landing.html` / `landing.zh.html` 共 4 份 | 24 条精确替换；新增就地改字 / 批注脱敏功能卡 |
| 其它文案 | `posts.md`、`preorder.md`（2 份） | 13 条中的 7 条 |
| 需求 | `docs/requirements/PRD.md`（§1.1 注 / §2 / §4 / §21）、`E:\PainRadar\docs\pdf-launch\PRD.md`（§1.1 注 / §2 / §4） | 6 条精确替换 |

合计：13 个文件经 **58 条**「先断言命中次数、再统一写入」的精确替换（批次 1 = 8 文件/45 条，批次 2 = 5 文件/13 条），另 2 份 `copy.md` 重写、1 个归档文件新建、2 处补改。

### 17.4 复验结果

| 项 | 结果 |
|---|---|
| 官网 6 页 + 4 份落地页：标签闭合 + 站内链接 | **10/10 PASS** |
| 旧主标题残留 / 旧功能层级「四件事」残留 | **0 / 0** |
| 误引「27% 提到弹窗」/ 对外宣称「没有免费替代」 | **0 / 0**（仅否定式说明与归档原文保留） |
| `npm test` / `npm run build` | **9 PASS / 0 FAIL**；构建 2.09s 通过 |
| `python scripts/preflight-real-device.py` | **PASS 23 / WARN 5 / BLOCK 2 / FAIL 0**；与 §16 的 25/3 差异是 1420 / 8787 / 9222 三个后台服务已随会话退出，**非本轮改动引起**；BLOCK 2 仍是 §16.3 的 tessdata 打包缺陷 |
| 更新签名验签 / 安装包 | 三段全 OK；SHA-256 仍 `0b41dbd…e736`，**字节未变**（文案改动不进代码） |
| 价格四字段 | `29 / 169 / 19 / 109`，`pending_confirmation=false`，**未改动** |

### 17.5 诚实边界

- `docs/positioning-v2.md` §A 是**用户原文**，「27% 提到弹窗」「没有免费替代」「37%」等按归档要求有意保留，不代表当前对外口径。
- `posts.md` 数据帖基于旧语料 723 条评论，37% 的论证自洽，**未改成 40%**，仅加顶部口径注记；PRD §1.1 同理。
- 真机 60 条 RD 用例**仍全部未执行**，GUI 未启动；本轮只改文案与文档，不构成任何功能验证。
- 预购 A/B、PRD §1.4 三假设、退款率 <3% 等上线后指标仍为**待验证假设**。
- 发布就绪度仍为「否」：无 Authenticode/EV 签名、macOS 未实机、安装包不可对外分发。

---

## 18. 对标 UPDF 的 L1 落地与重新定价（2026-11-20 21:52，用户授权「由你拍板」）

触发：用户要求「功能对标 UPDF（上云部分不要学）、价格比 UPDF 便宜一点、外观比 UPDF 漂亮实用」，并授权助手拍板。完整决策见 `docs/updf-parity-decision.md`，差距依据见 `docs/updf-gap-analysis.md`。

### 18.1 拍板结论

| 项 | 决定 |
|---|---|
| 对标范围 | **桌面端、离线、PDF 文档工作台能力**；上云能力（云端 AI / 账号 / 云同步 / Web 编辑器 / eSign 邀请跟踪）按用户要求排除 |
| 正式价 | **$39 / ¥199**（原 $29 / ¥169）；UPDF 买断 $84.99 / ¥238，¥199 便宜 16%，且对方另有 $69/年 AI 加购 |
| 早鸟 | **$29 / ¥149**，首发 14 天，非常设价 |
| 授权 | **一把钥匙最多 3 台设备**（原一机一密钥）；单档、12 个月 1.x 更新、2.x 五折、7 天试用、7 天退款全部不变 |
| L1 本轮交付 | F17 内置阅读视图、页面联动、tessdata 打包修复、3 设备授权、三栏工作台视觉 |
| L2 排期（本轮**未做**） | 预览上框选批注/脱敏、批注类型扩展、表单域类型、文档对比、PDF/A、Bates、测量、应用内打印预览、中文水印旋转/透明度、14 格式保版式互转 |
| L3 明确不做 | 云端 AI、账号、云同步、Web 端、eSign 邀请签署、跨设备接力；WYSIWYG 深度排版与高保真 Office 转换受商业 SDK / ~200 MB 体积约束；移动端因双平台签名未完成继续暂缓 |

### 18.2 改动清单

| 组 | 文件 / 内容 |
|---|---|
| **代码（Rust）** | 新增 `src-tauri/src/commands/viewer.rs`（`get_pdf_info` / `render_page_preview` + 2 个单测）；`engine/render.rs` 增 `render_page_png` / `page_size_pt`（纯内存 PNG，DPI 夹 48–200，不落盘不联网）；`commands/mod.rs` 导出 DTO；`lib.rs` 注册两个命令 |
| **代码（前端）** | 新增 `src/components/PdfPreview.tsx`（翻页 / 缩放 / 页码 / 密码 / 「用当前页」联动）；`backend.ts` 加两个命令封装（浏览器预览返回 `null`，不伪造渲染）；`types.ts` 加 4 个 DTO；`App.tsx` 集成画布、顶栏、浅暗主题（`localStorage`）与页码回填 |
| **配置** | `tauri.conf.json` → `resources: ["resources/*", "resources/tessdata/*"]`；`config/product.json` 重写（四字段 + `max_devices_per_key` + `pricing_v2`，旧裁决移入 `previous_resolution`）；`scripts/license-server.mjs` 按 key 记录机器指纹、上限 3 台 |
| **视觉** | `src/styles.css` 追加 v2 工作台层：四行骨架、顶栏、文档画布、卡片化、浅/暗双主题与显式 `html[data-theme]` 变量（覆盖原生 `@media (prefers-contrast: more)` 冲突）、克制动效、滚动条与焦点样式 |
| **文档** | 新增 `docs/updf-parity-decision.md`；PRD §22.10；CONTRACT §3 命令表 + 2 行 F17；官网 6 页 / 4 份落地页 / copy / posts / preorder / 两份 PRD / README 全部同步新价；6 份历史分析文档加「提价说明」注记并保留原始论证数字 |
| **测试** | `tests/rename-contract.test.mjs` 改价格断言并新增 2 个契约用例（F17 注册、决策/打包修复落盘）；`scripts/preflight-real-device.py` 更新安装包哈希与价格基线；真机 CSV 与手册 G0 新增 `RD-G0-07/08`（58 → **60** 条，P0 40 / P1 20）；`tests/capture-ui-v2.mjs` 新增双主题取证 |

### 18.3 复验结果（本轮实跑）

| ID | 命令 | 结果 |
|---|---|---|
| V-18.1 | `npm test` | **11 PASS / 0 FAIL**（原 9 + 新增 2） |
| V-18.2 | `npm run build` | **通过**，`tsc --noEmit` 0 error，62 modules |
| V-18.3 | `cargo check --lib` / `cargo test --lib -- --test-threads=1` | CHECK_OK；**33 PASS / 0 FAIL**（原 31 + viewer 2） |
| V-18.4 | `node tests/ui-interaction-smoke.mjs` | **20 PASS / 0 FAIL**（新 UI 下 selector 与文案全部保留） |
| V-18.5 | `python tests/acceptance.py --smoke` | 无 FAIL，14 项 `SKIP (manual)` |
| V-18.6 | 官网 6 页 + 落地页 4 份 HTML 标签闭合 / 站内链接 | **10/10 PASS** |
| V-18.7 | `python scripts/preflight-real-device.py` | **PASS 28 / WARN 2 / BLOCK 0 / FAIL 0**（BLOCK 从 2 清零） |
| V-18.8 | `npx tauri build --bundles nsis` → `rename-bundles.mjs` | 通过；日志 `tests/out/updf-build.log` |
| V-18.9 | `python tests/verify-updater-signature.py` | **① 文件签名 OK ② 全局签名 OK ③ 篡改 1 bit 负向自检 OK**；key id `654e340d1fe4b8e5` 与配置公钥匹配 |
| V-18.10 | `installer.nsi` 的 `File /a ...traineddata` | **2 条**（`eng` + `chi_sim`），另有 `CreateDirectory` / `Delete`；**§16.3 的打包缺陷已修复** |
| V-18.11 | 价格口径 grep | 对外文案 `final price $29` / `正式价 ¥169` / `¥169 买断` 残留 **0**；四字段 `39/199/29/149`、设备 3、`pending=false` |
| V-18.12 | 双主题截图 | `tests/out/updf-ui-light.png` 165,904 B；`updf-ui-dark.png` 177,400 B；computed `--text` 浅色 `#1b2429` / 暗色 `#e8f0f2` |

2 个 WARN 不阻塞：mock 激活服务未启动（LT3 按需）、目录里保留旧品牌安装包（仅供对照，不得分发）。

### 18.4 当前产物

| 产物 | 路径 | 大小 / 校验 |
|---|---|---|
| Windows NSIS 安装包 | `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,640,918 B（30.18 MiB）**，SHA-256 `193ff0c4ce816a6b6735dc0f3bae21e45ce33cccb609a60fd3cdbab8c08c800e` |
| 更新签名 | 同目录 `.sig` | 440 B，三段验签通过 |
| 未打包主程序 | `src-tauri/target/release/payoncepdf.exe` | 含 F17 两个新命令 |
| 构建链日志 | `tests/out/updf-build.log` | `BUILD_CHAIN_OK` |

安装包从 28,396,590 B 增至 31,640,918 B，增量来自随包 `tessdata`（eng 4.0 MB + chi_sim 2.4 MB）；**旧哈希 `0b41dbd…e736` 已失效**。

### 18.5 诚实边界（务必保留）

- **本轮只交付 L1**。L2 的功能宽度补齐**一条都没做**；L3 是明确不做，不是「已完成」。「功能对标 UPDF」的完整复刻需要商业 SDK、云端账号体系、移动端团队与多年迭代，与 27 MiB 离线包 / 单人排期 / 一次买断的硬约束冲突，已在决策文档 §0 写清定义。
- 真机 **60 条用例仍全部未执行**，Tauri GUI 未启动；F17 的 `get_pdf_info` / `render_page_preview` 只通过编译与契约单测，**从未在真实窗口里渲染过一页 PDF**。
- 两张截图来自**浏览器预览模式 + CSS 示意纸**，不是真机渲染，不可作为 G0 / RD-G0-07 的证据。
- 配置改了、安装包重建了，但**安装后的 B 路径 OCR 仍未经真机验证**；`RD-BLOCK-02` 必须照常执行。
- `.sig` 的 trusted comment 仍记录 Tauri 默认名 `PayOnce PDF_1.0.0_x64-setup.exe`，与连字符发行名不同；文件字节未变、验签通过，上线前建议统一。
- 3 台设备目前只落地在**本地 mock 激活服务**（内存台账）；生产激活服务的持久化、Gumroad 商品价格与折扣码、国内 ¥199 渠道、竞品官网价格复核均为**人工待办**。
- 安装包**仍无 Authenticode / EV 签名**（SmartScreen 必警告）、macOS 未构建未公证、更新端点占位、minisign 为开发密钥。**发布就绪度仍为「否」，安装包不可对外分发，官网价格页不是实时报价。**

---

## 19. 主要用途与赛道选择（2026-11-20 23:00，研究型决策）

触发：用户要求「先搞清楚主要用途（阅读批注 / 格式转换 / 深度修改排版），基于市场规模和最终收益确定后再突出功能特色，与同赛道竞品对比取得相对优势」。完整论证见 `docs/primary-use-case-decision.md`，口径同步见 PRD §22.11。

### 19.1 性质与范围

**本轮是研究 + 决策，不是功能开发。** 新增 `docs/primary-use-case-decision.md`；追加 PRD §22.11、本节与工作区日志。**未改任何代码、配置、价格、官网文案、真机用例或安装包**——因此本节不记录任何构建/测试「通过」结论，只记录状态未变的核验。

| ID | 核验项 | 结果 |
|---|---|---|
| V-19.1 | `npm test` | **11 PASS / 0 FAIL**（回归确认文档改动未影响契约单测） |
| V-19.2 | 安装包 SHA-256 | `193ff0c4ce816a6b6735dc0f3bae21e45ce33cccb609a60fd3cdbab8c08c800e`，**与 §18 一致，未变** |
| V-19.3 | `python tests/verify-updater-signature.py` | ①文件签名 OK ②全局签名 OK ③1 bit 负向自检 OK，**未变** |
| V-19.4 | `config/product.json` 四字段与设备数 | `39 / 199 / 29 / 149 / 3`、`pending_confirmation=false`，**未变** |
| V-19.5 | 真机用例表 | 仍为 **60 条，全部未执行**（P0 40 / P1 20），**未变** |

### 19.2 结论摘要（供后续引用，不得写成已验证）

- **主要用途 = 「桌面离线、一次买断的 PDF 就地修订」**；阅读批注是形态层（不收费）、格式转换降为辅助层（不主打、不承诺保版式）、深度排版明确不做。
- 依据的一手证据是 PainRadar 新语料（300 样本 / 81 条有效痛点：付费与订阅 40%/3.8、编辑与表单 11%/3.6、格式转换与 OCR 4% 且补验强度回落 3.5）与旧语料（723 评论 / 333 原子）；**40% 是收费方式痛点，不是用途痛点**。
- 联网核实的 5 份 2026 市场规模报告给出 $3.20B / $5.29B / $5.51B / $6.30B / $4.8B(2024)、CAGR 11.15%–18.79%，口径互相矛盾 → **只作背景，不用于数值推算、不写进对外文案**。硬数据仅采用 Adobe SEC 10-Q（FY2026 Q3 BPC 订阅收入 $1.91B、总 ARR $27.50B）与 Gumroad 3.2% 商品页转化均值。
- 收益模型（$39）：直连净 $33.17/单、Discover $27.30/单；盈亏平衡 15 / 18 单；300 单净 $9,951 / $8,190；0.1 / 0.3 / 0.5 封支持邮件吃掉 3.8% / 11.3% / 18.8% 净收入。300 单需 9,375 商品页访客或约 312,500 次内容阅读 → **瓶颈是分发不是功能宽度**。
- 赛道重定义为「桌面离线、一次买断的轻量 PDF 修订工具」；L2 首项「预览画布框选文字 → 就地改字/批注/脱敏」升为 **P0**（主用途交互闭环），**仍未做**。

### 19.3 诚实边界

评分矩阵权重与分数是结构化判断、不是测量数据；3.2% 来自数字内容商品平台均值而非桌面软件实测；312,500 阅读量是量级估算不是预测；PRD §1.4 三假设、预购价格 A/B、自然流量占比**至今一条未验证**。**主要用途是定位决策，不等于市场已验证**；能证伪它的是预购定金转化与商品页 A/B，不是再一轮文档分析。发布就绪度仍为「否」，安装包不可对外分发。对外文案随后经用户确认「按新定位改写文案」并已全量落地，见 §20。


---

## 20. 按主用途改写对外文案（2026-11-20 23:10，用户确认「按新定位改写文案」）

触发：§19 的主用途决策落盘后，用户在四个选项中确认「按新定位改写文案（推荐）」。决策依据 `docs/primary-use-case-decision.md` §5，本节记录**实际改动与复验**。

### 20.1 改动范围（仅对外口径，不含代码）

| 组 | 文件 | 改动 |
|---|---|---|
| **官网首页** | `site/index.html` / `site/index.zh.html` | meta description 与首屏 lead 从「15 个工具平铺」改为主用途主线（发现错字 → 原文件直接改 → 不转 Word / 不订 Acrobat / 不上传）；**主标题保持定位 v2 原样**；新增「主用途四层」section（收费核心 / 形态层 / 辅助层）；「What it does」标题改为「为就地修订而生，附带十五个工具」并在 intro 明确 15 工具是配套；「Why another PDF tool?」的 quote 改为「把一件事做扎实」 |
| **官网 FAQ** | `site/faq.html` | 新增 **Q20** 主用途是什么、**Q21** Acrobat Reader 免费为何还要付钱（Reader 改已有文字需付费方案）；Q16 明确承认「PDFgear 免费且能就地改字」；顶部 lead 引用改为 16 与 21 |
| **官网其余** | `site/pricing.html` | hero 增加一句「你付钱买的是什么」；`site/privacy.html` / `site/verify.html` 不含产品定位首屏、口径不冲突，**未改动**（6 页口径校验均通过） |
| **文案板** | `docs/requirements/copy.md`、`E:/PainRadar/docs/pdf-launch/copy.md` | §1 加主用途注记；§2 副标题改主线；§3 加「不逼你转格式」可演示卖点；§4 改为四层定位表 + 15 工具配套清单，并标注「不宣传尚未做的框选交互」；§7 新增 PDFgear / Acrobat Reader 两条反对意见；§9 不写清单扩三项；§10 加 2 条长尾词。两份正文逐行一致（仅顶部 2 行注记差异） |
| **落地页** | `docs/requirements/landing{,.zh}.html`、`E:/PainRadar/docs/pdf-launch/landing{,.zh}.html` | meta description、hero lead 同步主线；header 后新增「我们到底做什么 / What it is built for」section；「能做什么 / What it does」标题改为「附带什么——十五个配套工具」 |
| **其它文案** | `posts.md`、两份 `preorder.md` | 产品介绍段与「功能会不会很少」回应改为主用途优先、15 工具为配套；posts 的 37% 旧语料论证**保持原样** |
| **需求文档** | 两份 `PRD.md` | §2 增加主用途条；项目内 §21 补信息层级调整说明、§22.11 更新为「已落地」；参考 PRD §21 加历史口径注记 |
| **决策文档** | `docs/primary-use-case-decision.md` | §5 从「本轮不落地，待用户确认」更新为「已全量执行」，并写明刻意不宣传框选交互 |

**刻意不写的（本节记录 §20 当时的状态）**：L2 首项「预览画布框选文字 → 直接改字 / 批注 / 脱敏」尚未落地，所有对外表述只写「在原文件上就地改掉」这一结果；不写「没有免费替代」「功能全面超越」「在页面上点选即改」「零网络请求」。**该项随后以 F18 落地定位闭环 v1，见 §21；「点选即改 / 框选后立即改写」仍属禁写。**

### 20.2 复验结果（本轮实跑）

| ID | 命令 / 检查 | 结果 |
|---|---|---|
| V-20.1 | 10 份 HTML 标签栈平衡 + 站内相对链接存在 | **10/10 PASS**（FAQ 31 链接、中英首页各 26、定价 20 等） |
| V-20.2 | 旧首屏口径 `15 个本地工具：合并、拆分、压缩` / `Fifteen local PDF tools` 残留 | **0 / 0** |
| V-20.3 | 主标题 `Buy once. Works offline. No subscription.` / `一次买断的 PDF 工具` | 6 份首页/落地页全部保留 |
| V-20.4 | 禁止表述扫描（对外 HTML） | 「没有免费替代 / 框选即改 / 在页面上点选即改 / 零网络请求 / 完全离线 / 永久免费更新」**均 0**；「功能全面超越」1 处为 `index.zh.html` 的**否定式**「不宣称『功能全面超越』」，合规保留 |
| V-20.5 | 两份 `copy.md` 规范化正文 diff | **4 行差异 = 顶部 2 行注记 ×2**（与定位 v2 时期一致），正文逐行一致 |
| V-20.6 | `npm test` | **11 PASS / 0 FAIL** |
| V-20.7 | `npm run build` | **通过**，62 modules，963 ms |
| V-20.8 | 安装包 SHA-256 / 更新签名 / 价格四字段 | `193ff0c4…800e` **未变**；①文件签名 ②全局签名 ③1 bit 负向自检 OK；`39/199/29/149/3`、`pending=false` **未变** |

### 20.3 诚实边界

- 本轮**只改对外文案与口径文档**，未改代码、配置、价格、真机用例、安装包；因此没有新的功能验收结论。
- 文案描述的能力（F17 阅读视图、F10 就地改字、F15 真脱敏等）处于「代码与契约单测通过、**真机未验**」状态；真机 60 条用例仍全部未执行，F17 从未在真实窗口渲染过一页 PDF。对外文案与官网既有的 15 工具表述处于同一证据等级。
- 新增 Q21 的 Acrobat 价格 `$239.88/年` 与「Reader 不能改已有文字」来自 2026-11-20 联网复核（PDFgear 官方对比表 + 第三方评测 + 项目内竞品表），**属竞品价格，上线前需按 `config/product.json` 的 manual_todo 复核**。
- 主用途仍是**未经市场验证的定位决策**：PRD §1.4 三假设、预购价格 A/B、自然流量占比一条未验证；能证伪它的是商品页转化，不是文案改写。
- **发布就绪度仍为「否」**：无 Authenticode/EV、macOS 未构建未公证、更新端点占位、安装包不可对外分发；官网价格页不是实时报价。

---

## 21. F18 画布框选修订联动（2026-11-20 23:27，用户「好，开工吧」）

触发：§19/§20 把「预览画布框选文字 → 就地改字 / 批注 / 脱敏」定为主用途的 **P0 交互闭环**，用户确认开工。本节记录实际代码改动与复验；决策口径见 `docs/requirements/PRD.md` §22.12、`CONTRACT.md`、`docs/primary-use-case-decision.md` §5。

### 21.1 设计决定（先把容易做假的地方说清）

| 决定 | 理由 |
|---|---|
| **框选只做「定位」**，执行仍走既有 F10/F11/F15 与 `execute()` 外壳 | 现有写引擎是按**文本查找**的，若把框选文字直接当 `find`，同页重复文字会被一起改掉。与其重写三个引擎的「按矩形单次修改」，本轮选择不撒谎：框选 → 高亮 → 带入选中的文字与页码 → 用户确认 → 点底部「执行」；授权 / 进度 / 原子提交 / 写后读回校验完全不变 |
| **唯一性守卫** | 选中文字在本页出现 >1 次或整页文字层找不到时，禁止带入执行并给「扩到整行」/手动修正，避免静默误改 |
| ~~旋转页显式禁用~~ → **已支持（§32，2026-11-21）** | 当时实测：随包 PDFium 在 `/Rotate 90` 页上返回内容流坐标，与旋转后渲染图错位，故一度禁用。§32 用 `geometry` 双向坐标变换修复：F18 把字符框变换到显示坐标，F19 把选区逆变换回内容流坐标；查询仍不落盘 |
| **扫描件 / 超 6 万字符页 / 浏览器预览 / 结果预览态禁用** | 无文字层不猜坐标；超大页不撑爆 IPC；预览模式不伪造交互；结果态不制造「看着结果改了原文件」的错位 |
| **未做（仍属 L2）** | 按矩形精确只改框选那一处、框选即改、预览内直接编辑文字对象。**不得对外写「点选即改」「框选后立即改写」「功能不弱于 UPDF」** |

### 21.2 改动清单

| 层 | 文件 | 改动 |
|---|---|---|
| Rust 引擎 | `src-tauri/src/engine/render.rs` | 新增 `TextCharBox` / `PageTextLayout` DTO 与 `PdfDoc::page_text_layout()`（PDFium `FPDFText_GetCharBox`，PDF pt 左下原点、整页文字层、6 万字符上限、旋转检测）；新增 4 个真实 PDF 单测；把测试用 PDFium 实例收敛为唯一 `OnceLock`（`engine::extras` 端到端测试委托复用） |
| Rust 命令 | `src-tauri/src/commands/viewer.rs` / `mod.rs` / `lib.rs` | 新增只读查询命令 `get_page_text_layout`（`PageTextLayoutRequest` → `PageTextLayout`），注册进 `generate_handler!`；DTO camelCase 单测 |
| 前端几何 | `src/lib/selection.ts`（新建） | 屏幕↔pt 换算、矩形相交、字符命中、选中文字拼接、重复计数、按行分组/扩行、高亮百分比、唯一性守卫；纯函数无 DOM，Node 22 可直接 import 单测 |
| 前端契约 | `src/lib/types.ts` / `backend.ts` | F18 请求/响应 DTO 镜像、`COMMANDS.getPageTextLayout`、`getPageTextLayout()` 封装（非 Tauri 返回 `null`） |
| 前端组件 | `src/components/PdfPreview.tsx` | 图片外覆可拖选画布、命中字符高亮、拖拽 marquee、选中文字可编辑、出现次数与唯一性警告、「扩到整行」、改字/批注/脱敏三个「填入参数」动作、旋转/扫描/预览/结果态禁用提示、结果预览徽标与「返回原文件」 |
| 前端主控 | `src/App.tsx` | `applySelection()` 切工具并填 `find` / `pages` / `replace` / `kind` / `terms`；任务成功后把输出 PDF 自动送进预览画布；新增输入时清除结果态 |
| 样式 | `src/styles.css` | `.viewer-page-wrap` / `.viewer-hit` / `.viewer-marquee` / `.viewer-selection-bar` 等，浅暗双主题变量、焦点环；修复图片外层 `max-width` 缩放 |
| 测试 | `tests/selection-geometry.test.mjs`（新建 10 用例）、`tests/rename-contract.test.mjs`（+1 F18 契约用例）、`tests/ui-interaction-smoke.mjs`（+3 F18 预览模式断言；进度断言改轮询）、`tests/capture-ui-v2.mjs`（加 `Page.bringToFront`）、`package.json`（test 脚本纳入几何单测） | 见 21.3 |
| 文档/口径 | `CONTRACT.md`、两份 PRD（新增 §22.12）、`docs/primary-use-case-decision.md` §5、`docs/updf-parity-decision.md`、`docs/updf-gap-analysis.md`、`README.md`、两份 `copy.md`、`site/index.html` / `index.zh.html` / `faq.html`（新增 Q22）、`docs/real-device-test-plan.md`、`tests/out/real-device-results.csv`（新增 `RD-G0-09/10/11`）、`scripts/preflight-real-device.py`（新哈希） | §20 中「框选交互尚未落地」的口径已被本节取代；可写「拖选文字 → 带入文字与页码」，仍禁写「点选即改 / 框选后立即改写」 |

### 21.3 复验结果（本轮实跑）

| ID | 检查 | 结果 |
|---|---|---|
| V-21.1 | `npm test` | **22 PASS / 0 FAIL**（原 11 契约 → 12；新增 10 个几何单测） |
| V-21.2 | `npm run build` | **通过**，`tsc --noEmit` 0 error，63 modules |
| V-21.3 | `cargo check` / `cargo test --lib` | CHECK_OK；**38 PASS / 0 FAIL**（原 33 + viewer DTO 1 + render 4） |
| V-21.4 | `node tests/ui-interaction-smoke.mjs` | **23 PASS / 0 FAIL**（新增 3 条 F18 断言）。首次 22/1：mock 进度计时器受 HMR/CPU 抖动误报，已把固定 `sleep(700)` 改为最多 2 s 轮询，复跑全绿 |
| V-21.5 | `python tests/acceptance.py --smoke` | 无 FAIL，14 项 `SKIP (manual)` |
| V-21.6 | 官网 6 页 + 4 份落地页 HTML 标签栈 / 站内链接 | **10/10 PASS**；FAQ 编号 1..22 连续 |
| V-21.7 | 两份 `copy.md` 规范化正文 diff | **4 行 = 顶部 2 行注记 ×2**，正文逐行一致 |
| V-21.8 | 禁止表述 / 过时口径扫描 | 「没有免费替代 / 框选即改 / 点选即改 / 零网络请求 / 完全离线 / 永久免费更新 / 『框选交互未落地』」对外文件 **均 0** |
| V-21.9 | `python scripts/preflight-real-device.py` | **PASS 28 / WARN 2 / BLOCK 0 / FAIL 0**（哈希基线已更新） |
| V-21.10 | `npx tauri build --bundles nsis` → `rename-bundles.mjs --windows-only --require` | **成功**，release 1m35s，`TAURI_BUILD_EXIT=0`，日志 `tests/out/f18-build.log` |
| V-21.11 | `python tests/verify-updater-signature.py` | **① 文件签名 ② 全局签名 ③ 1 bit 负向自检** 三段 OK；key id `654e340d1fe4b8e5` 匹配 |
| V-21.12 | `installer.nsi` 的 `traineddata` 行 | **4 条**（`CreateDirectory` + 2 个 `File /a` + 卸载 `Delete`），OCR 资源仍随包 |
| V-21.13 | 双主题截图 | `tests/out/updf-ui-light.png` **167,129 B**、`updf-ui-dark.png` **175,536 B**；可见 F18 框选提示条与「框选不可用」页脚。截图脚本修复：加 `Page.bringToFront`（后台标签会让 `captureScreenshot` 挂起）；`fromSurface:false` 会产生空白图，已回退默认表面捕获 |
| V-21.14 | 真机用例表 | **63 条**（原 60 + `RD-G0-09/10/11`，P0 42 / P1 21），`result` 列 **63 条全空** |

### 21.4 当前产物

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,658,136 B（30.19 MiB）**，SHA-256 `0fe2fe3c…3757`（**F18 产物，已被 §23 的 F19 包取代**） |
| 同目录 `.sig` | 440 B，三段验签通过 |
| 未打包主程序 `src-tauri/target/release/payoncepdf.exe` | 含 F17 + F18 三个查询命令 |

旧哈希 `193ff0c4…800e` **已失效**（增量约 17 KB，来自 F18 前后端代码与资源）。价格 `$39 / ¥199`、早鸟 `$29 / ¥149`、3 台设备、`pending=false` **均未改动**。

### 21.5 诚实边界（务必保留）

- **F18 只交付了「定位闭环 v1」**：按矩形精确单次修改、框选即改、预览内文字对象编辑仍未做；L2 其余条目也仍未做。不能写「功能不弱于 UPDF」。
- **真机 63 条用例仍全部未执行**，Tauri GUI 从未启动，F18 **从未在真实窗口拖选过一次**；F17 也从未在真实窗口渲染过一页 PDF。截图来自浏览器预览模式 + CSS 示意纸，**不可作为真机证据**。
  - **注（2026-11-21 更新，见 §22）**：GUI 已实际启动，且 F17 已在真实窗口渲染出真实 PDF、F18 状态已变为「可框选文字」；但**框选手势、高亮层与动作条仍未在真实窗口验证**，63 条真机用例仍未人工执行。本节其余结论不变。
- Rust 新增的 4 个 render 单测为 `#[cfg(test)]`，在 NSIS 构建完成后加入，不影响 release 产物字节；`cargo check` / `cargo test --lib` 已在最终源码上复验通过。
- `.sig` 的 trusted comment 仍记录 Tauri 默认名 `PayOnce PDF_1.0.0_x64-setup.exe`，与连字符发行名不同（字节未变、验签通过），上线前建议统一。
- 3 设备授权仍只在本地 mock 用内存台账实现；生产激活服务持久化、Gumroad 价格与折扣码、国内 ¥199 渠道、竞品官网价格复核均为人工待办。
- **发布就绪度仍为「否」**：无 Authenticode/EV 签名（SmartScreen 必警告）、macOS 未构建未公证、更新端点占位、minisign 为开发密钥。**安装包不可对外分发，官网价格页不是实时报价。**

---

## 22. 真实窗口首次运行验证（2026-11-21 09:30，用户「你打开给我看一下」「继续」）

### 22.1 目的与背景

§21.5 曾明确记录「Tauri GUI 从未启动」「F17 从未在真实窗口渲染过一页 PDF」。本节把这两条从「未验证」推进到「已验证」，同时把过程中踩到的环境限制固化下来，供后续真机测试复用。

### 22.2 方法

1. 直接运行 `src-tauri/target/release/payoncepdf.exe`（F18 release 产物），**不安装 NSIS 包**。`target/release/` 下 PDFium 等 DLL、`qpdf.exe`、`tesseract.exe`、`resources/tessdata/{eng,chi_sim}.traineddata` 齐全，可独立启动。
2. 合成鼠标/键盘在本环境被 WebView2 忽略（`move` 生效、`down/up` 无效，只出现 hover tooltip），CDP 端口也被 Tauri 的 browser args 覆盖，因此改用 **UI Automation**：
   - `comtypes.client.GetModule("UIAutomationCore.dll")` 生成类型库 → `CUIAutomation` → 遍历 `Button` 控件，按子串匹配「添加文件」→ `IUIAutomationInvokePattern.Invoke()` 打开对话框；
   - 先用 `IUIAutomationValuePattern.SetValue()` 写入 `E:\PayOncePDF\tests\fixtures\text_cjk_special.pdf`（`SetWindowText` 不更新 IFileDialog 内部状态，会导致确认无效）；
   - 对对话框发 `WM_COMMAND(IDOK)` 完成确认（UIA 的 `Invoke` 与 `BM_CLICK` 均未能关闭对话框）。
3. 脚本：`scripts/uia-confirm-load.py`（可重复执行，窗口随脚本退出被回收）。

### 22.3 验证结果

| 编号 | 检查项 | 结果 |
|---|---|---|
| V-22.1 | release 二进制窗口启动 | **通过**，标题 `PayOnce PDF`，无需安装包 |
| V-22.2 | 「添加文件」对话框打开与文件注入 | **通过**，文件名框填入完整路径、对话框正常关闭 |
| V-22.3 | 文件列表 | **通过**，出现 `text_cjk_special.pdf`，`1 个文件`，状态「待处理」 |
| V-22.4 | **F17 真实渲染** | **通过**，文档预览渲染出真实页面；标题栏 `text_cjk_special.pdf`、`1 / 1`、`100%` |
| V-22.5 | 页面尺寸与 DPI | **通过**，状态行 `第 1 / 1 页 纸张 216 × 279 mm 96 DPI` |
| V-22.6 | **F18 文字层加载** | **通过**，状态行由「框选不可用」变为 **「可框选文字」**；提示语变为「在页面上按住鼠标拖过一段文字，即可选中并带入右侧工具。」 |
| V-22.7 | 右侧参数联动 | **通过**，「合并顺序（1 个文件）」自动更新 |
| V-22.8 | 框选手势 / 高亮 / 动作条 | **未通过（环境限制）**，合成鼠标的 press–drag–release 被 WebView2 忽略，仅出现文件名 tooltip |
| V-22.9 | 安装包字节 | **未变**，SHA-256 仍 `0fe2fe3c…3757`；本节未重建任何产物 |

证据：`tests/out/real-window-f17-loaded.png`（344,052 B）、`tests/out/real-window-f18-selected.png`（345,244 B，无高亮）。

### 22.4 环境坑（后续复用必看）

- **`Add-Type`（.NET 运行时编译）被安全策略拦截** → 改用 Python `ctypes` + `PIL.ImageGrab` 抓窗口矩形。
- **`SendInput` 返回 0**：自定义 `INPUT` 结构多加了 `_pad` 字段，`sizeof` 不是 Windows 期望的 **40 字节**；去掉后返回 3。
- **`win32com` 不可用于 UIA**：`CUIAutomation` 是纯 IUnknown 接口，`Dispatch` 报「不支持此接口」；必须 `comtypes` + `GetModule` 生成类型库。
- **`FindFirst` 按名字精确匹配会失败**：WebView2 的 UIA 树在页面挂载后约 5–10 秒才稳定，且中文名可能带不可见字符；改为**遍历 Button 控件 + 子串匹配 + 轮询重试**。
- **IFileDialog 的窗口类不是 `#32770`**（本项目实测），要按窗口名「添加文件」查找。
- **`SetWindowText` 到内部 Edit 不更新 IFileDialog 状态**，点击「打开」不生效；必须用 `ValuePattern.SetValue`，再用 `WM_COMMAND(IDOK)` 确认。
- **窗口进程随工具会话被回收**（两次 `run_in_background` / `Start-Process` 均在后续调用后 `NOT_RUNNING`，exit 0）。要常驻需用户自己双击 exe 或安装 NSIS 包。

### 22.5 诚实边界

- **只验证到「渲染 + 文字层就绪」**：框选手势、字符高亮层、动作条、唯一性守卫在真实窗口**仍未验证**，因为本环境无法注入有效鼠标按下事件。
- **63 条真机用例仍未人工执行**；本节是自动化冒烟，不等于 RD-G0-01…11 通过。
- **fixture 的 CJK 段本身是 mojibake**（`text_cjk_special.pdf` 生成时的编码问题，显示为 `å,æ-æ¡å…`），不是渲染缺陷；验证中文渲染需换正常中文 PDF。
- 截图是真实 Tauri 窗口（非浏览器预览、非 CSS 示意纸），但**不能替代真机人工验收**。
- **发布就绪度仍为「否」**：签名、公证、更新端点、生产激活服务均未完成；安装包不可对外分发。

## 23. F19 按矩形精确替换（2026-11-21 09:40，用户「继续」）

### 23.1 为什么做这一节

§21 的 F18 只交付「定位闭环」：选中文字 → 填入参数 → 仍由 F10 按**文本查找**执行，所以同页出现两次的文字必须靠唯一性守卫拦住。用户同意继续后，把「按矩形精确只改框选那一处」落地为 **F19**，从根本上绕开「按文本查找」的局限。

### 23.2 设计决定

| 决定 | 理由 |
|---|---|
| 新增任务命令 `edit_text_at_rect`，不改造 F10 | 两者语义不同（位置定位 vs 全文查找）。共用 `execute()` 外壳，授权/进度/取消/原子提交/写后读回不复制第二套 |
| 定位口径与前端完全一致：中心点在框内，或字符框被覆盖 ≥ 40% | 避免「界面高亮了 A，引擎改了 B」 |
| 三重失败即拒绝 | ① `/Rotate != 0`：字符坐标是内容流坐标，与渲染图不一致；② 文字层字符序列 ≠ 内容流文本对象序列（文字在 XObject / 顺序交错）：字符下标无法安全映射到对象；③ 跨行：合并两行会破坏排版 |
| 换行符先剔除再对齐 | 实测 PDFium 文本页会插入内容流里不存在的换行符（夹具 18 字符被读成 20 个），不剔除就永远对不齐 |
| 写后读回：原文出现次数恰好减 1 | 覆盖「静默写空格 / 内容流未提交 / 字体子集缺字形」三类假成功；替换文字包含原文时不做减法断言 |
| 输出同目录 `*_edited.pdf` | 不覆盖输入；同名由 `resolve_output_path` 自增 |

### 23.3 改动清单

| 文件 | 改动 |
|---|---|
| `src-tauri/src/engine/extras.rs` | 新增 `replace_text_at_rect` / `verify_rect_edit` / `pick_chars_by_rect` / `char_cover_ratio` / `count_str`、`MAX_RECT_CHARS`；新增 4 个真实 PDF / 纯函数单测 |
| `src-tauri/src/commands/mod.rs` | `RectEditRequest`（camelCase，`left/bottom/right/top` 为 PDF pt） |
| `src-tauri/src/commands/extras.rs` | `edit_text_at_rect` 命令，rect 自动归一（min/max），复用 `commit_one` |
| `src-tauri/src/lib.rs` | 注册 `edit_text_at_rect` |
| `src/lib/selection.ts` | `unionRect`（扩行后矩形同步，F19 与界面选中必须一致） |
| `src/lib/types.ts` / `backend.ts` | `RectEditRequest`、`editTextAtRect`、加入 `CommandPayload` 联合 |
| `src/components/PdfPreview.tsx` | 保存 `selRect`、`rectEditReady`、「直接改这一处」按钮、重复文字提示改为指向 F19 |
| `src/App.tsx` | 抽出 `startJob` 统一外壳；新增 `runRectEdit` 与 `editedOutputPath` |
| `CONTRACT.md` / `docs/requirements/PRD.md` | 命令表 + §22.13；§22.12「未做」改为已交付 |
| `docs/real-device-test-plan.md` / `tests/out/real-device-results.csv` | 新增 `RD-G0-12` / `RD-G0-13`，用例总数 **63 → 65**（P0 43 / P1 22） |

### 23.4 复验

| 编号 | 项目 | 结果 |
|---|---|---|
| V-23.1 | `npm test` | **24 PASS / 0 FAIL**（原 22 + `unionRect` 1 + F19 契约 1） |
| V-23.2 | `npm run build` | 63 modules，tsc 无错 |
| V-23.3 | `cargo check` | 通过，无 warning |
| V-23.4 | `cargo test --lib` | **41 PASS / 0 FAIL**（38 + 3 个 F19 单测；`rect_pick_matches_frontend_rule` 为纯函数） |
| V-23.5 | F19 真实 PDF 端到端 | 一页两处 `Draft`（两个独立文本对象）→ 框选第一处 → 改为 `Final`：输出 `Final one` + `Draft two`，`Draft` 计数 2→1，原文件字节不变，`qpdf --check` 通过 |
| V-23.6 | F19 失败即拒绝 | 空白框选 / 预览文字不一致 / 跨行框选三种都报错且**不产生输出文件** |
| V-23.7 | UI 交互冒烟（CDP，预览模式） | **23 PASS / 0 FAIL** |
| V-23.8 | 真机用例表 | **65 条**（P0 43 / P1 22），`result` 列 **65 条全空** |
| V-23.9 | NSIS 重建 + 验签 | `TAURI_BUILD_EXIT=0`，release 2m35s；产物 31,659,719 B，SHA-256 `4bbd7d4a…2d08`；三段验签（文件 / 全局 / 1 bit 负向）全 OK |
| V-23.10 | 真机预检 | 更新 `INSTALLER_SHA256` 后重跑：**PASS 26 / WARN 4 / BLOCK 0 / FAIL 0**（WARN 为 mock 8787 未起、试用目录非首次、旧品牌包残留、CDP 相关） |

### 23.7 真机首测缺陷修复：空格退化字符框误判（2026-11-21 10:19，用户实测反馈）

**用户实测**：真机窗口拖选一行 → 高亮正常 → 点「直接改这一处」→ **没有改成新字**（后台也没有 `_edited.pdf` 产物）。

**根因**（用 `tests/fixtures/text_cjk_special.pdf` 逐行复现确认）：PDFium 给空格等字符返回的是**退化字符框**——高仅 1~2pt、垂直中心比字母低 3pt+。F19 的「行内垂直中心连续性」闸门用*当前字符自己的框高*算容差（`h*0.6`），空格容差仅 1.2pt，字母→空格的 3.6pt 偏差直接触发「框选跨了多行」误报。**任何含空格的选区都被拒绝**，而英文句子几乎必含空格，所以真机首测必然失败。这不是坐标换算问题（高亮正常证明 F18 口径正确）。

**修复**（`engine/extras.rs::replace_text_at_rect`）：
1. 连续性检查**跳过退化字符框**（高 < 1pt 或宽 ≤ 0.01pt），容差改为 `0.6 × max(相邻两字符框高， 2pt 下限)`；
2. 「框选不连续」检查放宽：中间缺口**只含空白字符**仍算连续（真实拖选经常套不住空格的退化中心点），缺口含非空白才报错。

**回归验证**（新增 `e2e_rect_edit_fixture_line_with_spaces`，cargo test --lib **43/0**）：
- 含空格整行拖选 → 成功，`old_text` 为完整句子，输出改字正确，特殊符号行保留，原文件字节不变；
- 特殊符号行 `© ® ± × ÷ § ¶ €` → 成功；
- 紧拖选（细带套不住下伸字母 y/p）→ 明确报「框选不连续」且不写文件（保守拒绝，可接受）；
- mojibake 行（原始 UTF-8 字节按 Latin-1 解码）→ 被字体/连续性闸门拒绝，不写文件（该行本就不可安全编辑）。
- `npm test` 24/0、`npm run build` 通过；安装包随即重建（见 §23.8）。

### 23.6 产物

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,659,719 B（30.19 MiB）**，SHA-256 `b16bd927cae5bc23e76357261be8a3d782fa1eca56517ba8d1c6a60a8b5c18e9` |
| 同目录 `.sig` | 440 B；key id `654e340d1fe4b8e5`；文件签名 / 全局签名 / 1 bit 负向自检全 OK |
| `scripts/preflight-real-device.py` | `INSTALLER_SHA256` 已更新为新包；重跑结果见下 |

预检重跑：**PASS 26 / WARN 4 / BLOCK 0 / FAIL 0**；旧 F18 包哈希 `0fe2fe3c…3757` 已失效。**注意：此包不含 §23.7 的空格修复，已被 §23.8 的新包取代。**
`.sig` 的 trusted comment 仍记录 Tauri 默认文件名 `PayOnce PDF_1.0.0_x64-setup.exe`（与连字符发行名不同，但验签覆盖的是文件字节本身，故仍通过）；上线前建议统一命名。

### 23.8 修复后的产物（§23.7 空格修复已包含）——**已被 §25.7 取代**

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,654,645 B（30.19 MiB）**，SHA-256 `b16bd927cae5bc23e76357261be8a3d782fa1eca56517ba8d1c6a60a8b5c18e9` |
| 同目录 `.sig` | 440 B；文件签名 / 全局签名 / 1 bit 负向自检全 OK |
| `scripts/preflight-real-device.py` | `INSTALLER_SHA256` 已更新为 `b16bd927…18e9` |

**旧包哈希 `4bbd7d4a…2d08`（§23.6）不含空格修复，已失效，不得用于验证 F19。**

### 23.5 诚实边界（必须延续）

- **F19 从未在真实 Tauri 窗口跑过一次**：单测用的是代码生成的 PDF 与离线 `JobCtx`，不是安装后的桌面窗口；`RD-G0-12/13` 未执行。
- **F18 的框选手势仍未在真实窗口验证**（§22.5）：F19 的入口按钮依赖框选成功，真机拖选这一步没有验证过。
- **只支持同一行内连续文字**：跨行、跨段落、按矩形改字体/字号/颜色、删除整段后回流重排都未做，遇到时明确报错而不是猜。
- **依赖「文字层顺序 == 内容流顺序」**：文字被包在 Form XObject 里的 PDF（部分 Word / 排版软件导出）会被拒绝，提示改用「按文本替换」——这是保守策略，不是能力。
- **浏览器预览模式完全不可用**（无本地引擎）：F19 与 F18 一样在预览模式禁用。
- 价格 `$39 / ¥199`、早鸟 `$29 / ¥149`、3 台设备**未改动**；安装包已随本轮源码重建（见 §23.6），但**仍未做 Authenticode/EV 签名与真机安装验收**。
- **发布就绪度仍为「否」**：无 Authenticode/EV 签名、macOS 未构建未公证、更新端点占位、minisign 为开发密钥；安装包不可对外分发。

## 24. 真实中文样本与中文改字验证（2026-11-21 15:08，用户反馈 fixture 乱码）

### 24.1 用户看到的乱码不是渲染缺陷

用户在真机导入 `tests/fixtures/text_cjk_special.pdf`，第 3 行显示乱码。复现确认：
- **第 2 行 `© ® ± × ÷ § ¶ €` 渲染正常**（WinAnsi 八进制转义，标准符号）；
- **只有第 3 行中文是乱码**，原因是该 fixture 由 `generate-fixtures.py`（标准库限定）生成，**没有内嵌 CJK 字体**，中文按原始 UTF-8 字节写进 PDF 文字串，PDFium 按 WinAnsi/Latin-1 解码 → 必然乱码（`manifest.json` 的 `note` 与脚本 `FONT_LIMIT_NOTE` 均已注明）。

### 24.2 真正的问题：我们没有真实中文 PDF 样本

这意味着「中文 PDF 能否正确渲染」「能否就地改中文」**从未被验证过**。本轮补上：

| 项 | 内容 |
|---|---|
| 新样本 | `tests/fixtures/text_cjk_real.pdf`，8,788 B：内嵌 SimHei 子集（46 字形，9.6 KB）、Type0 / CIDFontType2 / Identity-H、**带 ToUnicode CMap**；三行真实中文（含中英混排与数字） |
| 生成脚本 | `scripts/make-cjk-fixture.py`（开发期工具，需 `fontTools` + 系统 CJK 字体；核心 fixture 生成器仍保持纯标准库红线） |
| 字体来源 | 用户本机 `C:/Windows/Fonts/simhei.ttf` 子集，**仅本机测试用，不随安装包分发**；缺失字体时脚本明确报错 |

### 24.3 验证结果

| 编号 | 项目 | 结果 |
|---|---|---|
| V-24.1 | 文字层还原（PDFium） | `"中文测试：就地改错字，不转 Word\r\n合同编号：2026 年第 018 号\r\n付款期限为 30 日，逾期按日计息。"`，**无替换符、无乱码** |
| V-24.2 | 渲染（150 DPI） | PNG 95,973 B / 1240×1754，目视三行中文清晰、无豆腐块 → `tests/out/cjk-real-render.png` |
| V-24.3 | F19 按位置改中文 | 框选 `30 日` → `45 日`：`old="30 日" chars=4`，输出文字层为 `付款期限为 45 日`，另两行（`中文测试…`、`合同编号：2026 年第 018 号`）原样保留，原文件字节不变 |
| V-24.4 | 改后渲染目视 | `tests/out/cjk-real-edited.png`（95,542 B）：第三行显示「付款期限为 45 日」，字形正确、无错位 |
| V-24.5 | 全量回归 | `cargo test --lib` **45 PASS / 0 FAIL**（43 + 中文渲染 1 + 中文改字 1） |

### 24.4 边界（诚实）

- **中文改字能成立的前提是字体里有目标字形**：本样本子集只含用到的字，替换成子集外的汉字时引擎会走「字体无法编码」路径报错（真实 PDF 若内嵌完整字体或 CID 字体通常可写；未内嵌的 PDF 仍会被拒绝）。
- 本轮**只加测试与样本，未改任何产品代码**：安装包仍是 §23.8 的 `b16bd927…18e9`，无需重建。
- 真机用例扩到 **66 条**（新增 `RD-G0-14` 中文渲染与按位置改字，P0），`result` 仍全空——**中文链路同样未经真机窗口验证**，目前只有引擎级 + 渲染目视证据。
- `text_cjk_special.pdf` **保留**：它仍用于验证 WinAnsi 特殊符号与「无字体时明确降级」的场景；其乱码行是预期行为，不再被当作中文测试样本。


## 25. F19 字符集：英文、科学符号、数学公式（2026-11-21 15:35，用户追问「这些能否改」）

### 25.1 为什么要单独验一轮

§24 只证明了「中文能改」。用户随即问「英文和一些科学符号或数学公式呢」。F19 的写入方式是 `text_obj.set_text()`——用**原文本对象的字体**重新编码，所以能不能改不取决于字符"看起来是什么"，而取决于三件事：字体里有没有这个字形、文字层和内容流能不能对齐、选区里的字号是否一致。这三条只能拿真实 PDF 跑，不能靠推理。

### 25.2 新样本：5 页，每页一个场景

`tests/fixtures/text_symbols.pdf`（32,861 B），由 `scripts/make-symbol-fixture.py` 生成（开发期工具，需 `fontTools`，用系统 `arial.ttf` 子集，仅本机测试、不分发）：

| 页 | 场景 | 构造 |
|---|---|---|
| 1 | 英文正文 | Helvetica 内置 + WinAnsi，**不嵌字体**（真实发票最常见的样子） |
| 2 | Latin-1 符号 | `Temp 25 ± 2 °C \| 5 µL \| 3 × 4 ÷ 2`，同样内置 WinAnsi |
| 3 | 数学符号 | `3 ≈ 3.14, x ≤ 5, y ≥ 2` / `∑ ∫ √π, Ω Δ`，内嵌 Arial 子集（Type0 / Identity-H / ToUnicode）。**子集故意不含 ≠** |
| 4 | 上下标 | `H₂O and x² + y² = z²`，小字号 + 基线偏移，每段是**独立文本对象** |
| 5 | 分式 | 分子 `1`、横线路径、分母 `2`，三个互不相干的对象 |

文字层还原优先确认（读不对就什么都不用谈）：5 页全部逐字正确，`± ° µ × ÷` 与 `≈ ≤ ≥ ∑ ∫ √ π Ω Δ` 都能被 PDFium 还原，上下标段落的**宽松框高 16.65 / 9.99**（正文 / 上标），分式两行文字层为 `"Result: 12..."`。

### 25.3 首轮实测：3 条通过，2 条暴露缺陷

| 场景 | 首轮结果 |
|---|---|
| 英文 `30`→`45` | **通过**，同页另一处 `30` 未被误改 |
| 数学符号 `3.14`→`2.72` | **通过**，`≈ ≤ ≥ ∑ ∫ √ π Ω` 原样 |
| 缺字形 `≈`→`≠` | **通过**（安全拒绝，删输出、原文件字节不变） |
| 分式跨行拖选 | **通过**（拒绝） |
| **Latin-1 `×`→`÷`** | **失败**：`第 2 页该处文本使用内置/未嵌入字体（Helvetica），无法写入中文等非 ASCII 文字` |
| **整段 `H2O`→`CO2`** | **失败**：被放行并执行——上标会被压成正文大小 |

### 25.4 两个缺陷的根因

**缺陷 1：Latin-1 被「非 ASCII」一刀切误拒。**
旧闸门是 `if new_text.chars().any(|c| !c.is_ascii())` → 要求字体「非内置且已嵌入」。但 `±` 是 0xB1、`÷` 是 0xF7，**WinAnsiEncoding 本来就覆盖 U+00A0–U+00FF**，内置 Helvetica 完全写得出来。更要命的是想区分「标准 14 字体」也判不准：`font.is_built_in()` 依赖字体名匹配，对内容流里的字体经常返回 `false`，实测就走进了「字体未嵌入」分支。

修法：判断依据改成「能不能真的写出这个字」——**已嵌入 → 放行**（缺字形由读回校验兜）；**未嵌入 → 只放行 Latin-1，并排除 Symbol/Dingbats**（它们是私有编码，不是 WinAnsi）。拿不准的交给写后读回，宁可报错也不写坏。

**缺陷 2：跨字号选区被放行，上标会被压平。**
原闸门只查「行内垂直中心连续性」。上下标的中心只比正文高 2.85pt，而容差是 `0.6 × 16.65 ≈ 10pt`，**根本拦不住**。一旦放行，`split_match` 会把整段新文字塞进首个对象（正文对象），沿用它 15pt 的字号，上标被压成正文大小，基线也被拉平——排版当场毁掉。

修法：新增**字号一致性闸门**，选区内最大/最小字高比 > 1.5 即拒绝，提示「只框同一字号；要改上标就单独框那个上标」。

这里有个必须记下来的判断：**用「宽松框」而不是「紧贴框」比字号**。紧贴框是字形墨迹，同一行里 `H` 与 `o` 的高度就能差 30%，拿它比会误伤正常文字；宽松框由字体 ascent/descent 决定，同一字号下所有字符恒等（实测正文一律 16.65、上标一律 9.99），才是可靠的字号代理。

### 25.5 修复后复验

| 编号 | 场景 | 结果 |
|---|---|---|
| V-25.1 | 英文 `30`→`45` | `"Payment due in 45 days"`，`Invoice No. 30` 不变 |
| V-25.2 | Latin-1 `×`→`÷` | `"3 ÷ 4 ÷ 2"`；改后渲染 `tests/out/symbol-latin1-edited.png`（69,246 B）目视确认 `÷` 有字形、无豆腐块 |
| V-25.3 | 数学符号 `3.14`→`2.72` | `"3 ≈ 2.72, x ≤ 5, y ≥ 2"`；渲染 `tests/out/symbol-math-edited.png`（60,200 B）目视 `∑ ∫ √π, Ω Δ` 全部正常 |
| V-25.4 | 缺字形 `≠` | 报「输出中读不到「≠」…已删除输出且未改动原文件」，输出文件不存在，原文件字节不变 |
| V-25.5 | 整段 `H2O`→`CO2` | 拒绝：`选区里有大小不同的字（最大 16.7pt / 最小 10.0pt，多半是上下标或脚注）` |
| V-25.6 | 单独上标 `2`→`3` | 成功：`"H3O and x2"`，上标没被压平 |
| V-25.7 | 分式跨行 | 拒绝：`框选跨了多行` |
| V-25.8 | 全量回归 | `cargo test --lib` **53 PASS / 0 FAIL**（45 + 8：1 个诊断 + 7 个场景） |

### 25.6 边界（诚实）

- **数学符号能否改，取决于 PDF 有没有内嵌含该字形的字体**。内嵌 → 能改；未嵌入 → 拒绝并说明原因。这不是实现偷懒，是 PDF 的客观约束：字体的 cmap 里没有这个字形，写进去就是空白。
- **公式整体改不了**。分式的分子分母是独立对象、横线是矢量路径；LaTeX 生成的公式更是几十个绝对定位的小对象。单个数字/符号可改，整式不行。
- **上下标只能单独改**。整段替换必然破坏排版，所以选择拒绝而不是"尽力而为"——这与「保排版、不回流」的产品定位一致。
- **Type3 字体、Symbol/Dingbats 编码、Form XObject 内的文字**仍会拒绝；这正是 F19 与"任意编辑"的差距。
- 本轮所有证据来自**引擎级测试 + 渲染目视**，**未在真实 Tauri 窗口人工验证**；真机用例扩到 **69 条**（新增 `RD-G0-15/16/17`），`result` 仍全空。

### 25.7 字符集修复的产物（已被 §26.7 取代）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,657,100 B（30.19 MiB）**，SHA-256 `1fa562fc666ba38f50514b0929ab5ea9098d1f7d427596b258e36ecc606bd669` |
| 同目录 `.sig` | 440 B；文件签名 / 全局签名 / 1 bit 负向自检全 OK |
| `scripts/preflight-real-device.py` | `INSTALLER_SHA256` 已更新为 `1fa562fc…d669` |

**哈希链路**：`0fe2fe3c…3757`（F18）→ `4bbd7d4a…2d08`（F19 初版，含空格缺陷）→ `b16bd927…18e9`（§23.8，含空格修复）→ **`1fa562fc…d669`（本轮，含 Latin-1 放宽 + 字号一致性闸门）**。前三者均已失效，不得用于验证。

> 注：§23.6 与 §23.8 记录的包大小分别为 31,659,719 B 与 31,654,645 B，但 SHA-256 同为 `b16bd927…18e9`——两个大小不可能对应同一份字节，其中一处是当时的记录笔误；以哈希为准。本轮记录的大小由 `os.path.getsize` 实测，与哈希同源。

**重建命令**（可复现）：
```bash
export CARGO_HOME=E:/PayOncePDF/.toolchain/cargo RUSTUP_HOME=E:/PayOncePDF/.toolchain/rustup
export PATH=/e/PayOncePDF/.toolchain/cargo/bin:$PATH CARGO_BUILD_JOBS=1 CARGO_INCREMENTAL=0
export TAURI_SIGNING_PRIVATE_KEY="$(cat .toolchain/payoncepdf-updater.key)"
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=oncepdf-dev-only
npm run tauri build && npm run bundle:rename
```

## 26. F19 连续编辑：同一份文档改多处（2026-11-21 16:27，用户实测反馈）

### 26.1 用户看到的现象

「导入 `tests/fixtures/text_symbols.pdf` 后测试发现：只能修改一次，如果这篇文档有多处修改怎么办？」——第一次改完再框选别处，要么报错，要么改动不生效/丢失。

### 26.2 根因

`runRectEdit` 里写的是 `const src = files[0]?.path`，**永远是用户导进来那份原始文件**。而第一次改完，`startJob` 已经把预览切到 `xxx_edited.pdf`。第二次框选时：

1. 矩形坐标来自 `xxx_edited.pdf` 的页面；
2. 引擎却打开 `text_symbols.pdf` 去按坐标找字符；
3. 两份文件内容已经不同 → 坐标错位 → `expected` 校验报「框选文字与预览不一致」，或者定位到别处改错；
4. 即使侥幸对上，输出也是从**原件**重新生成——第一处改动随即消失。

所以问题不在引擎，在「框选坐标属于哪个文件」和「拿哪个文件去改」不一致。

### 26.3 修法

| 层 | 改动 |
|---|---|
| 输入一致性 | `src = resultFile?.path ?? files[0]?.path`，与预览用的是同一个表达式，坐标与文件必然同源 |
| 单文件累积 | 新增 `chainedOutputPath`：源的 stem 属于本次会话产物就原样返回（原地写回），否则给出第一次的 `_edited` 名 |
| 覆盖许可 | `RectEditRequest` 新增 `in_place`；为真才走 `commit_one_chained`。前端只对会话内自己产出的路径置真 |
| 后端二次把关 | `commit_one_chained` 除「输入 == 输出」外，还要求文件名是 `*_edited` / `*_edited_N`（`looks_like_edited_stem`）。用户原件永远走不进来 |
| 预览刷新 | 原地更新时文件路径不变，`PdfPreview` 的加载 effect 不会自己重跑 → 新增 `reloadToken` 自增，三处 effect 都挂上 |
| 页码保持 | 预览重载时用 `lastPathRef` 区分「换文件」和「原地更新」，只有前者才 `setPage(1)`；否则每改一处就被弹回首页 |
| 中途不闪回 | `startJob` 新增 `keepPreview`，F19 传 true，任务执行期间不把预览退回输入文件 |

### 26.4 一个必须堵的安全缺口

最初想只靠文件名判断（stem 以 `_edited` 结尾就允许原地写），但这会误伤一类真实情况：**用户打开的文件本身就叫 `报告_edited.pdf`**（同事发来的、别处导出的）。按文件名判断就会直接覆盖他的原件。

所以改成「**只认本次会话自己生成的产物**」：前端 `ownEditedRef` 记录每次实际输出路径（含后端因重名自增后的 `_edited_1`），只有命中这个集合才置 `in_place`。用户自己带进来的同名文件不在集合里 → 仍另存为 `xxx_edited_edited.pdf`，原件不受伤。

### 26.5 验证

| 编号 | 用例 | 结果 |
|---|---|---|
| V-26.1 | 两轮连续改（`e2e_rect_edit_chained_two_rounds`） | 第一轮 `Invoice No. 30`→`60`，第二轮以结果为输入把 `Payment due in 30 days`→`45`；最终 `"Invoice No. 60 … Payment due in 45 days"`，两处都在，原件字节不变 |
| V-26.2 | 原地更新同一文件（`e2e_rect_edit_in_place_updates_same_file`） | 输入输出同为 `doc_edited.pdf`：更新成功，内容变为 `Invoice No. 60`，临时文件不留残影（验证 Windows 上覆盖已被 PDFium 读过的文件可行） |
| V-26.3 | 覆盖许可（`chained_commit_overwrites_own_edited_output`） | `doc_edited.pdf` 被新结果替换 |
| V-26.4 | 拒绝覆盖原件（`chained_commit_refuses_user_original`） | `report.pdf` 作为输出被拒，错误信息指明只允许 `_edited`，原件字节不变 |
| V-26.5 | 普通路径不受影响（`chained_commit_falls_back_for_new_output`） | 输入输出不同名时照旧另存 |
| V-26.6 | 文件名识别边界（`edited_stem_detection`） | `doc_edited`/`报告_edited`/`doc_edited_1` 通过；`report`/`_edited`/`doc_edited_x`/`doc_edited_` 拒绝 |
| V-26.7 | 全量回归 | `cargo test --lib` **59 PASS / 0 FAIL**；`npm test` 24/0；`npm run typecheck` 与 `npm run build` 通过 |

### 26.6 边界（诚实）

- **没有撤销**：每次点击都会立刻覆盖结果文件。需要中间版本请自行另存；真正的撤销栈未做。
- **同一份结果文件被外部改动**时，连续性无从判断：下一次仍按当前预览文件继续改，`expected` 校验会兜住明显的错位。
- **点「返回原文件」后再改** = 从原件重新开始，会另存为新文件（`_edited_edited.pdf` 或自增名），旧结果保留但不参与累积——这是有意为之，避免悄悄丢掉之前的改动。
- 连续编辑受 F19 全部既有闸门约束（单行、同字号、字体有字形；旋转页自 §32 起支持，走坐标变换 + 去旋转副本 + 写回 /Rotate），不因为「能连改」而放宽任何一条。
- 本轮仍是**引擎级 + 前端类型检查**的证据，**未在真实窗口人工验证**；真机用例 69 → **70 条**（`RD-G0-18`），`result` 仍全空。

### 26.7 连续编辑的产物（已被 §27.6 取代）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,655,896 B（30.19 MiB）**，SHA-256 `c9374790070d0f334892e9760fea999a31f69ebda3b0acd5e59a8b02663eb81d` |
| 同目录 `.sig` | 440 B；文件签名 / 全局签名 / 1 bit 负向自检全 OK |
| `scripts/preflight-real-device.py` | `INSTALLER_SHA256` 已更新为 `c9374790…b81d` |

**哈希链路**：`0fe2fe3c…3757` → `4bbd7d4a…2d08` → `b16bd927…18e9` → `1fa562fc…d669`（字符集）→ **`c9374790…b81d`（连续编辑）**。前四个均已失效。

> 构建过程中发现一次前端产物滞后：首次 `tauri build` 的 vite 阶段早于最后一处文案改动，导致包里缺一句提示。已用第二次 `tauri build` 重打包，并 `grep dist/assets/*.js` 确认新文案在包内（`index-KJdrrt4V.js`）。**这正是「构建产物要回读确认」的一个真实例子，记下来避免重犯。**

## 27. 真机回归：改完一处后画布不能再框选（2026-11-21 17:20，用户实测反馈）

### 27.1 现象

「改了 `Invoice No. 30 → 60` 后，鼠标就不能再选框了。」——第一处改成功，预览切到 `text_symbols_edited.pdf`，此后再拖鼠标没有任何反应，等于一次会话只能改一处。§26 修的是「改了不生效 / 改动丢失」，这一条是**连选都选不了**，两者表象不同、根因也不在同一层。

### 27.2 根因：一个为安全设置的闸门，挡住了自己要支持的流程

`PdfPreview` 里有这么一条：

```ts
const canSelect = ... && !resultMode; // 结果预览时源仍是输入文件，避免「看着结果、改了原文件」
```

本意是对的：任务成功后预览换成输出文件，而「填入改字 / 批注 / 脱敏」三个按钮填的是**文本查找**参数，执行时读的是 `files[0]`（原文件），一旦允许在结果上框选，用户就会看着结果、改到原件别处。

但 F19「直接改这一处」不是文本查找，它按**坐标**执行，而 §26 已把输入改成「当前预览的文件」。于是这条闸门对 F19 变成了纯阻碍：第一次改完 → `resultMode=true` → `canSelect=false` → 画布失去 `viewer-page-wrap-selectable`（连十字光标都没了），`onPointerDown` 直接 return。看起来就是「鼠标坏了」。

### 27.3 修法：区分「哪种结果」

| 层 | 改动 |
|---|---|
| 判定 | 新增纯函数 `canSelectOnCanvas()`（`src/lib/selection.ts`）：`!resultMode \|\| selectableInResult` |
| 来源 | `PdfPreview` 新增 `selectableInResult`：**只有本会话 F19 自己产出的 `*_edited`** 为真 |
| 记账 | `startJob` 新增 `chainableResult` 选项，与 `setResultFile` **同批**更新 `chainableResultPath`；晚一帧会让预览先按普通结果处理（禁用框选 + 重置页码） |
| 动作条 | 结果预览里隐藏「填入改字 / 批注 / 脱敏」——它们的目标是原文件，与当前预览不同源；只保留「直接改这一处」 |
| 页码 | 切到本次改字结果时**保留当前页码**；否则在第 5 页改完就被弹回第 1 页 |
| 替换词 | 每次任务成功清空「改成」输入框；不清的话上一处的 `60` 会留在框里，新选区一下手快就点成「把别的字改成 60」 |

安全边界没有放宽：压缩 / 拆分 / 合并 / 转换等输出仍然 `resultMode=true` 且 `selectableInResult=false`，照旧禁止框选；用户自己带进来的 `报告_edited.pdf` 不在会话产物集合里，同样不放行。

### 27.4 验证

| 编号 | 用例 | 结果 |
|---|---|---|
| V-27.1 | `canSelectOnCanvas` 普通预览 | 允许 |
| V-27.2 | 结果预览、非本会话产物 | 拒绝（保持原有防错位语义） |
| V-27.3 | 结果预览、本会话 `_edited` | **允许**（本次修复点） |
| V-27.4 | 有图但文字层加载中 / 无文字层 / 无图 / 不可预览 | 一律拒绝 |
| V-27.5 | 前端全量 | `node --test` **28 PASS / 0 FAIL**（24 → 28）；`tsc --noEmit` 通过 |

### 27.5 边界

- 仍是**前端逻辑 + 纯函数单测**，没有在真实窗口里人工点过；这一条本身就是在补「前几轮没上真机」的账。
- 「返回原文件」后再改仍另存新结果（不累积），与 §26 一致。
- 连续改字依旧没有撤销；每次点击立刻覆盖结果文件。

### 27.6 产物（已被 §28.8 取代）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,662,450 B（30.20 MiB）**，SHA-256 `6669b77f1c0b8aeef387706f982a85b98366e1ac33b0014ae91f2cb1563f82fa` |
| 同目录 `.sig` | 文件签名 / 全局签名 / 1 bit 负向自检均 OK |
| `scripts/preflight-real-device.py` | `INSTALLER_SHA256` 已更新为 `6669b77f…82fa` |
| 前端产物回读 | `grep -l "正在继续修改本次结果文件" dist/assets/*.js` → `index-9GxT77zY.js`，确认新文案真的进了包 |

**哈希链路**：`0fe2fe3c…3757` → `4bbd7d4a…2d08` → `b16bd927…18e9` → `1fa562fc…d669` → `c9374790…b81d` → **`6669b77f…82fa`（本轮）**。前五个均已失效。

## 28. 工作台布局改版：文档画布不再被挤成一条（2026-11-21 19:40，用户截图反馈）

### 28.1 用户看到的问题

「整个板面布局非常差，中间的文档编辑页面被挤在上面一小块地方。」截图里 1080×714 的窗口：左侧栏 184px、右侧栏 292px 常驻，中间工作区还要再竖着塞「拖放区 + 文件列表」，文档画布只剩一条，页面底部全靠滚动。

量出来的原因很直接：

```css
.viewer { height: clamp(300px, 44vh, 500px); flex: 0 0 auto; }
```

画布高度被**写死**成 44vh（714 高的窗口约 314px），而它下面还有 `hero-line` + 拖放区 + 文件列表（约 300px），两者相加超过工作区高度 → 整页滚动。三块常驻栏谁也不能收，画布永远拿不到空间。

### 28.2 改法：三块可收，画布吃满剩余

| 区域 | 改动 |
|---|---|
| 文档画布 | `flex: 1 1 auto; height: auto; min-height: 210px`，高度由剩余空间决定，不再写死 vh |
| 左栏 / 右栏 | 顶栏「工具」「参数」开关 + 面板自带 `‹` `›` 按钮；收起时对应网格列宽压到 **0px**，元素保留槽位（见 28.3） |
| 底部文件区 | 拖放区改为**一行紧凑条**（44px，原来是 22px 内边距的大空盒子），与文件列表一起放进可收起的 `.dock`；有文件后默认收起，没文件时强制展开（总得能导入） |
| 标语 | `Your files never leave this computer.` 从工作区挪到顶栏（≥1240px 才显示），不再白占一行 |
| 记忆 | 三个开关状态存 `localStorage['payoncepdf-layout']`，坏数据回默认，不会因为读配置把界面挂掉 |

### 28.3 一个踩到的坑：`display:none` 会让整行左移

第一版收起用的是 `display: none`。结果左栏一收，元素离开网格流，后面的列自动前移：工作区跑到第一列（184px）、参数面板跑到第二列（578px），布局直接错位。

改成「列宽归零 + `visibility: hidden` + `overflow: hidden`」：元素仍在网格里占位（宽度 0），只是不可见、不可点。改完三档实测：

| 状态 | `grid-template-columns`（1080×714） |
|---|---|
| 默认 | `184px 578px 292px` |
| 收起左栏 | `0px 762px 292px` |
| 左右都收起 | `0px 1054px 0px` |

### 28.4 另一处：文件区被 flex 平分压扁

`.dock` 最初写 `flex: 0 1 auto`，量出来 `clientHeight 89 / scrollHeight 159`——文件列表被截掉一半。原因是画布 `flex: 1` 与文件区一起参与收缩分配。改成 `flex: 0 0 auto`（文件区该多高就多高，上限 52%，超出自己在区内滚动），画布拿剩下的，实测 `159 / 159`，不再裁切。

### 28.5 实测（无头 Chromium + CDP，1080×714，`scripts/layout-shot.mjs`）

| 场景 | 画布 `.viewer` | 画布可用区 `.viewer-canvas` |
|---|---|---|
| 改版前（固定 44vh） | 554 × **314** | 552 × 196 |
| 现在：无文件（文件区展开） | 554 × 334 | 552 × 219 |
| 现在：有文件（文件区自动收起） | 554 × **503** | 552 × 388 |
| 现在：左右栏也收起 | 1030 × **527** | 1028 × 388 |

左右收起后可用宽度 552 → 1028，接近翻倍；高度 196 → 388。工作区与整页 `scrollHeight === clientHeight`（527/527、714/714），不再出现整页滚动。

截图证据：`tests/out/layout-1-default.png`（无文件）、`layout-2-left-hidden.png`、`layout-3-both-hidden.png`、`layout-5-with-file.png`（有文件、文件区收起）、`layout-6-light.png`（浅色主题）。

### 28.6 回归

| 编号 | 用例 | 结果 |
|---|---|---|
| V-28.1 | `node scripts/layout-shot.mjs`（CDP 量尺寸 + 截图） | 三档列宽、画布尺寸如上；无裁切、无重叠 |
| V-28.2 | `node tests/ui-interaction-smoke.mjs` | **24 PASS / 0 FAIL**（新增 1 条「有文件后文件区自动收起」；落文件后先展开文件区再断言列表） |
| V-28.3 | `npm test` / `tsc --noEmit` / `npm run build` | 28/0；通过；通过 |
| V-28.4 | 浅色 / 深色 | 均正常，新控件全部用主题变量，无硬编码色 |

### 28.7 边界（诚实）

- 这是**无头 Chromium（浏览器预览模式）**量的，不是真实 Tauri 窗口。WebView2 与 Chromium 同一内核，CSS 行为一致，但真机仍应目视一次。
- 面板只能「开 / 关」，**不能拖拽改宽度**；需要更宽的参数面板时暂无手段（列为后续）。
- 100% 缩放下 A4 页面仍高于画布，纵向需要滚动——画布变大不等于「整页铺满」，后者需要「适合窗口」缩放，本轮未做。
- 侧栏收起后，`‹` 与 `›` 按钮随面板一起隐藏，只能从顶栏开关恢复；这是刻意的（顶栏永远可见）。

### 28.8 本轮产物（含新布局，当前唯一有效包）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,660,833 B（30.19 MiB）**，SHA-256 `7f986bee97144e777dde7d28f655284f3522f174290a80520c68295a71ca899e` |
| 同目录 `.sig` | 文件签名 / 全局签名 / 1 bit 负向自检均 OK |
| `scripts/preflight-real-device.py` | `INSTALLER_SHA256` 已更新为 `7f986bee…899e` |
| 前端产物回读 | `grep -l "文件与导入" dist/assets/*.js` → `index-BdVNqwjx.js`，确认新布局文案在包里 |

**哈希链路**：`0fe2fe3c…3757` → `4bbd7d4a…2d08` → `b16bd927…18e9` → `1fa562fc…d669` → `c9374790…b81d` → `6669b77f…82fa` → **`7f986bee…899e`（本轮）**。前六个均已失效。

## 29. 左右栏拖拽调宽（2026-11-21 20:05，用户「还是不能缩放」）

### 29.1 现象与定性

用户复测 §28 的布局改版后反馈「左右两栏还是不能缩放」。核对代码：§28 只实现了**收起 / 展开**（列宽 0 ↔ 默认值两档），没有拖拽改宽——`grid-template-columns` 的 `--left-w / --right-w` 只由 CSS 类和媒体查询决定，用户拖面板边缘没有任何命中区域。这是**功能缺口**，不是回归。

### 29.2 改法

| 项 | 实现 |
|---|---|
| 命中区 | `.app-body` 下的绝对定位兄弟元素 `.col-resizer-left / .col-resizer-right`（9px 宽、`role="separator"`、`tabIndex=0`、`touch-action:none`），`left/right: calc(var(--*-w) - 4px)`。不占网格流，也不受 `.params { overflow:auto }` 滚动影响 |
| 拖拽 | `pointerdown` 记起点并 `setPointerCapture`（失败 try/catch 兜底），`pointermove` 实时 `setLayout`；左栏 `startW + dx`、右栏 `startW - dx`，夹在 160–380 / 260–520 |
| 复位 / 键盘 | 双击手柄 → `leftW/rightW = null`（回到 CSS 默认，含窄窗口媒体查询）；聚焦后方向键 ±24px |
| 持久化 | 存进 `localStorage['payoncepdf-layout']` 的 `leftW/rightW`；坏数据 / 越界值在读取时 `clamp` |
| 收起优先 | 内联 `--left-w` 优先级高于 `.is-left-hidden` 的 `0px`，所以收起时内联强制输出 `0px`。否则「拖过宽度 → 收起」会留一条空列 |
| 手感 | `data-resizing="1"` 期间 `transition: none`（否则 180ms 过渡追不上鼠标）、全局 `col-resize` 光标、禁选中；手柄悬停/聚焦显示 2px 主题色提示线 |

### 29.3 实测（1180×760，无头 Edge + CDP，`scripts/resize-check.mjs`）

| # | 断言 | 实测 |
|---|---|---|
| 1 | 左右手柄都存在 | 2 个 |
| 2 | 默认列宽 | 214 / 336 |
| 3 | 左栏拖宽 +70 | 214 → **284** |
| 4 | 宽度写入 localStorage | `leftW: 284` |
| 5 | 拖宽后收起仍归零 | 284 → **0**（画布 818） |
| 6 | 左栏拖窄 −60 | 284 → **224** |
| 7 | 右栏鼠标左移 −60（变宽） | 336 → **396** |
| 8 | 双击左栏复位 | **214** |
| 9 | 双击右栏复位 | **336** |
| 10 | 刷新后保持用户宽度 | **300** |

**10 PASS / 0 FAIL**。截图：`tests/out/resize/layout-1-default.png`（左栏 300px 状态）。

### 29.4 回归

| 编号 | 用例 | 结果 |
|---|---|---|
| V-29.1 | `node scripts/resize-check.mjs` | 10 PASS / 0 FAIL |
| V-29.2 | `node tests/ui-interaction-smoke.mjs` | **24 PASS / 0 FAIL** |
| V-29.3 | `npm test` / `tsc --noEmit` / `npm run build` | 28/0；通过；通过 |
| V-29.4 | `scripts/layout-shot.mjs`（RESET=0，保留拖宽状态） | 六档截图正常，无错位 / 裁切 |

### 29.5 边界（诚实）

- 仍是**无头 Edge（浏览器预览模式）**实测，不是真实 Tauri 窗口；WebView2 同内核，但仍应在真机拖一次（`RD-G0-21`）。
- 只做了「左右栏调宽」，**底部文件区高度不可拖拽**（只有收起/展开）。
- 宽度上下限写死（左 160–380 / 右 260–520），窗口很窄时可能挤到画布；此时收起面板比拖窄更合适。
- 手柄宽 9px，触屏设备上偏细；未做触摸放大。

### 29.6 本轮产物（含左右栏拖拽调宽，当前唯一有效包）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,660,314 B（30.19 MiB）**，SHA-256 `31a432d457be954b9e41012749a8aa9a83e6ccaa1b657cf3ba8a3e7916859ac4` |
| 同目录 `.sig` | 440 B；key id `654e340d1fe4b8e5`；① 文件签名 ② 全局签名 ③ 1 bit 负向自检 **三段 OK** |
| `scripts/preflight-real-device.py` | **PASS 26 / WARN 4 / BLOCK 0 / FAIL 0**；`INSTALLER_SHA256` 已更新为 `31a432d4…59ac4` |
| 前端产物回读 | `dist/assets/index-Omwb6UAi.js` 含 `col-resizer` / `拖动调宽` / `调整左侧工具栏宽度`，确认拖拽代码在包里 |

**哈希链路**：`0fe2fe3c…3757` → `4bbd7d4a…2d08` → `b16bd927…18e9` → `1fa562fc…d669` → `c9374790…b81d` → `6669b77f…82fa` → `7f986bee…899e` → **`31a432d4…59ac4`（本轮）**。前七个均已失效。

**打包踩坑**：`tauri build` 报 `os error 32`（另一个程序正在使用此文件）——已安装的 `payoncepdf.exe`（PID 44372）还开着，锁住了 sidecar。先 `taskkill /F /PID` 再构建；正常关闭信号无效（窗口未响应）。重建命令同 §25.8，需带 `TAURI_SIGNING_PRIVATE_KEY` / `_PASSWORD`，否则出包但缺 `.sig`。

## 30. 竞品对标审查与 4 项缺陷修复（2026-11-21 20:50）

用户要求「详细查看哪些功能还没完善好，拿竞品对比，找出缺陷并修改完善」。完整报告（竞品格局、逐条分级、未修理由）见 `docs/defect-review-2026-11-21.md`。

### 30.1 本轮修复

| 编号 | 缺陷 | 修法 | 回归测试 |
|---|---|---|---|
| D1 | F19 链式状态残留，可能「看着 A 改了 B」 | 统一 `resetChainState()`，7 个入口全清；连续编辑白名单保留 | `tests/defect-regression.test.mjs` 静态锁 7 入口 |
| D2 | `onUsePage` 对无页码工具假成功 | 先判 `pages` 字段，无则如实提示 | 同上，失败/成功两分支文案 |
| D3 | 表单扁平化中文静默变 `?`（数据损坏） | `winansi_bytes()`：Latin-1 正确写出、非 WinAnsi 写入前拒绝 E1001 | Rust +4 |
| D4 | OCR 下载只查大小，错误页落盘 | `validate_traineddata()` 魔数 + 结构校验，失败 E1006 | Rust +2（含真实包防误杀） |

### 30.2 验证数字

- `cargo test --lib` **65 PASS / 0 FAIL**（原 59 + 6）；`npm test` **32 PASS / 0 FAIL**（原 28 + 4；同时放宽 `rename-contract` 对 test 脚本的硬编码断言）。
- `tsc --noEmit` / `npm run build` 通过；UI 交互冒烟（CDP 9222）**24 PASS / 0 FAIL**。
- `cargo test` 必须用项目随带工具链 + `CARGO_BUILD_JOBS=1`；全局工具链并发编译时 `crossbeam-epoch` 触发 `STATUS_ACCESS_VIOLATION`，属环境差异。

### 30.3 明确未修（诚实）

发布链路（Authenticode、生产 minisign、更新端点、激活/售卖、macOS 包）需外部基础设施；真机 73 条仍 0 执行；文档比对 / WYSIWYG / 高保真 Office / AI / 嵌套书签 / 复选表单 / 合并书签 / 旋转页框选按 PRD §19 暂缓或工作量中，属产品策略而非本轮可修缺陷。详见 defect-review §2.2 / §5。

真机用例 **73 → 74**（`RD-G0-22`：表单填中文后勾选扁平化，须明确报错且不产出文件；填 `30 ± 4 °C` 须扁平化成功且符号正常）。

### 30.4 本轮产物（含 4 项缺陷修复，当前唯一有效包）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,669,539 B（30.20 MiB）**，SHA-256 `84121674d22998733c9beaa05dab8e8aeeae2db3d05f53ae5ad26387ddbb5be6` |
| 同目录 `.sig` | ① 文件签名 ② 全局签名 ③ 1 bit 负向自检 **三段 OK**，key id `654e340d1fe4b8e5` |
| `scripts/preflight-real-device.py` | **PASS 26 / WARN 4 / BLOCK 0 / FAIL 0**；`INSTALLER_SHA256` 已同步 |

**哈希链路**：… → `7f986bee…899e` → `31a432d4…59ac4` → **`84121674…bb5be6`（本轮）**。此前所有哈希均已失效。

**打包踩坑（复现）**：又有运行中的 `payoncepdf.exe`（PID 14484）锁住 sidecar，`tauri build` 会报 `os error 32`；正常 `taskkill` 无效，须 `taskkill /F` 后再构建。

## 31. 合并 PDF 保留书签（2026-11-21 21:40）

用户「继续」后从 §30.3 的未修清单中挑出**合并书签丢失**修复。前端合并面板默认勾选「保留书签」且提示「合并后保留各文件的书签层级」，但后端从未实现——属虚假承诺。

### 31.1 实现

| 层 | 改动 |
|---|---|
| `engine/objects.rs` | 新增 `merge_bookmarks(sources, password, merged, out_tmp)`：读各源 `get_toc()`，按文件页数累加偏移重映射目标页，用 parent 栈保留层级，`add_bookmark` + `build_outline` 写 /Outlines；越界跳过、同路径拒绝 |
| `commands/jobs.rs` | `keep_bookmarks=true` 时调用重建，失败即返回错误；不再输出 `OUTLINE_LIMITATION` 的「不支持」说明，改为实际合并条数 |
| 文档 | 删除过时常量，更新 README M1 / 竞品分析 F1 / defect-review §2.2 与 §5（加 §6 追修记录） |

### 31.2 验证

- `cargo test --lib` **70 PASS / 0 FAIL**（原 65 + 5）。新增：页偏移 + 中文标题、嵌套层级 1/2/3、无书签源 + 越界跳过、输入输出同路径拒绝、**真实 qpdf e2e**（造带书签 PDF → qpdf 合并 → 确认书签丢失 → 重建 → 读回 4/4 条且页号 1/2/3/4 → `qpdf --check` 通过）。
- `npm test` **32 PASS / 0 FAIL**；`npm run build` 通过；UI 交互冒烟 **24 PASS / 0 FAIL**（连跑 3 次稳定）。
- 顺带修测试稳定性：`tests/ui-interaction-smoke.mjs` 每轮先清 `localStorage['payoncepdf-layout']` 再刷新，并在等待超时时显式报错。此前布局偏好跨轮残留会造成 2 条假失败（dock 展开态串扰）。

### 31.3 边界（诚实）

- 只保留指向页面目的地的书签；命名目的地 / 动作的条目 lopdf `get_toc()` 读不到，会漏掉。
- 仍是引擎级 + 无头浏览器证据，**真机 0/75 执行**（新增 `RD-G0-23`）。
- F15 书签管理界面仍只做顶层 set/add/rename/delete；合并能保留层级不等于能在界面里编辑层级。

### 31.4 本轮产物（含合并书签保留，当前唯一有效包）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,666,491 B（30.20 MiB）**，SHA-256 `176bf902d95e4ee85e72890318a49daf7222d5e8d81cba0d1b147a58fce2b6bf` |
| 同目录 `.sig` | ① 文件签名 ② 全局签名 ③ 1 bit 负向自检 **三段 OK** |
| `scripts/preflight-real-device.py` | **PASS 26 / WARN 4 / BLOCK 0 / FAIL 0**；`INSTALLER_SHA256` 已同步 |

**哈希链路**：… → `31a432d4…59ac4` → `84121674…bb5be6` → **`176bf902…2b6bf`（本轮）**。此前所有哈希均已失效。

## 32. 旋转页框选与按位置替换（2026-11-21 22:30）

用户再次「继续」后从 defect-review §5 待办中挑出**旋转页框选**修复。此前旋转页被 `hasTextLayer=false` 显式禁用（如实提示但功能缺失），竞品普遍支持。

### 32.1 根因：两套坐标系

实测钉死（随包 PDFium 0.9.5，用诊断测试打印）：

| 量 | 语义 | 实测（内容流 595×842） |
|---|---|---|
| `page.width()/height()`、`render_page_png()` | **显示尺寸** | /Rotate 0/180 → 595×842；90/270 → **842×595** |
| `PdfTextChar::tight_bounds()/loose_bounds()` | **内容流坐标**（左下原点） | 四个角度读到的字符框完全一致，不随 /Rotate 变化 |

F18 此前把内容流坐标字符框直接回传，前端按「屏幕→显示 pt」求交，整体错位；F19 更深层：`page.text()` 在旋转页按**视觉顺序**返回字符（如 "Drafttwo / Draftone"），而 `collect_runs()` 按内容流顺序，一致性闸门必然拒绝（这是防改错对象的安全设计，不能拆）。

### 32.2 改法

| 层 | 实现 |
|---|---|
| `engine/geometry.rs`（新） | `PageRotation`、`display_size()`、`content_to_display()`、`display_to_content()`；0/90/180/270 全覆盖，round-trip 单测 |
| `render.rs` F18 | 去掉旋转页禁用闸门；用内容流尺寸（由显示尺寸反推）把字符框正向变换到显示坐标后回传；`rotation` 保留真实值；前端零改动 |
| `extras.rs` F19 | 外层用 `with_unrotated()` 保证临时副本任何提前返回都清理；inner 用显示尺寸把选区逆变换回内容流坐标；在去旋转副本上定位+修改（此时字符顺序恢复内容流，安全闸门正常通过）；保存后用 `restore_rotations()` 把每页原 /Rotate 写回输出，再写后读回校验 |
| 前端/文档 | 更新 3 处「旋转页禁用」过时文案；契约测试改为验证坐标变换链路 |

### 32.3 验证

- `cargo test --lib` **77 PASS / 0 FAIL**（原 70 + 7）：geometry 双向变换 4 条（含 PDFium 实测锚点与四角度 round-trip）；F18 四角度显示坐标框 1 条；F19 三角度旋转替换 e2e 1 条；**qpdf 真实旋转 fixture e2e 1 条**（`tests/fixtures/text_rotated90.pdf`，F18 有文字层 + F19 替换成功 + 输出保留 /Rotate + `qpdf --check`）。
- `npm test` **32 PASS / 0 FAIL**；`npm run build` 通过；UI 交互冒烟 **24 PASS / 0 FAIL**；旋转页前端检查 **3 PASS / 0 FAIL**（`scripts/rotated-selection-check.mjs`）。
- 所有替换 e2e 都断言：原文件字节不变、输出通过 `qpdf --check`、旋转保留、只改选中一处。

### 32.4 边界（诚实）

- **加密 + 旋转页明确拒绝**：`unrotated_copy()` 无法在保留密码的前提下生成去旋转副本，沿用既有拒绝，不写坏文件。
- `page.text().all()` 在旋转页可能返回空（PDFium 行为）；F18 用的是逐字符 `text.chars()`（正常），F19 读回校验用 `collect_runs()`（内容流顺序，正常）。
- 仍是引擎级 + 真实 qpdf fixture + 无头浏览器证据，**真机 0/76 执行**（新增 `RD-G0-24`）。
- 前端浏览器预览模式仍禁用框选（无本地引擎），旋转支持只针对桌面 Tauri 预览。

### 32.5 本轮产物（含旋转页框选，当前唯一有效包）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,667,539 B（30.20 MiB）**，SHA-256 `e2849b8ee19c01eba37c7c25cf3d9cbfa3ef2dbca6c8c235c18fe69e42b6e58c` |
| 同目录 `.sig` | ① 文件签名 ② 全局签名 ③ 1 bit 负向自检 **三段 OK** |
| `scripts/preflight-real-device.py` | **PASS 26 / WARN 4 / BLOCK 0 / FAIL 0**；`INSTALLER_SHA256` 已同步 |

**哈希链路**：… → `84121674…bb5be6` → `176bf902…2b6bf` → `e2849b8e…6e58c` → **`452ac5d5…bcd1`（本轮）**。此前所有哈希均已失效。

---

## 33. 书签管理支持多级层级（2026-11-21 23:00）

> 触发：用户再次「继续」，从 defect-review §5 待办挑出 **F15 书签界面层级编辑**——§31 让合并保住了层级，但管理界面仍只会造顶层，属半截功能 + 数据丢失。

### 33.1 根因

- `parse_bookmark_spec` 硬编码 `level: 1`，spec 无法表达子级；
- `manage_bookmarks` 重建 `/Outlines` 时对每条书签 `add_bookmark(..., None)`，`list_bookmarks` 读出的 `level` 被丢弃；
- 结果：合并保住的层级，只要用书签管理做一次 set/add/rename/delete 就被压平成一整列顶层书签。前端提示还明写「只做顶层书签」。

### 33.2 改法

| 层 | 改动 |
|---|---|
| spec 解析 | 格式扩展为 `[>...]页码 标题`，行首 `>` 数量 = 层级（无 `>` 一级 / `>` 二级 / `>>` 三级），旧「页码 标题」完全兼容；层级跳跃（父级不存在）报错，不静默挂错父节点 |
| 写回 | `manage_bookmarks` 用 `(level, id)` parent 栈把每条挂到上一级最后一条，`add_bookmark` 传 parent；rename/delete 读回的 level 原样带出，任何操作都不压平 |
| 前端 | 提示「只做顶层书签」→「支持多级书签」；spec 输入说明 `>`/`>>` 与不可跳跃；读取结果按 level 缩进、展示最深层级 |
| 文档 | CONTRACT / PRD / README / 用户手册 / 竞品矩阵 / 站点首页 / 真机计划同步更新过时表述 |

### 33.3 验证

- `cargo test --lib` **79 PASS / 0 FAIL**（原 77 + 2）：`bookmark_spec_parses_and_validates_levels`（旧格式兼容、层级解析、标题含 `>`、跳跃拒绝、同级/降级允许）；`manage_bookmarks_keeps_nested_levels_across_operations`（真实带层级 PDF → set 带层级 spec → 读回 level `[1,1,2,3,1]` → rename → 层级不压平 → delete 子级后仍保层级）。
- `npm test` **34 PASS / 0 FAIL**（原 32 + 2 缺陷契约：`tests/defect-regression.test.mjs` 锁定 parent 栈写回、spec 层级解析、前端多级提示与缩进）；`tsc --noEmit` / `npm run build` 通过；UI 交互冒烟 **24 PASS / 0 FAIL**。
- 仍是引擎级 + 无头浏览器证据，**真机 0/77 执行**（新增 `RD-G0-25`）；层级由 spec 文本决定、不支持界面拖放换父级（诚实边界）。

### 33.4 本轮产物（含多级书签，当前唯一有效包）

| 产物 | 大小 / 校验 |
|---|---|
| `src-tauri/target/release/bundle/nsis/PayOnce-PDF-Setup-1.0.0.exe` | **31,671,628 B（30.20 MiB）**，SHA-256 `452ac5d55f67b437c831bdb0bc6e340c0d62ed2ebdc0d4fc159d0957da82bcd1` |
| 同目录 `.sig` | ① 文件签名 ② 全局签名 ③ 1 bit 负向自检 **三段 OK** |
| `scripts/preflight-real-device.py` | **PASS 26 / WARN 4 / BLOCK 0 / FAIL 0**；`INSTALLER_SHA256` 已同步 |
