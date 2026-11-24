# 部署边界：唯一对外部署物

> 收口上线前 CodeReview 的 P0-8 / P2-6。本文件是部署流程的**唯一事实源**。

| 目录 | 角色 | 能否对外部署 |
| --- | --- | --- |
| `site-cloudflare/` | **唯一官网**：Cloudflare Pages，中英 8 页深色静态站 | ✅ 是 |
| `site/` | 2026-11-16 时代的浅色 6 页旧站（15 工具口径、旧定价、指向 `../docs/*.md` 的死链） | ❌ **禁止**，历史归档 |
| `dist/` | `npm run build` 产出的浏览器预览版 React 应用；UI 常驻「预览模式：未调用本地引擎」，预览模式下任意合规 key 会把授权显示为 active | ❌ **禁止**，非官网 |

## 唯一部署配置

- Cloudflare Pages 项目 `payoncepdf`：Framework preset **None**、Build command **留空**、Build output directory **`site-cloudflare`**。
- `site-cloudflare/wrangler.toml` 的 `pages_build_output_dir = "."` 是**以 `site-cloudflare/` 自身为根**；不要把仓库根目录设为输出目录。
- `site-cloudflare/` 内除 8 个 HTML、`styles.css`、`_headers`、`_redirects` 外，`README.md` 与 `wrangler.toml` 也会被 Pages 当成静态资源发布；`_headers` 已给这两者加 `X-Robots-Tag: noindex`。
- 本地预览脚本已从部署目录移到 `scripts/serve-cloudflare-site.py`；`__pycache__/` 与任何 `.pyc` **不得**进入部署目录（`.gitignore` 已忽略）。

## 域名与 DNS 现状（2026-11-24 实测）

> 触发原因：用户在浏览器输入 `payoncepdf.com` 打不开。这不是官网代码缺陷，也不是本机网络故障。

| 项 | 实测结果 | 依据 |
| --- | --- | --- |
| 域名注册 | **已注册**，阿里云万网 / HiChina（IANA 1599），2026-11-16 注册、2027-11-16 到期，状态 `active` | Verisign RDAP `payoncepdf.com` |
| 权威 NS | `dns19.hichina.com` / `dns20.hichina.com`（阿里云默认，**未接入 Cloudflare**） | RDAP nameservers + 本机 `nslookup -type=NS` |
| A / AAAA / CNAME | 向权威 NS 查询仅返回 SOA 否定应答，**没有任何指向网站的记录** | `nslookup -type=A/-type=CNAME payoncepdf.com dns19.hichina.com` |
| Cloudflare Pages 项目 | **已创建并部署**（2026-11-24），固定地址 **https://payoncepdf.pages.dev/**；同一账号下另有 3 个项目（`zhuazhua` / `git-relay` / `demoforge-video`） | `wrangler pages project create` + `pages deploy`，详见「首次部署记录」 |
| Wrangler 认证 | 本机用户级 `CLOUDFLARE_API_TOKEN` 有效，`whoami` 返回单账号（邮箱与 Account ID 已脱敏，仅本地可见）。Token 认证与 OAuth 互斥 | `wrangler whoami` |
| 结论 | **站点已在 Pages 上线并可访问**；但自定义域仍无 DNS 记录且未绑定，浏览器输 `payoncepdf.com` 必然打不开 | 已排除「只有本机代理异常」的可能 |

**`npx wrangler login` 报错的真实原因**：本机 `HKCU:\Environment` 设置了用户级 `CLOUDFLARE_API_TOKEN`，Wrangler 检测到后强制走 Token 认证，于是拒绝 `X [ERROR] You are logged in with an API Token. Unset the CLOUDFLARE_API_TOKEN in the environment to log in via OAuth.`。**这不是故障，也不是没登录** —— `whoami` 与 `pages project list` 均已成功。详见下方「Wrangler 认证模式」。

注意域名拼写是 **`payoncepdf.com`**（pdf 在前）；`payoncepfd.com` 是早期需求里的笔误，从未注册或使用。

### 让 `payoncepdf.com` 真正可访问的剩余两步（第 1 步已完成）

**已完成**：Pages 项目 `payoncepdf` 已创建并部署，固定地址 **https://payoncepdf.pages.dev/**，中英 8 页线上逐页验收通过（见「首次部署记录」）。现在可以先打开这个地址看官网，不必等自定义域生效。

