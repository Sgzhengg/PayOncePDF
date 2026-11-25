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
| 权威 NS | **已切换为 `elinore.ns.cloudflare.com` / `jaxson.ns.cloudflare.com`**（原 `dns19/dns20.hichina.com` 已替换） | `nslookup -type=NS payoncepdf.com 8.8.8.8` 返回这两个；Cloudflare API `zone.status = active`、`original_name_servers` 为万网旧值 |
| A / CNAME | 根域 CNAME `payoncepdf.com → payoncepdf.pages.dev`（proxied，CNAME flattening）；公网返回 A `104.21.41.85` / `172.67.163.67` | Cloudflare DNS Records API + `nslookup -type=A payoncepdf.com 8.8.8.8` |
| Cloudflare Pages 项目 | **已创建并部署**（2026-11-24），固定地址 **https://payoncepdf.pages.dev/**；自定义域 `payoncepdf.com` **已绑定并可访问** | `wrangler pages project create` + `pages deploy`；Pages Domains API，详见「自定义域绑定记录」 |
| Wrangler 认证 | 本机用户级 `CLOUDFLARE_API_TOKEN` 有效，`whoami` 返回单账号（邮箱与 Account ID 已脱敏）。Token 认证与 OAuth 互斥；**读/写权限均已实证**（建项目、部署、绑域、写 DNS 记录都成功） | `wrangler whoami`、`pages project list`、Cloudflare API |
| 结论 | **https://payoncepdf.com/ 已可访问**：首页 HTTP 200、TLS 验签通过、`server: cloudflare`、13 条路径逐页验收全通过 | 独立网络通道 + 系统代理 curl + `npm run site:verify -- https://payoncepdf.com` |

> **缓存滞后说明**：本机默认 DNS 是链路本地 IPv6（`fe80::…`），以及 Verisign RDAP 快照，在切换后仍会短暂返回旧的 `dns*.hichina.com` NS，导致**不走系统代理的直连**偶尔报 `Could not resolve host`。这不是配置错误：向 8.8.8.8 查询、Cloudflare API、工具侧独立网络、系统代理通道均已确认新 NS 与站点可用。旧缓存按 TTL 自动过期；`Clear-DnsClientCache` 可清本机存根缓存，上游缓存需等 TTL。

**`npx wrangler login` 报错的真实原因**：本机 `HKCU:\Environment` 设置了用户级 `CLOUDFLARE_API_TOKEN`，Wrangler 检测到后强制走 Token 认证，于是拒绝 `X [ERROR] You are logged in with an API Token. Unset the CLOUDFLARE_API_TOKEN in the environment to log in via OAuth.`。**这不是故障，也不是没登录** —— `whoami` 与 `pages project list` 均已成功。详见下方「Wrangler 认证模式」。

注意域名拼写是 **`payoncepdf.com`**（pdf 在前）；`payoncepfd.com` 是早期需求里的笔误，从未注册或使用。

### 让 `payoncepdf.com` 真正可访问的三步（2026-11-24 全部完成）

| 步 | 操作 | 状态 |
| --- | --- | --- |
| 1 | 创建 Pages 项目并部署 `site-cloudflare/` | ✅ 完成，固定地址 https://payoncepdf.pages.dev/ |
| 2 | Cloudflare **Add a site** → 阿里云万网把权威 NS 改成两个 `*.ns.cloudflare.com` | ✅ 完成（用户在阿里云控制台操作，截图确认「DNS服务器 = elinore.ns.cloudflare.com」；8.8.8.8 已返回两个新 NS，zone `active`） |
| 3 | Pages 绑自定义域 `payoncepdf.com` + CNAME + Universal SSL | ✅ 完成，https://payoncepdf.com/ 逐页验收 13/13 通过 |

逐步点击路径保留在下方「万网修改 DNS 服务器」，作为下次换域名或复核时的操作手册；绑定细节见「自定义域绑定记录」。

