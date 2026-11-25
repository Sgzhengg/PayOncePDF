# PayOnce PDF（一次付费 PDF）

> **一个不订阅、不注册、文件内容永不上传的本地 PDF 工具箱。**（PRD §2）
> 合并 / 拆分 / 压缩 / 图片互转 / 本地 OCR / 签名与表单 / 批量 / **就地改字 / 批注 / 页面管理 / 水印与加密 / 文档导出 / 文档对象（表单·脱敏·书签）/ 文档比对 / 文本转 PDF / 打印与扩展**，
> **十六个工具全部在本机完成**；`$39 / ¥199` 一次买断，含 12 个月 1.x 更新，7 天试用、7 天无理由退款。**单一标价，不设早鸟 / 划线折扣**（2026-11-24 用户决定取消早鸟，此前 `$29 / ¥149` 首发折扣作废）。定价依据见 `docs/pricing-decision.md`；国内渠道尚未开放，人民币价仅作参考。
> 2026-11-16 两轮范围变更：功能面对标 PDFgear 桌面版（PRD §22、`docs/pdfgear-parity-matrix.md`）。
> **隐私口径**：文件内容与文件名永不上传；运行时网络请求只有三类且可枚举——一次性激活、可选更新检查、用户主动安装 OCR 语言包。不得宣称「零网络请求」。
> **2026-11-20 F18**：内置阅读视图支持在渲染页拖选文字，把选中文字与页码带入改字 / 批注 / 脱敏；框选只做定位，执行仍走既有命令，同页重复文字会被唯一性守卫拦下。详见 `docs/requirements/PRD.md` §22.12 与 `docs/test-report.md` §21（**真机未验**）。
> **2026-11-21 F19**：在 F18 选区上可直接「按框选坐标替换」——只改选中那一处，同页其它相同文字不受影响；改完可直接接着框选别处继续改，多处改动累积在同一份 `_edited` 文件里（原件始终不变）；只支持同一行内连续文字且字号一致（上下标请单独框那个小字），跨行 / 文字层与内容流顺序不一致时明确拒绝且不写文件；旋转页（/Rotate 90/180/270）已支持框选替换——显示坐标与内容流坐标双向变换、在临时去旋转副本上定位后写回原旋转（加密+旋转页因无法安全生成副本明确拒绝）。字符范围：英文、数字、中文（需内嵌字体）、Latin-1 符号（± ° µ × ÷）；≈ ≤ ≥ ∑ ∫ √ π 这类数学符号取决于原 PDF 是否内嵌含该字形的字体，缺字形时拒绝而不写坏。详见 `docs/requirements/PRD.md` §22.13 与 `docs/test-report.md` §23（**真机未验**；布局：左栏 / 右栏 / 底部文件区都可收起（顶栏「工具 / 参数 / 文件」），收起后空间全部给文档画布；左右栏边缘可拖拽调宽、双击复位，宽度会记住。

> **2026-11-25 交付 1.0.1（体积精简，下载提速）**：`provision-sidecars.mjs` 原先按「qpdf.exe 同目录 DLL 全量拷贝」把整条 GTK/ICU 栈带进安装包（`libicudt75.dll` 单文件 29.3 MB，另有 pango/cairo/glib/harfbuzz 等共 43.6 MB 死重量）。改为沿导入图做依赖闭包、只拷真正被引用的 DLL 后，resources 由 63 个文件 78 MB 降到 46 MB，安装包 **30.25 MiB → 18.48 MiB（19,375,119 B）**，SHA-256 `39121646…e67147`，更新签名 `.sig` 已用开发密钥重签（`version:1.0.1`）。干净目录（仅 36 个 DLL）跑应用内自检 **32/32**，`libstdc++-6.dll` 因被 qpdf 真实 import 必须保留。详见 `docs/test-report.md` §42。
> **2026-11-24 交付（opener 权限加固后重打包）**：Tauri `opener` 由「任意 URL」收紧为显式 allow-list 后再次重建，`PayOnce-PDF-Setup-1.0.0.exe` 30.25 MiB / 31,719,938 B，SHA-256 `8307c7b5…6d2d92`，更新签名 `.sig` 三段验签通过；release 自检 32/32。完整记录见 `docs/test-report.md` §41（§40 / §39 两版包的哈希均已作废）。
> **2026-11-16 品牌改名**：产品现名 PayOnce PDF；官网 `https://payoncepdf.com`，联系 `hello@payoncepdf.com`。旧名 / 旧域名 / 旧许可证前缀的完整映射与迁移风险见 `docs/RENAME-MIGRATION.md`；价格四字段集中记录见 `config/product.json`（**冲突已于 22:13 裁决**，`pending_confirmation: false`）。

