# PayOnce PDF 官网 · Cloudflare Pages 静态站

深色多页官网，参考 `zhuazhua.video` 的信息架构与转化打法（痛点打击 + 一次买断 + 本地隐私），内容为 PDF 编辑器主题。纯静态：**共享 CSS + HTML，无框架、无构建步骤、无外部 CDN、无追踪脚本**（仅 JSON-LD 结构化数据）。

> 域名：**payoncepdf.com**（项目既有域名；`payoncepfd.com` 是提问里的笔误，已统一为 pdf 在前的正确拼写）。

## 1. 信息架构

导航固定四项 **首页 / 下载 / 价格 / 帮助**，与参照站一致，各自独立 URL、点击跳转到对应页面（非锚点）：

| 路径 | 中文文件 | 英文文件 | 主要内容 |
|---|---|---|---|
| `/` | `index.html` | `en/index.html` | 状态条 + 导航 + Hero（左钩子/右演示卡）+ Bento 6 卡 + 3 列对比表 + 痛点对照 + 平台支持 + 收尾 CTA + 法务 |
| `/download` | `download.html` | `en/download.html` | 三步开始 + 平台支持 + 安装与安全 FAQ + 试用 CTA |
| `/pricing` | `pricing.html` | `en/pricing.html` | 一次买断双卡（含 Gumroad 占位按钮）+ 对比表 + 付费 FAQ |
| `/help` | `help.html` | `en/help.html` | FAQ 帮助中心（隐私/编辑边界/系统授权）+ 联系方式 |

首页两个 CTA 分别指向下载页与价格页，不再放在同一页。`/en/` 下为英文镜像四页，结构完全同构。

**URL 策略（避免本地 404）**：上表是线上对外使用的干净 URL，由 `_redirects` 重写到对应 `.html` 文件；8 个页面内所有导航、按钮、语言切换一律直连 `.html`（如 `/download.html`、`/en/index.html`），因此 `python -m http.server` 等任意静态服务器都能直接点击浏览。`canonical` / `hreflang` 仍指向干净 URL，权重集中且不与 `_redirects` 冲突。`_redirects` **只在 Cloudflare Pages 生效**，本地静态服务器不解析它——这正是用普通静态服务器访问 `/download` 会看到 `Error code: 404 / File not found.` 的原因。

### 转化流程：两条入口，同一份 7 天试用

参考 `zhuazhua.video` 的下载页 / 价格页设置（下载页直接给安装包、价格页档位卡 + Gumroad 订单邮箱判定授权与退款、全站强调「先免费试用 7 天」），本站与客户端的流程为：

| 入口 | 点击后 | 试用到期 |
|---|---|---|
| 下载 / 免费试用 | 获取同一个安装包，装完自动进入 7 天全功能试用，无需账号 / 绑卡 / 激活码 | 软件内**弹出缴费窗口**，点击缴费 → Gumroad 一次买断 → 输入激活码 → 永久 |
| 在 Gumroad 购买（先付款） | 先结账付款拿激活码，再装同一个包；安装后仍先显示 7 天试用 | **自动转入买断永久模式**，不再弹窗 |

客户端判定只取决于授权状态（`src/lib/commerce.ts` 的 `shouldShowPaywall()`）：仅 `expired` 弹窗，`active` 永不弹；试用到期只拦导出，不删除或改写文件。两个入口装的是同一个包，不存在「功能阉割版试用」。

## 2. 文件清单