1. **接入 Cloudflare**：Dashboard → *Add a site* → `payoncepdf.com`，拿到分配的两个 nameserver；到阿里云万网把该域名的 DNS 服务器改成这两个 NS，等待生效（10 分钟～数小时）。
2. **绑自定义域**：Pages → *Settings* → *Domains* → Add `payoncepdf.com`（可另加 `www.payoncepdf.com`，`_redirects` 已做 www→主域 301），等 Universal SSL 状态变 **Active**；根域用 CNAME flattening，**不要在阿里云手工填猜测的 A 记录**。

后续改代码可用 `npm run site:deploy` 重新部署；或改用 Git 集成（仓库 `Sgzhengg/PayOncePDF` / 分支 `main`，Framework preset **None**、Build command **留空**、Build output directory **`site-cloudflare`**），但须按「上线前置」处理 webhook 绕过 CI 的问题。**Add a site、改阿里云 NS、绑自定义域**这些账号/域名控制权操作仍必须用户本人在控制台完成。

## Wrangler 认证模式（API Token 与 OAuth 互斥）

| 模式 | 判定 | 是否要 `wrangler login` |
| --- | --- | --- |
| **A. API Token**（本机现状） | `CLOUDFLARE_API_TOKEN` 已设置（用户级 `HKCU:\Environment`，len=53） | ❌ **不要执行**，执行必报 `You are logged in with an API Token` |
| B. OAuth | 未设置该变量，`~/.wrangler/config/default.toml` 等凭据文件存在 | ✅ 需要，浏览器授权须本人操作 |
| C. 无凭据 | 两者皆无 | ✅ 需要 |

- `wrangler whoami` 是唯一精确判定方式；只读检测脚本 `scripts/deploy-cloudflare-site.mjs` 的 `detectAuth()` 会报告当前模式，**绝不打印 Token 内容**。
- `pages deploy` 没有 `--account-id` 参数；账号 ID 通过 `CLOUDFLARE_ACCOUNT_ID` 环境变量提供，脚本支持 `--account-id <ID>`（仅注入 wrangler 子进程，不改用户环境变量）。本机未设置该变量时由 Wrangler 从单账号凭据自动推断。
- Token 权限在 Dashboard → Profile → API Tokens 查看。`whoami` 曾提示缺少 `User->User Details->Read`（读不到邮箱），但 `pages project list` 成功，说明账号/Pages 读权限具备；**写权限（`Cloudflare Pages:Edit`）需实际部署才能确认**。
- 切回 OAuth：仅当前窗口清除（cmd `set CLOUDFLARE_API_TOKEN=` / PowerShell `Remove-Item Env:CLOUDFLARE_API_TOKEN`），或在「系统属性 → 环境变量 → 用户变量」删除该条目。脚本不会替你 unset、覆盖或写入凭据；该变量可能服务于本机其他项目，删除前请自行确认。

## 首次部署记录（2026-11-24，站点已上线）

| 项 | 结果 |
| --- | --- |
| Wrangler | 4.167.0，`npx --yes wrangler`，经本机 `CLOUDFLARE_API_TOKEN` 认证 |
| 项目 | `payoncepdf`，production branch `main`。**Wrangler 4.x 不再由 `pages deploy` 自动建项目**，首次必须 `pages project create`（脚本已自动处理） |
| 固定地址 | **https://payoncepdf.pages.dev/** |
| 部署 URL | 首版 `dad56a52.payoncepdf.pages.dev` → 修复版 `b1b7624b.payoncepdf.pages.dev` |
| 上传内容 | 11 个静态文件 + `_headers` + `_redirects`（第二次为增量，仅传 1 个变更文件） |
| 线上验收 | `npm run site:verify -- https://payoncepdf.pages.dev`：本地门禁 10/10、13 条 HTTP 路径全可达、CSP/HSTS 命中，**0 阻断** |
| 交叉验证 | 独立网络通道取到完整中文首页（标题、导航、`$39 / ¥199`、7 天试用文案）；`/download` `/pricing` `/help` `/en/` `/en/pricing` 均 200；旧链接 `/index.en.html`、`/zh/` 均 301 |
| 自定义域 | **未绑定**：`payoncepdf.com` 仍无 A/CNAME 记录、DNS 未接入 Cloudflare，浏览器直接输该域名仍打不开 |

### 上线时抓到并修复的真实阻断项：308 无限重定向循环