- 项目根目录：`E:\PayOncePDF`（2026-11-16 由 `<old-repo>` 整体迁入，21:35 完成目录重命名；项目内 Rust 工具链、构建缓存、sidecar 与安装包一并保留）。改名时先停 dev server、移动后在新路径重启 1420 预览服务；历史日志与 cargo 缓存里的旧绝对路径是历史生成物。
- 技术栈：Tauri 2.12（Rust）+ React 19 + TypeScript；一套代码，Windows 10/11 + macOS 11+（Apple Silicon & Intel）双平台。
- 需求文档：`docs/requirements/PRD.md`（项目内副本，原始位置 `E:\PainRadar\docs\pdf-launch\PRD.md`）；接口契约：`CONTRACT.md`（命令、错误码、验收映射，所有开发以此为准）。
- 第二轮实测（对标补齐后）：`npm run build` 61 modules 通过、`cargo check --lib` 0 warning / 0 error、`cargo test --lib -- --test-threads=1` **28 PASS / 0 FAIL**（含 11 项真实 PDF 文件 + PDFium DLL + `qpdf --check` 文件级端到端）、UI 交互冒烟 **20 PASS / 0 FAIL**（15 入口）、验收 smoke **9 PASS / 0 FAIL / 14 SKIP(manual)**。详见 `docs/test-report.md` §13 与 `docs/pdfgear-parity-matrix.md` §2.2。
- 当前有效安装包：**`PayOnce-PDF-Setup-1.0.1.exe`（18.48 MiB / 19,375,119 B，含 F1–F21 + 授权加固 + opener allow-list + 依赖闭包精简）+ 更新签名 `.sig`**（开发密钥，`version:1.0.1`），位于 `src-tauri/target/release/bundle/nsis/`；SHA-256 `39121646c9ebc791ecaf6019705d3d75cc7c1ab484303ec896c4f24df3e67147`，重建与验签见 `docs/test-report.md` §42。上一版 `PayOnce-PDF-Setup-1.0.0.exe`（30.25 MiB / 31,719,938 B，SHA-256 `8307c7b5…6d2d92`）连同更早的 `f1276027…721ae83f03`、§39 包与 27.08 MiB 包（F1–F16）哈希全部作废，均不得再对外分发。
- **诚实边界**：安装包**没有 EV/Authenticode 签名**、macOS 公证、Gumroad 密钥发放、真机双平台端到端测试均未完成。2026-11-25 起 Windows 包已按用户决定对外提供下载（`https://dl.payoncepdf.com/v1.0.1/`，附 `SHA256SUMS.txt`），但**仍是未签名包**：Windows 会提示未知发布者，官网已如实披露，不要把它描述成已签名或已公证。

---

## 1. 目录结构

```
PayOnce PDF/                          # <repo>
├─ README.md / CONTRACT.md        # 说明 + 前后端接口契约
├─ package.json / vite.config.ts / tsconfig.json / index.html
├─ src/                           # React 19 UI（27 个 ts/tsx/css 文件）
│  ├─ main.tsx / App.tsx / styles.css
│  ├─ lib/                        # types / backend（Tauri 适配 + 预览 mock）/ errors / params
│  ├─ components/                 # Sidebar / DropZone / FileList / ParamPanel / ProgressBar / Dialog ...
│  └─ panels/                     # 十六个功能参数面板（ExtrasPanels.tsx F10–F14、MorePanels.tsx F15–F16、ComparePanel.tsx F20）
├─ src-tauri/
│  ├─ Cargo.toml / build.rs / tauri.conf.json / capabilities/default.json
│  ├─ src/                        # Rust 后端（21 个 .rs）
│  │  ├─ commands/                # jobs(F1–F7 + 9 种批量) / extras(F10–F14) / objects(F15) / more(F16) / app
│  │  ├─ engine/                  # sidecar / qpdf / render / pdf / ocr / extras / objects(F15) / more(F16)
│  │  └─ error / license / logging / state
│  ├─ bin/                        # sidecar（按 target-triple 命名，不入库）
│  ├─ resources/                  # tessdata / pdfium / 依赖 DLL / 第三方许可（随包）
│  ├─ keys/public.key             # Ed25519 公钥（入库）；私钥绝不入库
│  └─ icons/                      # icon.ico / icon.icns / PNG（由 scripts/make-icon.py 生成）
├─ site/                          # 官网：index(.zh) / pricing / faq / privacy / verify + styles.css
├─ docs/                          # 用户手册 / 错误码表 / 测试报告 / 支持模板 / 痛点竞品分析 / requirements(需求副本)
├─ scripts/                       # provision-sidecars / gen-license-keys / license-server
│                                 # generate-fixtures / make-icon
├─ tests/                         # fixtures / acceptance.py / ui-interaction-smoke.mjs / verify-updater-signature.py / license_test.md / out/
└─ .github/workflows/build.yml    # 三路 CI 矩阵 + universal + verify
```