后续改代码可用 `npm run site:deploy` 重新部署；或改用 Git 集成（仓库 `Sgzhengg/PayOncePDF` / 分支 `main`，Framework preset **None**、Build command **留空**、Build output directory **`site-cloudflare`**），但须按「上线前置」处理 webhook 绕过 CI 的问题。**Add a site 与改阿里云 NS 属于域名控制权操作，由用户本人完成**；第 3 步的绑定与 DNS 记录是在用户已授权的上线流程内、用其本机 `CLOUDFLARE_API_TOKEN` 经 Cloudflare API 完成的一次性操作，**并未并入部署脚本**（`scripts/deploy-cloudflare-site.mjs` 依旧不绑域、不改 DNS）。

### 万网修改 DNS 服务器（逐步可照做，2026-11-24 核对）

> 这一步改的是**注册局层面的权威 NS（谁负责解析）**，不是「解析设置」里加 A / CNAME / NS 记录。在阿里云云解析控制台加记录无效，必须去**域名控制台**改 DNS 服务器。

**A. 先在 Cloudflare 拿到两个 NS**

1. Dashboard → **Add a site**（添加站点）→ 输入根域 `payoncepdf.com`（**不带 www、不带 https**）。
2. 选 **Free** 计划 → 保持默认「快速扫描 DNS 记录」→ Continue。
3. 该域名目前没有 A / CNAME（权威 NS 只返回 SOA 否定应答），扫描为空时会弹 `Add records later — Without DNS records, Cloudflare is unable to activate your site`，**直接 Confirm**；Pages 后续会自动补记录。
4. 点 **Continue to activation** → 进入 **Update your nameservers** 页：
   - *Add each of your assigned Cloudflare nameservers*：两个 `xxx.ns.cloudflare.com`。**每个账号分配的名字不同，必须用自己页面显示的**，别抄教程示例。
   - *Delete your other nameservers*：列出的 `dns19.hichina.com` / `dns20.hichina.com`（本机实测）正是要替换掉的万网默认 NS。
5. 复制这两个 NS。此时站点状态为 **Pending nameserver update**，属正常。

**B. 再到阿里云万网改 DNS 服务器**

1. 登录阿里云控制台 → **域名与网站 → 域名**（域名列表）。
2. 找到 `payoncepdf.com` → 点操作列的 **管理**。
3. 左侧菜单 **DNS 管理 → DNS 修改**（新版也叫「DNS 修改 / 修改 DNS 服务器」）；页面显示当前 DNS 服务器为 `dns19/dns20.hichina.com`。
4. 点 **修改 DNS 服务器** → 选 **修改为自定义 DNS**。
5. 两个「DNS 服务器」输入框分别粘贴 A 步骤的两个 `*.ns.cloudflare.com`（本项目只要 2 个，需要更多行点 **+ 添加 DNS**）。**结尾不要带点、不要有空格**。
6. 点 **确定** → 弹出「请确认该操作」→ 选 **手机认证 / 获取验证码**（短信或邮箱）→ 填验证码 → 再点 **确定**。阿里云会要求一次安全验证，这是「没保存成功」最常见的原因。
7. 回到 DNS 修改页，**「当前 DNS 服务器」变成两个 `*.ns.cloudflare.com` 即提交成功**。

**C. 等待与验证**

- 生效时间：常见 10 分钟～数小时；阿里云官方口径受旧 NS 的 TTL 影响、最长 48 小时。Cloudflare 检测到后会发邮件，状态从 *Pending nameserver update* 变 **Active**。
- 本机验证：`nslookup -type=NS payoncepdf.com` 应只返回那两个 `*.ns.cloudflare.com`；`nslookup payoncepdf.com` 出现 A 记录说明已开始解析。也可 `curl -I https://payoncepdf.com/` 看响应是否带 `server: cloudflare` / `cf-ray`（绑定自定义域之后）。
- 长时间不生效：检查是否填了尾点或空格、是否只填了一个 NS、是否误在「解析设置」页操作、注册局安全锁是否拦截。

**常见坑**