首版部署后，线上 `/download.html` → 308 → `/download`，而 `/download` 又 308 回自身，`curl -L` 报 `Maximum (50) redirects followed`，**点导航会打不开页面**。根因是 `_redirects` 里 7 条 `200` rewrite（`/download → /download.html 200` 等）与 Cloudflare Pages **原生 clean URL** 冲突：Pages 本来就把 `/download` 映射到 `download.html` 返回 200，并把 `/download.html` 308 到 `/download`；再显式 rewrite 会让两者互相来回。本地 `serve-cloudflare-site.py` 模拟的是内部 200 映射，所以本地 13/13 通过、没能暴露这个线上语义差异。

修复：删掉全部 7 条 `200` rewrite，`_redirects` 只保留 www 与旧链接的 3 条 301；`verify-cloudflare-site.mjs` 的 HTTP 验收改为 `redirect: 'follow'`（把 Pages 的 308 视为预期，循环明确报「疑似重定向循环」），并新增「禁止 200 rewrite」门禁与 3 条契约测试；`serve-cloudflare-site.py` 注释说明它只是本地模拟。修复后重新部署并验收：干净路径 200、`.html` 308→干净路径 200，循环消失。

> 教训：本地模拟器只能复现「路径映射结果」，复现不了 Pages/CDN 自身的重定向语义；凡涉及 `_redirects` 的变更，必须以线上 HTTP 验收为准。

## 上线前置（质量门禁）

1. GitHub Actions `quality` job（`npm run typecheck` / `npm test` / `npm run build` / `python scripts/verify-public-key.py`）必须绿；
2. Windows `build` job 在供给 sidecar 后、`tauri build` 前会跑 `cargo test --lib`，打包后跑「发行包公钥收录复核」，两者都必须绿；
3. **Cloudflare 的 Git 集成是独立 webhook，不受上述 CI 约束**。上线前需在 Cloudflare 侧关闭「推 main 即自动部署」，或把部署动作改为由通过门禁的 workflow 触发 / 人工确认，否则任何回归仍可直达生产。

## 一键部署脚本与验收（2026-11-24 新增）

| 命令 | 作用 | 是否产生写操作 |
| --- | --- | --- |
| `npm run site:check` | 部署前门禁 + 环境状态报告 | ❌ 否，纯本地只读 |
| `npm run site:deploy` | 门禁通过后执行 `wrangler pages deploy` | ✅ 是，向 Cloudflare Pages 上传站点文件 |
| `npm run site:verify -- <URL>` | 上线后逐页验收（也可不带 URL 只查本地文件） | ❌ 否，只读 HTTP GET |

- `scripts/deploy-cloudflare-site.mjs`：默认 `--check`，只有显式 `--deploy` 才调用 Wrangler。执行前强制跑 `scripts/verify-cloudflare-site.mjs` 的本地门禁，失败即中止；`--deploy` 先跑 `whoami` 精确验明认证状态，失败按当前模式给出 Token 权限排查或 `npx wrangler login` 指引。**不打开浏览器、不自动执行 login、不读写凭据文件、不 unset 或覆盖 `CLOUDFLARE_API_TOKEN`、不修改用户级环境变量、不绑定自定义域、不修改 DNS**。可用 `--project` / `--branch` / `--account-id` 覆盖默认值。
- `scripts/verify-cloudflare-site.mjs`：本地门禁覆盖 13 个必需文件、8 页标签配平、导航「4 项 / 1 active」、180 条站内链接（含干净路径残留与死链）、canonical/hreflang、`$39 / ¥199` 单一标价、Gumroad 占位与零假直链、`_headers` 的 CSP/HSTS、`_redirects` 规则、部署目录无 `.py`/`.pyc`/`__pycache__`。带 URL 时额外探测 8 个 `.html` + 5 条干净路径共 13 条，**跟随重定向**后校验终态 200 / `<title>` / 产品标识；Pages 的 `.html`→干净路径 308 属预期行为，重定向循环会明确报「疑似重定向循环」。线上再检查 CSP 与 HSTS 响应头。
- 本地普通静态服务器不解析 `_redirects`，干净路径 404 记 **WARN** 不阻断；用 `scripts/serve-cloudflare-site.py` 起预览可全通过。
- `tests/site-contract.test.mjs` 已并入 `npm test`，因此 CI 的 `quality` job 会在任何构建/制品之前守住官网结构；同时锁住「部署脚本默认不部署、不得自动执行 login」这条安全边界。

## 仍属外部依赖（代码层无法单独完成）

真实激活端点与生产激活服务、生产 minisign 密钥与 update manifest、Gumroad 商品与结账链接、自定义域 DNS/证书、macOS 双架构产物、隐私政策与服务条款、安装包代码签名与公证。当前状态见 `docs/test-report.md` §40 与上线前 CodeReview 报告。