---

## 1.1 功能清单（16 个 UI 入口 / F1–F7 + F10–F16 + F20，2026-11-21 增补文档比对后）

| 编号 | 功能 | 引擎 | 诚实边界 |
|---|---|---|---|
| F1–F7 | 合并 / 拆分 / 压缩 / 图片互转 / 本地 OCR / 签名表单 / 批量 | qpdf、lopdf、PDFium、Tesseract | 见 `CONTRACT.md` 与原 PRD；批量 9 种操作、带 `report.csv` 错误码台账 |
| **F10** | **就地替换文字** | PDFium `FPDFText_SetText` + 内容流显式提交 + **写后读回校验** | 不重排、不回流；变长替换可能覆盖右侧；原字体缺字形**明确报错**，不写乱码；扫描件需先 OCR |
| **F11** | 批注（高亮/下划线/删除线/波浪线/便签） | PDFium 查找 + lopdf 标准注解字典 | 只标注可选中文本；旋转页有兼容层 |
| **F12** | 页面管理（旋转/删除/抽取/插入空白/裁剪） | lopdf 重建页树 + qpdf 切页 | 嵌套书签可能不随页号更新；裁剪只改 `/CropBox` |
| **F13** + F16 | 水印、页眉页脚、页码、文档属性、AES-256 密码与权限、**中文水印** | lopdf 内容流 + qpdf 加密 + PDFium/系统 CJK 字体 | **水印文字支持中文**（水平居中灰字，无旋转/独立透明度）；页眉页脚仍限 WinAnsi 拉丁字符；不破解已有密码 |
| **F14** | 导出 TXT / HTML / DOCX / XLSX | PDFium 文字层 + 自写 OOXML ZIP（**无新增依赖**） | **只导文字层，不还原版式/表格/图片**；与商业 SDK 转换差距明确写入 UI/FAQ |
| **F15** | 创建 AcroForm 文本域 / 真脱敏 / 多级书签管理 | lopdf `/AcroForm` + PDFium 删文字对象 + `/Outlines` 重建 | 仅文本域（无复选框/单选/下拉）；脱敏**真删文字**而非只盖黑框；书签用 spec 行首 `>` 表达层级（不支持拖放换父级）、不随页号自动更新 |
| **F16** | TXT/CSV/HTML → PDF、系统打印 / 邮件、42 种 OCR 可选语言包 | PDFium + 系统 CJK 字体嵌入 + `ureq` | 纯文本 A4 排版，**不还原 Office 版式**；打印无应用内预览、mailto 附件需手动添加；语言包下载是**唯一运行时联网点** |
| **F20** | 文档比对（两版 PDF 文字层增删改差异 + 报告导出） | PDFium 文字层 + 自写 LCS diff（**无新增依赖**） | **只比文字层，不识别图片/版式/页序差异**；扫描件需先 OCR；页数不同时按整页列出；只读查询不占导出授权；结果需人工复核 |

> 功能对标 PDFgear 桌面版的范围变更与未决决策见 `docs/pdfgear-parity-matrix.md`。
> **用户明确暂缓**：定价决策、高保真 Word/Excel 转换、AI 文档问答、移动端。
> **仍有明确落后项**（不得宣称「功能全面超越」）：WYSIWYG 编辑器、图片/形状/链接/文本框编辑、图章与绘图、复选框/单选/下拉表单域、应用内打印预览、邮件自动附附件、网页版、macOS 构建与签名、真机 Tauri 端到端、阅读器兼容性验证。

