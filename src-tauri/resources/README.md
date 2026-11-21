# Bundled resources

This directory is shipped inside the app (`bundle.resources: ["resources/*"]`).
The provisioning script (`scripts/provision-sidecars.mjs`) downloads the
platform-specific files here before `tauri build`:

| File | Purpose | License |
|---|---|---|
| `tessdata/eng.traineddata` | Tesseract English model | Apache-2.0 |
| `tessdata/chi_sim.traineddata` | Tesseract Simplified Chinese model | Apache-2.0 |
| `pdfium.dll` / `libpdfium.dylib` | PDFium renderer (BSD-3) | BSD-3-Clause |
| `THIRD_PARTY_LICENSES.md` | License texts / attribution | — |

`qpdf` and `tesseract` executables are Tauri sidecars (`bundle.externalBin`),
named with the target triple, e.g. `bin/qpdf-x86_64-pc-windows-msvc.exe`,
`bin/tesseract-aarch64-apple-darwin`. They are located next to the app
executable at runtime.

**License red line (PRD 9.2):** PDFium (BSD-3), qpdf (Apache-2.0),
Tesseract (Apache-2.0), lopdf/image (MIT/Apache-2.0) are allowed.
MuPDF and Ghostscript (AGPL-3.0) are forbidden. LibreOffice is deferred.