| 坑 | 说明 |
| --- | --- |
| 跑去「解析设置」加记录 | 无效。注册商决定「谁来解析」，解析服务商决定「解析成什么」；换 NS 只能在域名控制台改 |
| 抄教程里的 NS 名字 | Cloudflare 每个账号分配不同的 `xxx.ns.cloudflare.com`，必须用自己页面显示的 |
| 保留旧的万网 NS | 只保留 Cloudflare 的 2 个，旧的 `dns*.hichina.com` 全部替换掉 |
| DNSSEC 已开启 | 先在阿里云「DNS 管理 → DNSSEC 设置」关闭（新买域名默认未开），否则切换后可能解析失败；需要时再在 Cloudflare 侧重开 |
| 原有 MX / TXT 记录 | 改 NS 后阿里云云解析里的记录不再生效，须先在 Cloudflare **DNS → Records** 补上（邮箱类保持「仅 DNS / 灰云」）。本项目域名当前无任何网站记录，影响很小；若绑过企业邮箱务必检查 |

**C 验证已通过**（`nslookup -type=NS payoncepdf.com 8.8.8.8` 返回 `elinore.ns.cloudflare.com` / `jaxson.ns.cloudflare.com`，Cloudflare zone `active`），随后完成绑定，详见下一节。

## 自定义域绑定记录（2026-11-24）

用户发来阿里云「基本信息」页截图：注册信息里 **DNS服务器 = `elinore.ns.cloudflare.com`**，说明 NS 修改已提交。交叉验证：

| 检查 | 结果 |
| --- | --- |
| `nslookup -type=NS payoncepdf.com 8.8.8.8` | `elinore.ns.cloudflare.com` + `jaxson.ns.cloudflare.com`（新 NS 已全局生效） |
| Cloudflare API `zones?name=payoncepdf.com` | `status = active`、`type = full`、Free 计划、`original_name_servers = dns19/dns20.hichina.com` |
| Verisign RDAP / 本机 `fe80::` DNS | 仍短暂返回旧万网 NS —— **缓存滞后，不是配置失败**（RDAP `last update` 有延迟、本机上游 DNS 有 TTL） |
| Pages Domains（绑定前） | 只有 `payoncepdf.pages.dev`，故域名无任何网站记录，浏览器必然打不开 |

绑定过程（用本机已验证的 `CLOUDFLARE_API_TOKEN`，**不改动阿里云 NS、不触碰其他 Pages 项目**）：

1. `POST /accounts/{id}/pages/projects/payoncepdf/domains`，body `{"name":"payoncepdf.com"}` → 返回 `status=initializing`、`certificate_authority=google`、`validation.method=http`，domain id `c1fc6068-…`。
2. 约 30 秒后转 `pending`，但 `verification_data.error_message = "CNAME record not set"`，DNS Records 为空 —— **API 绑域不会自动补 CNAME**（官方 Dashboard 流程在 zone 已接入时会自动创建，API 路径未触发）。
3. 按官方文档补建根域 CNAME：`payoncepdf.com → payoncepdf.pages.dev`，`proxied=true`、`ttl=1`（record id `9520210393…`）。随即 8.8.8.8 返回 CNAME flattening 后的 A `104.21.41.85` / `172.67.163.67`，`verification_data.status` 转 `active`。

验证结果：`curl --resolve payoncepdf.com:443:104.21.41.85` **HTTP 200、`ssl_verify_result=0`**、`server: cloudflare`、CSP/HSTS 命中；独立网络通道取到完整中文首页；经系统代理 13 条路径全部 200（`.html` 308 到干净路径）；`npm run site:verify -- https://payoncepdf.com` **本地门禁 10/10 + 13 条 HTTP 路径 + 安全头，0 阻断**。

> 状态口径：绑定后先 `initializing` → `pending`（约 30 秒，等待补 CNAME）→ 补完 CNAME 后 `verification_data` 转 `active` → 最终 `status = active` 且 `validation_data.status = active`（HTTP 证书打包完成，约数分钟）。**这段 pending 不影响访问**：站点在 verification 转 active 后就已经能正常返回 200。
>
> **www 补充（同日稍后）**：又添加了 `www.payoncepdf.com` 自定义域 + proxied CNAME（`www → payoncepdf.pages.dev`），verification 已 active。但实测 `https://www.payoncepdf.com/` 返回 **200 而非 301** —— Pages `_redirects` 不支持域名级重定向（官方 "Domain-level redirects ❌"），那条规则是死规则，已删除。现在 www 作为镜像提供内容，靠 8 页 canonical 归一；严格 301 属于待授权项（清单第 11 项）。