---

## 2. 本地开发前置条件

| 依赖 | 用途 | 说明 |
|---|---|---|
| **Node 22** | 前端 / Vite 7 / Tauri CLI | 本机 22.22.2 |
| **Rust**（rustup stable） | Tauri 2 后端 | `rust-version = 1.77.2`；Windows 需 MSVC 生成工具 + WebView2，macOS 需 Xcode CLT |
| **qpdf** | 合并 / 拆分 / 已知密码解密 | Apache-2.0 sidecar；本机已供给 11.9.1 mingw64 + 4 个依赖 DLL |
| **tesseract** | 本地 OCR | Apache-2.0 sidecar；本机 5.5.0 + 56 个依赖 DLL |
| **PDFium** | PDF → 图片渲染 | BSD-3；`pdfium.dll`（Win）/ `libpdfium.dylib`（mac），本机已供给 7.5MB |
| **tessdata** | OCR 语言模型 | `eng` + `chi_sim`（tessdata_fast），本机已供给 |
| **签名材料** | 发行 | Windows EV / Azure Trusted Signing；macOS Developer ID + notarytool 公证 |
| **minisign 密钥** | 更新校验 | 已生成**开发用**密钥对：公钥写入 `tauri.conf.json`，私钥仅存于 gitignored `.toolchain/payoncepdf-updater.key`。发布前必须用生产密钥替换公钥并在 CI 配置 `TAURI_SIGNING_PRIVATE_KEY` |

三平台目标：`x86_64-pc-windows-msvc`、`aarch64-apple-darwin`、`x86_64-apple-darwin`；macOS 两架构合成 universal binary。

---

## 3. 启动命令

```bash
npm install
npm run dev                  # 仅 Vite（浏览器预览模式，明确标注"未调用本地引擎"）
npm run tauri dev            # Tauri 桌面开发
npm run typecheck            # tsc --noEmit（严格模式）
npm run build                # tsc --noEmit && vite build
node scripts/provision-sidecars.mjs --target x86_64-pc-windows-msvc
# createUpdaterArtifacts: true -> 构建需提供 minisign 私钥（CI 走 secrets）
#   TAURI_SIGNING_PRIVATE_KEY=<key文件路径> TAURI_SIGNING_PRIVATE_KEY_PASSWORD=<口令>
npx tauri build --bundles nsis       # Windows 包 + updater .sig（无 Authenticode 签名）
```

Rust 工具链可完全隔离在项目内，不污染系统环境：

```bash
CARGO_HOME=<repo>/.toolchain/cargo RUSTUP_HOME=<repo>/.toolchain/rustup \
  rustup-init -y --no-modify-path --profile minimal --default-toolchain stable-x86_64-pc-windows-msvc
cargo check --manifest-path src-tauri/Cargo.toml
```

> 预览模式（浏览器）下文件选择只能拿到文件名、命令走 mock、不写盘、不产生真实输出；UI 顶部有常驻提示横幅。

---

## 4. Sidecar 供给

- `tauri.conf.json`：`bundle.externalBin: ["bin/qpdf", "bin/tesseract"]`，`bundle.resources: ["resources/*", "resources/tessdata/*", "keys/public.key"]`（Ed25519 公钥随包；另由 `include_str!` 编译期内嵌作为信任根回退）。
- 可执行文件按目标三元组命名，例如 `bin/qpdf-x86_64-pc-windows-msvc.exe`、`bin/tesseract-aarch64-apple-darwin`。
- `node scripts/provision-sidecars.mjs --target <triple>` 从上游获取二进制，校验 sha256 与 CPU 架构，并把 Windows 依赖 DLL 放进 `resources/`；Rust 端 `sidecar::apply_child_env()` 会把 resource_dir 前置到子进程 `PATH` / `DYLD_LIBRARY_PATH`，保证干净机器可运行。
- `--dry-run` 打印计划、`--offline <dir>` 从本地缓存供给；未核实的 URL 集中标在 `SOURCES` 常量表，绝不静默成功。
- `get_app_info` 启动时自检 qpdf / tesseract / tessdata / PDFium，缺失返回 `E2001` / `E2002` / `E1006`。
- macOS 需对 sidecar 与 dylib 逐文件 `codesign --options runtime`，再公证 staple（流程已写入 CI）。