| 文件 | 作用 |
|---|---|
| `index.html` / `download.html` / `pricing.html` / `help.html` | 中文四页（lang=zh-CN） |
| `en/index.html` / `en/download.html` / `en/pricing.html` / `en/help.html` | 英文四页（lang=en），镜像文案 |
| `styles.css` | 8 页共享样式（深色主题、响应式、导航当前页高亮、Gumroad 占位样式） |
| `../scripts/serve-cloudflare-site.py` | 零依赖本地预览服务器（**在部署目录之外**，模拟 `_redirects` 的干净路径与 301 跳转） |
| `wrangler.toml` | Cloudflare Pages 项目配置（`pages_build_output_dir = "."`） |
| `_headers` | 安全响应头（CSP / HSTS / nosniff / SAMEORIGIN / Referrer-Policy / Permissions-Policy）、`README.md` 与 `wrangler.toml` 的 noindex、`styles.css` 短缓存 |
| `_redirects` | www → 主域 301；`/download` `/pricing` `/help` 及 `/en/` 四页的干净路径 rewrite；旧链接兼容 |
| `README.md` | 本文件 |

**多语言实现**：沿用项目既有"文件级镜像"惯例，不引入 i18n 框架。语言切换器为普通链接，各页跳到对应语言同一页；`hreflang`（zh-CN/en/x-default）+ `canonical` 已逐页标注。任务提到的 es/pt/de/fr 在本项目**不存在**，未伪造。

## 3. 部署到 Cloudflare Pages（两种方式，选一）

### 方式 A：wrangler CLI（本地一键，最快）

```bash
npx wrangler login                       # 浏览器授权 Cloudflare 账号
npx wrangler pages dev .                 # 本地预览（可选）
cd E:/PayOncePDF/site-cloudflare
npx wrangler pages deploy . --project-name payoncepdf --branch main
```

首次部署会创建 Pages 项目并返回 `*.pages.dev` 预览地址；后续同名部署即更新。

### 方式 B：GitHub 集成（推荐，改代码自动上线）

1. Cloudflare Dashboard → **Workers & Pages** → **Create** → **Pages** → **Connect to Git**
2. 选仓库 `Sgzhengg/PayOncePDF`，分支 `main`
3. 构建配置：Framework preset **None**；Build command **留空**；Build output directory **`site-cloudflare`**
4. 保存后自动部署；以后往 `main` 推 `site-cloudflare/` 改动即自动更新。

> Cloudflare Pages 走 Git webhook 自动部署，与桌面安装包 CI 是两条独立流水线。**上线前请在 Cloudflare 侧把 `quality` 门禁或人工复核设为部署前置**，见根目录 `DEPLOYMENT.md`。

## 4. 绑定自定义域名 payoncepdf.com（需你在 Cloudflare 操作）

前提：域名已添加到你的 Cloudflare 账号且 NS 已指向 Cloudflare。

1. Pages 项目 → **Settings** → **Domains** → **Add custom domain**，填 `payoncepdf.com`
2. Cloudflare 自动创建指向 Pages 的 CNAME/DNS 记录，并签发 Universal SSL，等待状态 **Active**
3. `www` 已在 `_redirects` 做 301 跳主域；如需 `www` 也可直接访问，在 Domains 里同样添加 `www.payoncepdf.com`
4. 根域 `@` 用 Cloudflare 的 **CNAME flattening**（Pages 默认支持，无需手动 A 记录）

**重要**：本步骤涉及你的 Cloudflare 账号授权与域名 DNS 控制权，我无法代做。

## 5. 上线前必须替换的占位项（诚实，不伪造）

| 位置 | 现状 | 上线前动作 |
|---|---|---|
| **Gumroad 购买按钮**（pricing 页双卡主按钮，中/英两版） | 金色按钮「在 Gumroad 购买 →」，`href="#gumroad-pending"` + `aria-disabled="true"`，点击跳到同页说明框，**不是**假结账链接 | 商品创建后，把 `href="#gumroad-pending"` 替换为真实 Gumroad 商品 URL（如 `https://你.gumroad.com/l/payoncepdf`），并移除 `aria-disabled` 与说明框 |
| **客户端缴费地址**（试用到期弹窗 / 授权面板的「点击缴费」） | `src/lib/commerce.ts` 的 `PAYMENT_URL = "https://payoncepdf.com/pricing"`，指向价格页，不伪造 Gumroad 直链 | 商品就绪后改为真实 Gumroad 结账直链；官网购买按钮同步替换 |
| 试用申请按钮（download 页 + 各页 CTA） | `mailto:hello@payoncepdf.com` 申请试用 | 试用通道开放后换成真实下载/邮件列表链接 |
| 「订阅上线提醒」小字 | `mailto:` 订阅 | 有正式邮件列表后替换 |
| 移动端订阅提醒 | `mailto:` 订阅 | 同上 |
| 联系邮箱 | `hello@payoncepdf.com` | 需确认邮箱已开通并配置 MX |
| 法务链接 | 各页 footer 的段落 | 若有独立法务页，替换锚点为真实 URL |