## Wrangler 认证模式（API Token 与 OAuth 互斥）

| 模式 | 判定 | 是否要 `wrangler login` |
| --- | --- | --- |
| **A. API Token**（本机现状） | `CLOUDFLARE_API_TOKEN` 已设置（用户级 `HKCU:\Environment`，len=53） | ❌ **不要执行**，执行必报 `You are logged in with an API Token` |
| B. OAuth | 未设置该变量，`~/.wrangler/config/default.toml` 等凭据文件存在 | ✅ 需要，浏览器授权须本人操作 |
| C. 无凭据 | 两者皆无 | ✅ 需要 |

- `wrangler whoami` 是唯一精确判定方式；只读检测脚本 `scripts/deploy-cloudflare-site.mjs` 的 `detectAuth()` 会报告当前模式，**绝不打印 Token 内容**。
- `pages deploy` 没有 `--account-id` 参数；账号 ID 通过 `CLOUDFLARE_ACCOUNT_ID` 环境变量提供，脚本支持 `--account-id <ID>`（仅注入 wrangler 子进程，不改用户环境变量）。本机未设置该变量时由 Wrangler 从单账号凭据自动推断。
- Token 权限在 Dashboard → Profile → API Tokens 查看。`whoami` 曾提示缺少 `User->User Details->Read`（读不到邮箱），但 `pages project list` 成功；后续 `pages project create`、`pages deploy`、Pages Domains 写入、DNS Records 写入均成功，**`Cloudflare Pages:Edit` 与 `DNS:Edit` 写权限已实证具备**。
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
| 自定义域 | 首次部署时**未绑定**；同日稍后已完成绑定并逐页验收，见「自定义域绑定记录」。最终对外地址 **https://payoncepdf.com/** |

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
- `scripts/verify-cloudflare-site.mjs`：本地门禁覆盖 **19 个必需文件 / 12 页**标签配平、导航「4 项 / 1 active」、296 条站内链接（含干净路径残留与死链）、canonical/hreflang、`$39 / ¥199` 单一标价、Gumroad 占位与零假直链、`_headers` 的 CSP/HSTS、`_redirects` 规则、部署目录无 `.py`/`.pyc`/`__pycache__`，以及「法务页口径 / 安装包校验值与 README 一致 / 校验脚本离线 / 试用入口有就地兜底」。带 URL 时额外探测 12 个 `.html` + 9 条干净路径共 **21 条**，**跟随重定向**后校验终态 200 / `<title>` / 产品标识；Pages 的 `.html`→干净路径 308 属预期行为，重定向循环会明确报「疑似重定向循环」。线上再检查 CSP 与 HSTS 响应头。
- 本地普通静态服务器不解析 `_redirects`，干净路径 404 记 **WARN** 不阻断；用 `scripts/serve-cloudflare-site.py` 起预览可全通过。
- **本机网络注意（2026-11-24 实测）**：本机设了系统代理（`HTTPS_PROXY`），Node 22 的 `fetch` 默认直连，直接跑 `npm run site:verify -- https://payoncepdf.com` 会把 21 条路径全部报 `fetch failed`，这是本机链路问题、**不是站点故障**（同时用 `curl` 走代理全部 200）。正确命令是 `NODE_USE_ENV_PROXY=1 npm run site:verify -- https://payoncepdf.com`（Node 22.21+），验收脚本在 `fetch failed` 时会给出这条提示。`curl` 本身会读代理环境变量，无需额外设置。
- `tests/site-contract.test.mjs` 已并入 `npm test`，因此 CI 的 `quality` job 会在任何构建/制品之前守住官网结构；同时锁住「部署脚本默认不部署、不得自动执行 login」这条安全边界。

## 已收口（本轮不再列为待办）

