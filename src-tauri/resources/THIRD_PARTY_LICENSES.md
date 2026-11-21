# PayOnce PDF — 第三方组件许可证与归属（Third-Party Licenses）

> 本文件随应用打包（`tauri.conf.json` → `bundle.resources: ["resources/*"]`），是 PRD §9.2「许可红线」的落地清单。
> 原则：只使用宽松许可组件；**禁止任何 AGPL / 强 copyleft 组件进入发行包**。所有下列组件均在本机运行，不向第三方发送任何数据；本应用无遥测、无上传。
> 新增任何第三方组件，必须先更新本文件与 `CONTRACT.md` §9 白名单，再改代码。

---

## 1. 允许使用的组件（PRD §9.2 / §9.1、CONTRACT §9）

| 组件 | 用途 | 许可 | 随包位置 / 获取地址 |
|---|---|---|---|
| **PDFium** | PDF 页面渲染为 PNG/JPG（经 `pdfium-render` 绑定） | **BSD-3-Clause** | `resources/pdfium.dll`（Windows）/ `resources/libpdfium.dylib`（macOS）；上游 https://pdfium.googlesource.com/pdfium/ |
| **qpdf** | 合并 / 拆分 / 线性化 / 移除已知密码（sidecar） | **Apache-2.0** | `bin/qpdf-<target-triple>`；上游 https://github.com/qpdf/qpdf |
| **Tesseract** | 本地 OCR 引擎（sidecar） | **Apache-2.0** | `bin/tesseract-<target-triple>`；上游 https://github.com/tesseract-ocr/tesseract |
| **tessdata** | OCR 语言模型 `eng` / `chi_sim` | **Apache-2.0** | `resources/tessdata/eng.traineddata`、`chi_sim.traineddata`；上游 https://github.com/tesseract-ocr/tessdata |
| **lopdf** | PDF 结构操作（图片→PDF、压缩重编码、签名图章、表单填写/扁平化） | **MIT** | Rust crate `lopdf`；上游 https://github.com/J-F-Liu/lopdf |
| **image** | 图像重编码（PNG/JPG） | **MIT / Apache-2.0** | Rust crate `image`；上游 https://github.com/image-rs/image |
| **React / React DOM** | 用户界面 | **MIT** | npm 依赖；上游 https://github.com/facebook/react |
| **Tauri 2** | 桌面框架 / 插件 | **MIT / Apache-2.0** | Rust crate + npm `@tauri-apps/*`；上游 https://github.com/tauri-apps/tauri |

### 其它运行时依赖（随 Cargo / npm 引入）

`serde`、`serde_json`、`thiserror`、`ed25519-dalek`（BSD-3）、`base64`、`ureq`、`sha2`、`hex`、`uuid`、`walkdir`、`chrono`、`pdfium-render`、`tauri-plugin-dialog / opener / updater`、Vite / TypeScript 等。
它们均为 MIT / Apache-2.0 / BSD 系宽松许可，通过 crates.io 与 npm 获取；完整许可文本以各 crate / 包随附的 `LICENSE` 为准。

<!-- TODO(许可证复核): lopdf、Tauri 等上游可能采用双许可（MIT/Apache-2.0）；以随包 LICENSE 文本为准，逐项复核后再定稿 -->
<!-- TODO(许可证地址复核): 上线前逐条核对上表上游仓库地址与许可文本版本 -->

---

## 2. 明确禁用 / 延后评估的组件

| 组件 | 许可 | 结论 |
|---|---|---|
| **MuPDF** | AGPL-3.0 | **明确禁用**（PRD §9.2）。不得以任何形式引入：静态/动态链接、sidecar、衍生代码、内嵌资源 |
| **Ghostscript** | AGPL-3.0 | **明确禁用**（PRD §9.2）。同上，不得作为转换/渲染后端 |
| **LibreOffice** | MPL-2.0 | **延后评估**：体积大，v1.1 再评估（PRD §9.2 / §17）。这也是 v1 不做 PDF → Word/Excel 转换的原因之一；若未来引入，须重新做许可与体积评估 |

> 许可污染是法律风险（PRD §17）。任何 AGPL 组件都会破坏「本地、买断、可分发」的成本与合规结构，因此列入硬红线。

---

## 3. 完整许可证文本的提供 / 获取地址

发行包应随附各组件的完整许可证文本与归属声明：

- **随包提供**：`resources/licenses/<component>/LICENSE`（与 `THIRD_PARTY_LICENSES.md` 一同由 `bundle.resources` 打包）。
  <!-- TODO(随包许可证): 目录目前尚未填充，需在打包脚本中把下列上游 LICENSE 文本拷贝进 resources/licenses/ 并随包签名 -->
- **获取地址（上游官方仓库的 LICENSE 文件）**：
  - PDFium：https://pdfium.googlesource.com/pdfium/ （BSD-3-Clause LICENSE）
  - qpdf：https://github.com/qpdf/qpdf （LICENSE.txt，Apache-2.0）
  - Tesseract：https://github.com/tesseract-ocr/tesseract （LICENSE，Apache-2.0）
  - tessdata：https://github.com/tesseract-ocr/tessdata （LICENSE，Apache-2.0）
  - lopdf：https://github.com/J-F-Liu/lopdf （LICENSE）
  - image：https://github.com/image-rs/image （LICENSE-MIT / LICENSE-APACHE）
  - React：https://github.com/facebook/react （LICENSE，MIT）
  - Tauri：https://github.com/tauri-apps/tauri （LICENSE_MIT / LICENSE_APACHE）
  - Rust crates：`cargo vendor` 或 crates.io 各 crate 页面的 License 字段；npm 包：`node_modules/<pkg>/LICENSE`。
- **当前状态（诚实）**：完整许可证文本**尚未随包提供**，上述 `resources/licenses/` 目录为空占位，需在发布前补齐（TODO 随包许可证）。本清单只列组件、许可与获取途径，不替代完整许可证文本。

---

## 4. 变更约束

1. 新增 / 更换第三方组件：先改 `CONTRACT.md` §9 白名单与本文件，再改 `Cargo.toml` / `package.json`。
2. 禁止引入 §2 中「明确禁用」的组件；`LibreOffice` 在 v1 内不引入。
3. 每次发布前复核：随包 `LICENSE` 文本齐全、sidecar 与动态库均已签名、上游许可版本未变更。
4. 本文件与 `src-tauri/resources/README.md`、`site/privacy.html`（第三方组件说明）保持一致。
