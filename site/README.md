# PayOnce PDF 官网（site/）

静态官网，属于 PRD §20「官网」交付物的一部分。纯 HTML + 一份共享 CSS，**无构建步骤、无外部 CDN、无 JavaScript、无脚本追踪、无 emoji**。

## 1. 页面清单

| 文件 | 语言 | 内容 | 依赖 |
|---|---|---|---|
| `index.html` | 英文 | 主站：首屏价值主张、**15 张功能卡**（F1–F7 + F10–F16）、三不卖点、37% 数据锚点、价格与条款、不支持清单、占位下载入口、FAQ/隐私/验证入口 | `styles.css` |
| `index.zh.html` | 简体中文 | 中文主站，结构与英文主站一致（同为 15 工具 + 中文水印 / 表单脱敏书签 / 文本转 PDF / 打印扩展） | `styles.css` |
| `pricing.html` | 英文 | **已定价 $39 / ¥199**（首发 14 天早鸟 $29 / ¥149，见 `../docs/pricing-decision.md`）、买断 vs 订阅对比（竞品价「需复核」）、12 个月 1.x 更新、2.x 五折、7 天退款、**15 工具**清单、单档与国内渠道说明、团队采购说明 | `styles.css` |
| `faq.html` | 英文 | **19 条 FAQ**，覆盖本地/离线/OCR 局限与 42 种语言包/压缩/加密/签名/激活/断网/试用到期/更新/Gatekeeper/Word-Excel/退款，新增表单脱敏书签、中文水印与文本转 PDF、打印邮件与语言包联网口径 | `styles.css` |
| `privacy.html` | 英文 | 不收集数据、文件内容与文件名永不上传、本地日志默认关闭、**三类可枚举网络请求**（激活 / 可选更新检查 / 用户主动安装 OCR 语言包）、激活仅密钥 + 不可逆指纹哈希、无遥测无账号、第三方组件、联系方式占位 | `styles.css` |
| `verify.html` | 英文 | **重点**：自行验证「文件不上传」——三类请求清单、Windows Wireshark/Fiddler、macOS Little Snitch/Wireshark 具体步骤、**15 工具**断网测试、语言包请求核对、预期抓包结果、更新签名自检（`tests/verify-updater-signature.py`）、异常反馈 | `styles.css` |
| `styles.css` | — | 共享样式：浅色主题、系统字体、teal `#0f766e`、导航/卡片/表格/FAQ/页脚、高对比度支持 | — |
| `README.md` | 中文 | 本文件：清单、用法、部署前占位项 | — |

导航完整性：每个页面顶部导航与底部页脚都提供全部 6 个入口（首页、定价、FAQ、隐私、验证、语言切换），页面之间可互相到达。语言切换：英文页 → `index.zh.html`（中文），中文主站 → `index.html`（EN）。

> 说明：`pricing / faq / privacy / verify` 目前仅有英文版；中文子页面待补齐（已在 `index.zh.html` 内用 `TODO(中文本地化)` 标注）。

## 2. 如何使用

纯静态，无需 Node/npm 构建：

```bash
# 本地预览（任选其一）
# 方式 A：直接用浏览器打开 site/index.html
# 方式 B：起一个本地静态服务器（可正确加载相对链接）
cd site
python -m http.server 8080
# 然后访问 http://localhost:8080/ 与 http://localhost:8080/index.zh.html
```

部署：把整个 `site/` 目录（含 `styles.css`）上传到任意静态托管即可，入口为 `index.html`。