- 官网部署、域名 NS 接入、`payoncepdf.com` 自定义域绑定、Universal SSL、CSP/HSTS、干净路径与旧链接 301 —— 全部完成并线上验收通过。
- 隐私口径的事实披露（设备指纹 SHA-256 哈希、邮箱用途边界、三类网络请求）已在中英 12 页 footer、帮助页 FAQ/JSON-LD 与独立隐私政策页补齐。
- Tauri `opener` 权限已从「任意 URL」收紧为显式 allow-list。
- **发现并清掉一条死规则**：Pages 官方文档明确标注 `_redirects` 不支持域名级重定向，原 `https://www.payoncepdf.com/* → https://payoncepdf.com/:splat 301` 从未生效（线上实测 www 返回 200）。已删除该规则，并加门禁禁止再出现域名级 source。`www.payoncepdf.com` 已绑定可访问，12 页 canonical 全部指向主域，由 canonical 归一；按 Grabber 先例，严格 301 已降级为非阻断，见下方决策清单第 11 项。

## 参考 Grabber / zhuazhua.video 的先例（2026-11-24）

用户授权「关于你的建议，可以参考 Grabber 及官网的做法」后，对 `E:/grabber` 站点源码、
线上 `https://zhuazhua.video/privacy` 与 `/terms`、以及桌面 `Grabber+Recast/` 的上线方案做了只读调研。
结论：上一版清单里有 3 项被我判成「只有用户能给事实、代码层无法代做」，Grabber 已经给出可直接照做的解法。

| 先例 | Grabber 的实际做法 | 对 PayOncePDF 的影响 |
| --- | --- | --- |
| 运营主体 | 官网法务页只写品牌名 + 唯一邮箱 `hi@zhuazhua.video` + Gumroad 为 merchant of record；无公司全称 / 注册地 / 备案号（`docs/compliance-checklist.md` 把执照列为 P0 待办）；管辖条款写在 EULA「适用中华人民共和国法律、开发者所在地法院」 | 第 7 项不再是硬阻塞，法务页按同口径已上线 |
| 邮箱 | Cloudflare **Email Routing** 免费转发到常用邮箱，不是自建邮局 | 第 8 项给出零成本具体做法 |
| www | apex + www 一起绑到 Pages，`_redirects` 无任何域名级规则，仓库里也没有 Redirect Rules 配置；靠平台与 canonical 归一 | 第 11 项降级为非阻断，现状态与先例一致 |
| 下载校验 | `download.html` 列版本号 / 体积 / 4 个 SHA-256，用浏览器 WebCrypto 本机比对、文件不上传，`content/hashes.json` 为单一事实源 | 第 4 项的代码部分已复刻（直链仍不伪造） |
| 购买开关 | `content/stage.json` + `/buy/<plan>` 302 中间跳转 + Gumroad URL 走环境变量，内测态禁止出现购买直链 | 第 1/2 项写成 `docs/activation-service-plan.md` 设计稿 |
| 真机测试 | 「现在必跑 / 只记录不判失败 / 开卖后补测」三分法 + 假失败清单 + 离线授权 5 步验证 | `docs/real-device-test-plan.md` §10 |

**本次按先例实际落地**（均为纯代码 / 文档，未伪造任何外部账号状态）：

- 中英 4 个独立法务页 `/privacy`、`/terms` 及其 `/en/` 镜像上线，12 页 footer 全站挂链接；口径只写品牌名、`hello@payoncepdf.com` 与 Gumroad 收款主体，并明确标注运营主体全称 / 注册地 / 数据留存期限尚未确定，**不编造**。
- 下载页中英版新增「安装包校验值」区块：展示当前内部验证包真实体积 `31,719,938 B` 与 SHA-256 `8307c7b5…6d2d92`；`/verify-sha256.js` 用 WebCrypto 在本机计算，文件不上传、脚本无任何网络请求，兼容站点 CSP 的 `script-src 'self'`；页面明确写「未代码签名、未对外分发」。
- 门禁从 13 文件 / 8 页扩到 **19 文件 / 12 页 / 296 条站内链接 / 21 条 HTTP 路径**；新增契约锁住「法务页不编造主体」「校验值与 README 一致」「校验脚本离线」「试用入口不再裸 mailto」。
- 新增 `docs/activation-service-plan.md`（设计稿，未实现）；真机计划补 §10 三分法。