**Gumroad 配置提示**：商品就绪后需同时更新两处——`pricing.html` 与 `en/pricing.html` 里购买按钮的 `href`；两页的 `#gumroad-pending` 说明框可在商品上线后删除。结构化数据（JSON-LD）里的 `offers[].url` 目前指向 pricing 页，商品创建后可改为直链 Gumroad。

定价 `$39 / ¥199`（单一标价，不设早鸟/划线折扣）、12 个月 1.x 更新、7 天试用/退款、最多 3 台设备——来自 `config/product.json` 与 `docs/pricing-decision.md`，已拍板。

## 6. 与旧站 site/ 的关系

- `site/`：浅色多页站（首页/定价/FAQ/隐私/验证/EN），信息更全，可作文档型站点保留。
- `site-cloudflare/`：深色四页转化站，跑在 Cloudflare Pages + `payoncepdf.com`。
- **唯一对外部署物是 `site-cloudflare/`**：`site/` 是历史归档站，已标记禁止部署（见根目录 `DEPLOYMENT.md`）；`dist/` 是浏览器预览版 React 应用，同样禁止部署。

## 7. 本地预览

> 预览脚本位于 `scripts/serve-cloudflare-site.py`，**不在部署目录内**：Pages 会把 `site-cloudflare/` 下的所有文件当作静态资源发布。

### 最简单：普通静态服务器（推荐）

```bash
cd E:/PayOncePDF/site-cloudflare
python -m http.server 8899 --bind 127.0.0.1
# 中文首页：http://127.0.0.1:8899/
# 英文首页：http://127.0.0.1:8899/en/
```

从 `http://127.0.0.1:8899/` 进入后点导航即可，四个标签、语言切换、页脚链接都能正常跳转（站内链接已全部直连 `.html`）。

### 验证线上干净 URL：带 `_redirects` 的预览服务器

```bash
python ../scripts/serve-cloudflare-site.py 8899
# 或 npx wrangler pages dev .   （需 wrangler，会读取 _redirects）
```

`../scripts/serve-cloudflare-site.py` 基于 Python 标准库、零依赖，内置与 `_redirects` 一致的 200 rewrite 与 301 跳转，可验证 `/download`、`/pricing`、`/help`、`/en/…`，以及旧链接 `/index.en.html`、`/zh/` 的行为。

> ⚠️ 若浏览器出现 `Error response / Error code: 404 / Message: File not found. / Error code explanation: 404 - Nothing matches the given URI.`，这是 Python `http.server` 的默认 404 页，说明你用普通静态服务器访问了干净路径 `/download`（或点击了旧版页面里的干净链接）。**回到 `http://127.0.0.1:8899/` 根路径刷新后再点导航**，或改用 `python ../scripts/serve-cloudflare-site.py`。HTML 修改会被静态服务器实时读取，无需重启。

**内容诚实降级**（与项目已拍板事实一致）：高保真 Word/Excel 互转 → 文字层导出（CONTRACT.md 禁止宣称）；无损压缩 → 压缩且文字层保留；1.x 全版本更新 → 12 个月 1.x；签章 → AcroForm 文本域签名（复选框/图章不支持）；下载试用 → 邮件申请占位；Gumroad → 明确占位不伪造结账页。原则：不编造功能、不伪造链接。