<!-- TODO(站内文档链接): 页面中有指向 ../docs/*.md 与 ../src-tauri/resources/*.md 的相对链接（用户手册、错误码、支持模板、第三方许可）。部署时需把这些 Markdown 一并发布并改成站内可读页面，或改为真实 URL；当前链接指向仓库内文件 -->
注意：`privacy.html` / `verify.html` / `faq.html` 中的若干链接指向仓库内的 Markdown（`../docs/user-manual.md`、`../docs/error-codes.md`、`../docs/support-template.md`、`../src-tauri/resources/THIRD_PARTY_LICENSES.md`）。部署前需处理这些链接。

## 3. 设计约定

- 浅色主题、系统字体、teal `#0f766e` 点缀（沿用 landing 的配色与简洁风格）。
- 所有颜色变量集中在 `styles.css` 的 `:root`，改一处即可全站生效。
- 禁止：登录、上传、云端、订阅按钮、遥测、AI 功能宣传、外部 CDN、脚本追踪、emoji。
- 下载入口是**占位**，明确说明「安装包需签名后发布」，不伪造可下载链接。
- 诚实口径：不写「100% 准确」「永久免费」「lifetime updates」；明确列出「不支持」清单。
- 所有占位内容用醒目的 `TODO(...)` 注释标出（HTML 注释），便于检索：`grep -R "TODO(" site/`。

## 4. 部署前需替换的占位项清单

> 检索命令：`grep -R "TODO(" site/`。以下为全部占位项，按文件分组。

### 全局（每个页面页脚）
- `TODO(联系/版权)`：替换为真实运营主体名称、版权年份、支持邮箱。
- `hello@payoncepdf.com`：全站统一联系邮箱；DNS / MX 由人工配置，见 `docs/RENAME-MIGRATION.md`。
- `https://payoncepdf.com`：唯一对外域名（DNS / 证书由人工处理，见 `docs/RENAME-MIGRATION.md`）。

### index.html / index.zh.html
- `TODO(下载链接)`：安装包签名并发布后，把 Windows `.msi/.exe`、macOS `.dmg` 的 `href="#"` 占位替换为真实直链（当前 `aria-disabled="true"`，不可点击）。
- `TODO(发布信息)`：补上版本号、发布日期、SHA-256 校验值、更新日志地址。
- `TODO(购买链接)`（价格区 note 与 `pricing.html`）：签名发布后填入 Gumroad（海外）/ 爱发电（国内）真实商品链接。
- `index.zh.html` 额外：`TODO(中文本地化)`（补齐中文子页面）。

### pricing.html
- `TODO(竞品定价复核)`：PayOnce PDF 自身定价已定（$39 / ¥199，见 `../docs/updf-parity-decision.md`）；上线前仍需逐条核对 Adobe / Smallpdf / iLovePDF / PDF Expert / PDFelement 的当前官方定价与计费方式，去掉「需复核」标记。
- `TODO(退款入口)`：填入 Gumroad / 爱发电 退款入口与 `hello@payoncepdf.com`。
- `TODO(购买链接)` / `TODO(purchase links: Gumroad / 爱发电)`：真实商品链接。
- `TODO(团队采购)` / `hello@payoncepdf.com` / `TODO(team purchasing terms)`：批量购买方式、发票/收据开具方式、商务邮箱、团队条款。

### faq.html
- `TODO(macOS 签名)`：Developer ID 与公证流水线就绪后，补充签名主体与 `spctl/codesign` 验证命令。
- `TODO(退款入口)` / `hello@payoncepdf.com`：退款入口与邮箱。
- 全局页脚占位同上。

### privacy.html
- `TODO(服务端地址)` / `TODO(activation host)` / `TODO(update-check host)`：上线前公布激活服务端与更新检查端点的真实主机名，供用户核对抓包结果。
- `TODO(日志路径)` / `TODO(local log path on Windows / macOS)`：确认并记录两平台本地日志文件路径、开关与清理方式。
- `hello@payoncepdf.com`：隐私 / 安全联系邮箱已统一；反馈表单地址仍为上线前待办。
- 全局页脚占位同上。

### verify.html
- `TODO(服务端地址)` / `TODO(activation host)` / `TODO(update-check host)`：同上，公布真实端点主机名。
- `hello@payoncepdf.com`：安全 / 隐私反馈邮箱已统一；反馈表单地址仍为上线前待办。
- 全局页脚占位同上。

### 跨文件（`docs/` 与根目录，非本目录但同批交付）
- `docs/user-manual.md`：日志路径、快捷键与实现核对、支持邮箱。
- `docs/error-codes.md`：支持邮箱（中英模板）。
- `docs/support-template.md`：支持邮箱与退款/激活入口。
- `docs/test-report.md`：真机/双平台待测项、fixtures 与测试脚本。
- `README.md`（项目根）：域名、购买/下载链接、签名证书、CI、sidecar 供给。
- `src-tauri/resources/THIRD_PARTY_LICENSES.md`：随包许可证文本路径与获取地址复核。

## 5. 已知限制 / 待外部条件

- 安装包未签名、未发布，下载与购买链接均为占位（需 Windows EV / Azure Trusted Signing、macOS Developer ID + 公证）。
- 竞品定价标注「需复核」，上线前需人工核对。
- 中文子页面（`pricing / faq / privacy / verify`）尚未本地化。
- 激活/更新端点主机名为占位，需在发布前公布并写入 `privacy.html` / `verify.html`。