## 2026-11-25 修复：试用按钮点了没反应

线上反馈「在首页点『立即开始 7 天试用』后无法进入下载页面」。用无头 Edge + CDP 复现，结论分两段：

1. **首页那段是好的**：hero 区 `class="btn primary" href="/download.html"` 真实鼠标点击后落到 `https://payoncepdf.com/download`（308 → 200），`<title>` 正常，无元素遮挡、无控制台报错。
2. **断在下载页**：页面上那个大字「立即开始 7 天试用 →」的 `href` 是 `mailto:hello@payoncepdf.com?subject=…`。系统没配默认邮件程序时，点 mailto **不会有任何反应**（无跳转、无提示、无报错），在用户看来就是"按钮坏了、进不了下载"。

修复（不伪造下载直链，下载通道仍在准备中）：

- 中英下载页的**两个**试用按钮都从 `<a href="mailto:">` 改成 `<button data-getbuild-open>`，点击就地展开登记面板，同时把整封邮件（收件人 / 主题 / 模板正文）写进剪贴板。
- 面板给三条都能走通的路：复制邮箱、复制整封邮件、打开本机邮件应用（标注"仅系统已设默认邮件程序时有效"）；复制走 `navigator.clipboard`，失败回退 `document.execCommand('copy')`，全部失败时给手动复制指引，绝不静默。
- 反馈写进 `aria-live="polite"` 区域，并维护 `aria-expanded`；英文页按 `<html lang>` 切英文提示，不出现中文。
- 新脚本 `site-cloudflare/get-build.js` 零网络请求（站点 CSP 仍是 `script-src 'self'`）；门禁新增第 12 项「试用入口有就地兜底，不再是裸 mailto」，契约测试锁住「首页 CTA 直连下载页」「下载页不得再出现整块 mailto 主按钮」。

验证：`npm test` **77/0**；`site:check` **12/12 PASS**（19 文件 / 12 页 / 296 链接）；CDP 实测中英两页按钮点击后面板展开、`aria-expanded=true`、底部 CTA 也能展开并滚入视口；授予剪贴板权限后剪贴板确为完整邮件正文（含真实换行）。

## 下载托管（2026-11-25 建立）

| 项 | 值 |
| --- | --- |
| 桶 | `payoncepdf-dl`（R2，与 `grabber-dl` / `recast-dl` 同账号） |
| 公网域名 | `dl.payoncepdf.com`（R2 自定义域，CNAME → `public.r2.dev`，proxied） |
| 当前版本目录 | `v1.0.1/`（2026-11-25 精简包；`v1.0.0/` 保留但已作废） |
| 安装包 | `https://dl.payoncepdf.com/v1.0.1/PayOnce-PDF-Setup-1.0.1.exe`（19,375,119 B / 18.48 MiB，SHA-256 `39121646…e67147`） |
| 校验清单 | `https://dl.payoncepdf.com/v1.0.1/SHA256SUMS.txt` |
| 更新签名 | `https://dl.payoncepdf.com/v1.0.1/PayOnce-PDF-Setup-1.0.1.exe.sig`（开发密钥，`version:1.0.1`；口令见 `docs/RENAME-MIGRATION.md`） |
| 备用 r2.dev | `https://pub-6c35caeaa9b549919cec09566806a467.r2.dev`（已启用，仅应急；r2.dev 有限流，不作正式入口） |

发版流程（每次都要做，否则页面哈希与真实文件不一致）：

1. 打包后核对本地包 SHA-256 与体积，更新 `README.md` 的发行包段落；
2. `npx wrangler r2 object put payoncepdf-dl/v<新版本>/<包名> --file <本地包> --content-type application/octet-stream --content-disposition "attachment; filename=<包名>" --remote`（**必须带 `--remote`**，否则只写进本地 miniflare）；
3. 同步上传 `SHA256SUMS.txt` 与 `.sig`；
4. 改 `site-cloudflare/download.html` 与 `en/download.html` 的直链、版本号、体积、哈希（门禁会校验三者一致）；
5. `npm test && npm run site:deploy`，再跑 `NODE_USE_ENV_PROXY=1 npm run site:verify -- https://payoncepdf.com`。