---

## 5. 双平台构建与签名流程概要

1. **前端**：`npm run build` → `dist/`（target 按 `TAURI_ENV_PLATFORM` 选 `chrome105` / `safari16`）。
2. **CI 矩阵**（PRD §2.3）：`windows-latest` + `macos-14`（arm64）+ `macos-13`（x64）三路构建，另加 `universal` 与 `verify` job；禁止「最后一起联调」。
3. **Windows**：`.msi` / `.exe`（nsis）；EV 证书或 Azure Trusted Signing 签名，避免 SmartScreen 警告。
4. **macOS**：分别构建两架构 → `lipo` 合成 universal → Developer ID 签名 → `notarytool submit --wait` → `stapler staple` → `.dmg`。不上 Mac App Store（沙箱会限制 sidecar）。
5. **更新**：Tauri Updater + 签名校验；前端「关于」里可关闭检查、手动检查与下载安装；失败保留旧版本可回滚。公钥已换成本机开发密钥，endpoint `https://payoncepdf.com/api/update/...` 仍为占位，发布前统一替换。
6. **凭据**：CI 全部走 `secrets.*`（18 项清单见 workflow 末尾），未配置时明确标注 `UNSIGNED` / `NOTARIZE_SKIPPED` 并跳过签名，绝不硬编码。

---

## 6. 当前里程碑状态表（M0–M8，PRD §15）

状态图例：`完成（本机）` / `部分完成` / `待外部条件` / `待测`。

| 里程碑 | 内容 | 当前状态 | 说明 |
|---|---|---|---|
| **M0** | 骨架：Tauri + React + 侧边栏 + 拖拽 | **完成（Windows 本机）** | 前端构建、Rust 编译、UI 截图核验均通过；macOS 侧由 CI 矩阵保证，尚未在本机实机验证 |
| **M1** | 合并 / 拆分 + qpdf 封装 | **部分完成** | F1/F2 代码完成；页数之和与 500→50 拆分通过 smoke；三平台 sidecar 仅 Windows 已实供；勾选保留书签时由 lopdf 按页偏移重建各源文件书签（含层级），真实 qpdf 链路已测 |
| **M2** | 压缩 + 图片互转 | **部分完成** | F3/F4 代码完成（lopdf 重编码 + PDFium 渲染）；效果与性能待测 |
| **M3** | 本地 OCR | **部分完成** | F5 代码完成，eng/chi_sim 已随包；准确率与耗时待测 |
| **M4** | 签名 / 表单 | **部分完成** | F6 代码完成；Adobe Reader 打开与扁平化待测 |
| **M5** | 批量 + 报告 | **部分完成** | F7 代码完成，`report.csv` 字段通过校验；200 文件真机待测 |
| **M6** | 授权 / 试用 / 更新 | **部分完成** | 7 天试用、Ed25519 离线缓存、导出限制 E2005 已实现并验签通过；F9 前端开关与检查已实现，开发公钥/`.sig` 已就绪，但生产 pubkey、endpoint 与售卖仍未配置 |
| **M7** | 打包与签名 | **部分完成** | Windows NSIS 包 27.08 MiB + 开发密钥 `.sig` 已于收尾轮重建（`tests/out/build-round2.log`、`docs/test-report.md` §14）；EV/Authenticode 签名、Developer ID/公证、18 个 secrets 均未就绪 |
| **M8** | 硬化：错误码、日志、FAQ、官网 | **完成（文档与代码层）** | 12 个错误码、本地日志、官网 6 页、手册、支持模板、验证页、第三方许可齐备；§14 真机测试仍待测 |
| **M9** | 对标 PDFgear 第一轮：F10–F14 | **代码完成，验证被 M10 推翻重做** | 5 个命令 + 12 入口 + 批量 9 种；当时 11 项单测通过，但第二轮实证发现 F10 改字未生效、旋转页文字层为空（详见 M10） |
| **M10** | 对标 PDFgear 第二轮：真实 PDF 文件级验证 + F15/F16（2026-11-16） | **引擎级验证完成 / 真机待测** | 修好两个真 bug（`regenerate_content()`、旋转页 `with_unrotated()`）；新增 8 个命令、3 个前端入口（共 15）；**28 PASS / 0 FAIL** 单测含 11 项真实 PDF 文件级；`npm run build` 61 modules、UI 冒烟 20 PASS / 0 FAIL；NSIS 包已重建并验签（§14）；**真机 Tauri GUI 端到端、Adobe Reader / Word / Excel 打开产物、AES-256 阅读器实测、macOS 分支全部未做** |
| **M11** | 缺陷追修 + F20 文档比对（2026-11-21） | **引擎级验证完成 / 真机待测** | 修连续编辑/布局/拖拽调宽/合并书签层级/旋转页框选/多级书签；新增 F20 文字层比对（增删改 diff + 报告导出，共 16 入口）；**cargo 90 PASS / 0 FAIL**、npm 35 PASS / 0 FAIL、UI 冒烟 26 PASS / 0 FAIL；NSIS 包已重建验签；**真机 Tauri GUI 端到端仍未做** |

