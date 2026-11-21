# Tauri externalBin sidecar 目录

本目录存放 qpdf / tesseract 两个 Tauri **externalBin** sidecar，以及供给脚本生成的
`sidecar-manifest.json`。它们由 `scripts/provision-sidecars.mjs` 在 `tauri build` 之前
按目标 triple 生成，属于**构建产物，不入库**：

根目录 `.gitignore` 已有 `src-tauri/bin/*` + `!src-tauri/bin/README.md`，
因此本 README 入库，而二进制与清单文件被忽略；CI 每次构建前重新供给。

## 1. 命名规范（Tauri 2）

`src-tauri/tauri.conf.json` 中声明：

```json
"bundle": {
  "externalBin": ["bin/qpdf", "bin/tesseract"],
  "resources": ["resources/*"],
  "createUpdaterArtifacts": true
}
```

Tauri 2 要求声明项对应的**实际文件名**为：

```
src-tauri/bin/<声明名>-<target-triple>[.exe]
```

Windows 需要 `.exe` 后缀，macOS 不需要；文件会被放到主可执行文件的相邻目录，
运行时由 `engine/sidecar.rs` 定位。`resources/*` 则整体打包进
`.app/Contents/Resources/`（macOS）或安装目录（Windows），用于 tessdata 与 PDFium 动态库。

## 2. 三平台文件名示例

| 平台 / runner | target triple | qpdf | tesseract | resources 中的 PDFium |
|---|---|---|---|---|
| Windows x64 (`windows-latest`) | `x86_64-pc-windows-msvc` | `qpdf-x86_64-pc-windows-msvc.exe` | `tesseract-x86_64-pc-windows-msvc.exe` | `pdfium.dll` |
| macOS arm64 (`macos-14`) | `aarch64-apple-darwin` | `qpdf-aarch64-apple-darwin` | `tesseract-aarch64-apple-darwin` | `libpdfium.dylib` |
| macOS x64 (`macos-13`) | `x86_64-apple-darwin` | `qpdf-x86_64-apple-darwin` | `tesseract-x86_64-apple-darwin` | `libpdfium.dylib` |

配套资源（由同一脚本写入 `src-tauri/resources/`）：

```
resources/tessdata/eng.traineddata       (tessdata_fast, Apache-2.0)
resources/tessdata/chi_sim.traineddata  (tessdata_fast, Apache-2.0)
resources/pdfium.dll | libpdfium.dylib  (PDFium, BSD-3-Clause)
```

`sidecar-manifest.json` 记录每个文件的版本、来源 URL / brew 公式、sha256、
架构、是否签名，以及 `TODO(verify-url)` 待核实项，可用于启动自检（CONTRACT E2001/E2002）。

## 3. 本地供给方式

```bash
# 只打印计划（不联网、不写文件），三平台都可用
node scripts/provision-sidecars.mjs --target x86_64-pc-windows-msvc --dry-run
node scripts/provision-sidecars.mjs --target aarch64-apple-darwin --dry-run
node scripts/provision-sidecars.mjs --target x86_64-apple-darwin --dry-run

# 在线下载（上游 URL 见脚本顶部 SOURCES 常量表，多数标注 TODO(verify-url)）
node scripts/provision-sidecars.mjs --target <triple>

# 离线兜底：从本地缓存目录按文件名递归拷贝（推荐在 URL 未核实 / 无网络时使用）
node scripts/provision-sidecars.mjs --target <triple> --offline ./sidecar-cache
```

`--offline` 目录支持 `<dir>/<triple>/<file>` 或 `<dir>/<file>` 两种布局，
所需文件名见各 unit 的 `offlineNames`（qpdf[.exe] / tesseract[.exe] /
eng.traineddata / chi_sim.traineddata / pdfium.dll|libpdfium.dylib）。

脚本会校验：文件存在、非空、sha256 一致、CPU 架构（Windows 读 PE Machine、
macOS 读 Mach-O cputype，并在 macOS 上用 `lipo -archs` 复核）。
失败以明确退出码终止（2 参数 / 3 网络 / 4 解压 / 5 校验 / 6 架构），**不会静默成功**。

### 已知运行时依赖（TODO，需协调）

- qpdf Windows mingw64 的 `qpdf.exe` 依赖同目录 `qpdf29.dll`、
  `libgcc_s_seh-1.dll`、`libstdc++-6.dll`、`libwinpthread-1.dll`。
  单独部署 `qpdf.exe` 在干净机器上会因缺 DLL 失败。当前 `bundle.externalBin`
  只声明了 qpdf/tesseract 两项，需与负责 `tauri.conf.json` 的 Agent 协调把
  依赖 DLL 作为 resources 随包并置于 DLL 搜索路径，或改用静态链接构建。
- macOS 用 `brew` 安装后拷贝的 tesseract 动态链接 brew 的 `libtesseract` /
  `liblept` 等 dylib，生产应改用静态构建或把依赖 dylib 一并随包并修正 rpath。

## 4. macOS 必须逐文件签名

Apple 的公证（notarization）与 Hardened Runtime 要求 **所有嵌套代码**都有有效签名，
否则 `notarytool` 会报 "binary has no signature" 或 Gatekeeper 拦截。
`--deep` 只作为最后对整个 `.app` 的兜底，正确顺序是先签内部再签外部：

```bash
SIGN_IDENTITY="Developer ID Application: <Team Name> (<TEAMID>)"
APP="src-tauri/target/aarch64-apple-darwin/release/bundle/macos/PayOnce PDF.app"

# 1) 逐文件签名 resources 内的动态库（PDFium）
codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" \
  "$APP/Contents/Resources/libpdfium.dylib"

# 2) 逐文件签名 externalBin sidecar（它们位于 .app/Contents/MacOS/）
for f in "$APP/Contents/MacOS"/qpdf-* "$APP/Contents/MacOS"/tesseract-*; do
  codesign --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$f"
done

# 3) 最后签整个 bundle
codesign --deep --force --options runtime --timestamp --sign "$SIGN_IDENTITY" "$APP"
codesign --verify --deep --strict --verbose=2 "$APP"
```

注意：

- 签名必须在 `tauri build` 产出 `.app` 之后、打 dmg / 提交公证之前完成；
  `tauri build` 本身不会用 Developer ID 给你的 sidecar 签名。
- 逐文件签名后再用 `--deep` 会重签嵌套项，务必保证顺序与 `codesign --verify` 通过。
- 之后还需 `xcrun notarytool submit --wait` 公证、`xcrun stapler staple`，
  以及把公证票据 staple 到 `.app` / `.dmg`（见 `.github/workflows/build.yml`）。
- `tessdata/*.traineddata` 是纯数据文件，不需要签名。
- Windows 侧由 EV 证书 / Azure Trusted Signing 对安装包签名（见 workflow）。

## 5. 许可

| 组件 | 许可 | 来源 |
|---|---|---|
| qpdf | Apache-2.0 | qpdf GitHub releases |
| Tesseract | Apache-2.0 | tesseract-ocr / UB Mannheim (Windows) / brew (macOS) |
| tessdata_fast | Apache-2.0 | tesseract-ocr/tessdata_fast |
| PDFium | BSD-3-Clause | bblanchon/pdfium-binaries |

许可红线（PRD 9.2）：禁止 MuPDF / Ghostscript（AGPL-3.0）；LibreOffice 延后评估。