已知边界：自定义域首次绑定时 `ownership_status`/`ssl_status` 会是 `pending`，此时访问返回 `error code: 1014`，通常 1–3 分钟内自动转 `active`；R2 对象默认按文件名给 `Content-Disposition: attachment`，不要去掉，否则 `.exe` 可能被浏览器当内联内容处理。

### 1.0.1 为什么变小了（2026-11-25）

用户反馈「安装包下载很慢」后实测：本机到 Cloudflare 落地 `SJC`，单流 117–240 KB/s、6 条并行合计 1.22 MB/s，而国内镜像 6–18 MB/s —— 慢在**国际链路单流受限**，不是文件或桶的问题。同期发现包里有 43.6 MB 死重量：

- `scripts/provision-sidecars.mjs` 的 `copySiblingDlls()` 原先把 `qpdf.exe` **同目录所有 DLL** 拷入 resources，而 MSYS2 发行目录里带着整条 GTK/ICU 栈：`libicudt75.dll` 29.3 MB、`libicuin/icuuc` 4.1 MB、pango/cairo/glib/harfbuzz/freetype/fontconfig 等约 10 MB。主程序只 import `pdfium` + `qpdf`，这些库只互相引用。
- 改为沿导入图做依赖闭包（`referencedDlls()` + BFS），只拷真正被引用的 DLL：resources 63 文件 / 78 MB → 41 文件 / 46 MB，安装包 **31.7 MB → 19.4 MB**。
- 踩坑记录：`libstdc++-6.dll` 被 qpdf 真实 import，但文件名含 `+`，早期用 `[A-Za-z0-9_.\-]` 扫描会漏掉，删掉后自检报 `engine.qpdf_slice E2004`；恢复后 32/32。
- 回归护栏：`tests/bundle-deps.test.mjs` 锁定「22 个死 DLL 不得回归 / DLL 合计 < 46 MB / 单文件不得 > 8 MB / qpdf 运行时依赖必须在 / provision 不得退回全量拷贝」。

## 剩余上线决策清单（按 Grabber 先例重新评估）

> 仍区分「只有你能给的事实 / 操作」与「信息齐备后我能接着做的代码」。凡是涉及真实商业主体、账号、密钥、证书的信息，不会编造或用占位符冒充上线。