> 结论：M0 已在本机达成；M1–M6、M8 的**代码与文档层**已落地；M7 只完成了 Windows 未签名包与更新签名；M9/M10 是 2026-11-16 两轮范围变更，M10 已达到「引擎级真实 PDF 文件级验证」但**尚未达到真机 Tauri GUI 端到端**。PRD 要求「每个里程碑结束提交两平台可运行安装包」目前**未达成**（macOS 未构建、Windows 未做 Authenticode 签名），不得宣称里程碑全部完成。
> 范围变更依据：`docs/requirements/PRD.md` §22、`docs/pdfgear-parity-matrix.md`。

### 待外部条件清单

- [ ] Windows EV / Azure Trusted Signing；macOS Apple Developer ID（$99/年）+ notarytool 公证 + staple
- [ ] CI secrets（18 项）、**生产** minisign 密钥对（替换当前开发公钥）、真实更新端点
- [ ] Gumroad / 爱发电 商品与密钥发放服务（当前 `scripts/license-server.mjs` 为本地 mock）
- [ ] macOS arm64 / x64 真机或 CI 实跑，universal dmg
- [ ] 双平台端到端测试：OCR 准确率、压缩画质、性能、抓包无上传、安装包体积/冷启动/内存
- [ ] 域名核实与官网占位项（下载 / 购买 / 联系 / 端点主机名）替换，见 `site/README.md`

---

## 7. 许可红线（PRD §9.2 / CONTRACT §9）

**允许（白名单）**：PDFium（BSD-3）、qpdf（Apache-2.0）、Tesseract + tessdata（Apache-2.0）、lopdf（MIT）、image（MIT/Apache-2.0）、React（MIT）。
**明确禁用**：MuPDF（AGPL-3.0）、Ghostscript（AGPL-3.0）——不得引入发行包。
**延后评估**：LibreOffice（MPL-2.0，体积大）——v1 不做 Word/Excel 转换的原因之一。
**工程红线**：任何文件处理功能都不联网、文件内容与文件名永不上传、不收集邮箱、不做遥测、日志默认关闭；运行时请求仅三类且可枚举（一次性激活、可选更新检查、用户主动安装 OCR 语言包）；失败不损坏原文件；只用白名单组件。**禁止再写「零网络请求」**。
完整许可证文本与归属见 `src-tauri/resources/THIRD_PARTY_LICENSES.md`。

---

## 8. 相关交付物

- 官网：`site/index.html`、`index.zh.html`、`pricing.html`、`faq.html`、`privacy.html`、`verify.html`
- 文档：`docs/user-manual.md`、`error-codes.md`、`test-report.md`（§13 第二轮验证）、`support-template.md`、`pain-and-competitive-analysis.md`（PainRadar 真实语料分析，§10/§11 补记两轮范围变更）、`pdfgear-parity-matrix.md`（对标矩阵与未决决策）、`google-ai-pain-points-adoption.md`（2026-11-22 借鉴 Google AI 痛点报告的产品改进清单，按投入产出排序）
- 测试：`tests/acceptance.py`、`tests/ui-interaction-smoke.mjs`、`tests/verify-updater-signature.py`（用 `tauri.conf.json` 公钥复现 minisign 验签）、`tests/license_test.md`、`tests/fixtures/`、`tests/out/acceptance-report.md`
- 契约：`CONTRACT.md`；需求：`docs/requirements/PRD.md`（及 `copy.md` / `preorder.md` / `landing*.html`，原始位置 `E:\PainRadar\docs\pdf-launch\`）