| # | 项 | 现状（2026-11-24） | 参考 Grabber 后的结论 | 需要你提供 / 操作 |
| --- | --- | --- | --- | --- |
| 1 | Gumroad 商品与结账链接 | 中英价格页按钮为占位（`aria-disabled`），客户端 `PAYMENT_URL` 指向价格页 | Grabber 用 `/buy/<plan>` 中间跳转 + `content/stage.json` 单一开关，开卖只需配 `GUMROAD_*_URL`；内测态禁止出现购买直链。可照搬，见 `docs/activation-service-plan.md` §5 | 创建商品、给出结账直链与 product_id、确认退款期限 |
| 2 | 生产激活服务 `/api/activate` | 仅本地 mock；客户端默认地址为占位域名 | Grabber 的 Pages Functions + D1 已跑通：license key 校验（必须查 refunded / chargebacked / disputed）、Ping 回查销售真实性、Ed25519 签发、名额原子控制、CORS 预检。骨架方案与 11 条验收已写成设计稿 | 决定服务形态、创建 D1、保管生产 Ed25519 私钥、提供 `GUMROAD_ACCESS_TOKEN` |
| 3 | 更新包签名与 update manifest | 开发 minisign 密钥，端点占位 | Grabber 同样用 minisign + `latest.json` + `SHA2-256SUMS`，生产密钥仍是其待办 | 生成并保管生产 minisign 密钥对、提供 update endpoint |
| 4 | 真实下载直链 | **已完成（2026-11-25）**：R2 桶 `payoncepdf-dl` + 自定义域 `dl.payoncepdf.com`，直链指向 `v1.0.1/PayOnce-PDF-Setup-1.0.1.exe`（18.48 MiB，附 `SHA256SUMS.txt` 与 `.sig`），中英下载页主按钮已接真实直链 | 无（后续发版只需上传新版本目录并改页面链接） |
| 5 | 代码签名 / 公证 | 报告标注「办理中」；**当前对外分发的包仍未签名**，官网已如实披露 SmartScreen 提示 | Grabber 也把 EV 证书列为 P0 待办，官网如实写「macOS 尚未完成 Apple 公证」；不伪装已签名 | EV/Authenticode 证书、Apple Developer ID 与公证账号 |
| 6 | macOS 双架构产物 | 官网承诺双平台，实际无 macOS 包 | Grabber 有 arm64 / x64 双包但公证未完成，同样如实披露 | macOS 构建机或 CI runner、Apple 账号 |
| 7 | 独立隐私政策 / 服务条款页 | **已完成**：中英 4 页上线，12 页 footer 全站链接 | Grabber 先例证明「运营主体全称」不是法务页硬阻塞；已按品牌名 + 邮箱 + Gumroad MoR 同口径上线，管辖条款沿用其 EULA 口径 | 仅当日后确定运营主体全称、注册地、数据留存期限时告诉我补进两页（不编造） |
| 8 | `hello@payoncepdf.com` 邮箱 | 域名 MX 未配置 | **Grabber 用 Cloudflare Email Routing**：Dashboard → Email → Email Routing → Enable → 填目标邮箱并完成验证 → 按面板给出的 2 条 MX + 1 条 TXT/SPF 添加记录，DKIM/DMARC 由面板生成。不需要自建邮局，也不需要买企业邮箱 | 本人启用转发并确认目标邮箱；我可验证 DNS 与收发 |
| 9 | 真机双平台端到端 | 文档标注「真机未验」 | 已按 Grabber 三分法重构 `docs/real-device-test-plan.md` §10：A 现在必跑 / B 只记录不判失败 / C 开卖后补测，含假失败清单与离线授权 5 步 | Windows / macOS 实机、抓包工具 |
| 10 | Git 集成自动部署门禁 | 当前走 `npm run site:deploy`（CLI），**未启用** Git webhook | 不变 | 若改用 Git 集成，需在 Cloudflare 侧关闭「推 main 自动部署」或改为门禁后触发 |
| 11 | `www` → 主域严格 301 | www 已绑定可访问但返回 **200**；靠 12 页 canonical 归一 | **按 Grabber 先例降级为非阻断**：zhuazhua.video 同样只把 apex + www 绑到 Pages，`_redirects` 与仓库里都没有域名级 / Redirect Rules 配置 | 仅当 SEO 明确需要严格 301 时才做；仍不擅自给 API Token 增加 Rulesets 权限（该 API 曾返回 403） |

**优先级建议（按先例修订）**：① 第 8 项 Email Routing（零成本，让所有 `mailto:` 变成有效入口）→ ② 第 1/2 项一起做（Gumroad 与激活服务必须共用订单校验，方案见 `docs/activation-service-plan.md`）→ ③ 第 5/6 项决定安装包能否对外分发 → ④ 第 4 项在签名完成后才有意义 → ⑤ 第 3、9、10、11 项按需，其中 11 已降级。

**仍然 NO-GO（未伪装完成）**：Gumroad 商品与结账、生产激活端点、生产 minisign、代码签名与公证、macOS 产物、真机双平台端到端，以及法务页里运营主体全称 / 数据留存期限的补充。

**分发状态（2026-11-25 变更）**：用户明确选择「放真实安装包直链」，Windows 包已对外提供下载。**它仍无 Authenticode 签名**，Windows 会提示未知发布者，官网下载页/帮助页已如实披露，并给出 SHA-256 校验与邮件兜底。不要把「已可下载」说成「已签名」或「已公证」。

当前状态另见 `docs/test-report.md` §40 与上线前 CodeReview 报告。
