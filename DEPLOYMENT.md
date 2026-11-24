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
- `scripts/verify-cloudflare-site.mjs`：本地门禁覆盖 13 个必需文件、8 页标签配平、导航「4 项 / 1 active」、180 条站内链接（含干净路径残留与死链）、canonical/hreflang、`$39 / ¥199` 单一标价、Gumroad 占位与零假直链、`_headers` 的 CSP/HSTS、`_redirects` 规则、部署目录无 `.py`/`.pyc`/`__pycache__`。带 URL 时额外探测 8 个 `.html` + 5 条干净路径共 13 条，**跟随重定向**后校验终态 200 / `<title>` / 产品标识；Pages 的 `.html`→干净路径 308 属预期行为，重定向循环会明确报「疑似重定向循环」。线上再检查 CSP 与 HSTS 响应头。
- 本地普通静态服务器不解析 `_redirects`，干净路径 404 记 **WARN** 不阻断；用 `scripts/serve-cloudflare-site.py` 起预览可全通过。
- `tests/site-contract.test.mjs` 已并入 `npm test`，因此 CI 的 `quality` job 会在任何构建/制品之前守住官网结构；同时锁住「部署脚本默认不部署、不得自动执行 login」这条安全边界。

## 已收口（本轮不再列为待办）

- 官网部署、域名 NS 接入、`payoncepdf.com` 自定义域绑定、Universal SSL、CSP/HSTS、干净路径与旧链接 301 —— 全部完成并线上验收通过。
- 隐私口径的事实披露（设备指纹 SHA-256 哈希、邮箱用途边界、三类网络请求）已在中英 8 页 footer 与帮助页 FAQ/JSON-LD 补齐。
- Tauri `opener` 权限已从「任意 URL」收紧为显式 allow-list。
- **发现并清掉一条死规则**：Pages 官方文档明确标注 `_redirects` 不支持域名级重定向，原 `https://www.payoncepdf.com/* → https://payoncepdf.com/:splat 301` 从未生效（线上实测 www 返回 200）。已删除该规则，并加门禁禁止再出现域名级 source。`www.payoncepdf.com` 已绑定可访问，8 页 canonical 全部指向主域，由 canonical 归一；严格 301 需 Cloudflare Redirect Rules，见下方决策清单第 11 项。

## 剩余上线决策清单（必须由用户本人提供信息或操作，代码层无法代做）

> 下表刻意区分「需要你给的**事实**」与「我能接着做的**代码工作**」。凡是涉及真实商业主体、账号、密钥、证书的信息，我不会编造或用占位符冒充上线。

| # | 项 | 现状 | 需要你提供 / 操作 | 信息齐备后我可接着做 |
| --- | --- | --- | --- | --- |
| 1 | Gumroad 商品与结账链接 | 中英价格页按钮为占位（`aria-disabled`），客户端 `PAYMENT_URL` 指向价格页 | 创建商品、给出结账直链与商品价格/币种策略、退款期限与通道 | 替换 8 页按钮与 `PAYMENT_URL`，移除占位标记，补契约测试 |
| 2 | 生产激活服务 `/api/activate` | 仅本地 mock；客户端默认地址为占位域名 | 决定服务形态（建议 Cloudflare Worker + D1/KV 持久化设备台账）、提供 Gumroad license key 校验方式、保管生产 Ed25519 私钥 | 实现服务骨架、订单校验、每把 key 最多 3 台的持久化台账、换机/解绑流程，并把端点固化进发行包 |
| 3 | 更新包签名与 update manifest | 开发 minisign 密钥，端点占位 | 生成并保管生产 minisign 密钥对、提供 update endpoint | 轮换公钥（需重新编译）、接入 manifest、CI 注入生产密钥 |
| 4 | 真实下载直链 | 官网用 `mailto:` 登记发放 | 选择托管（GitHub Releases / R2）并确认**先完成代码签名** | 替换下载页 CTA、写入真实 SHA-256 与校验说明 |
| 5 | 代码签名 / 公证 | 报告标注「办理中」 | EV/Authenticode 证书、Apple Developer ID 与公证账号 | 接入签名脚本与 CI、验证签名链 |
| 6 | macOS 双架构产物 | 官网承诺双平台，实际无 macOS 包 | macOS 构建机或 CI runner、Apple 账号 | 产出 arm64/x64/universal dmg，或先修正官网平台口径 |
| 7 | 独立隐私政策 / 服务条款页 | 只有 footer 事实说明，无独立页 | **运营主体全称与注册地、联系方式、法律管辖、退款政策、数据留存期限** | 生成中英独立页并接入导航/footer，同步契约测试 |
| 8 | `hello@payoncepdf.com` 邮箱 | 域名 MX 未配置 | 开通邮箱服务并添加 MX/TXT（改 NS 后必须在 Cloudflare DNS 侧加） | 验证收发，替换所有 `mailto:` 占位 |
| 9 | 真机双平台端到端 | 文档标注「真机未验」 | Windows / macOS 实机、抓包工具 | 更新 `docs/real-device-test-plan.md` 结果，跑安装态 OCR、试用到期弹窗、三类请求抓包 |
| 10 | Git 集成自动部署门禁 | 当前走 `npm run site:deploy`（CLI），**未启用** Git webhook | 若改用 Git 集成，需在 Cloudflare 侧关闭「推 main 自动部署」或改为门禁后触发 | 调整 workflow 触发条件并复验 |
| 11 | `www` → 主域严格 301 | www 已绑定可访问但返回 **200**；靠 8 页 canonical 归一，不是 301 | Cloudflare Dashboard → Rules → Redirect Rules 建一条 `http.host eq "www.payoncepdf.com"` → 301；**或**给现有 API Token 增加 `Zone Rulesets:Edit`（本次调用该 API 返回 403，我不会擅自改你的 Token 权限） | 我复验 301 与 canonical 一致，并更新门禁断言 |

**优先级建议**：① 先定 7 的运营主体信息（这是唯一会阻塞法务页、且只有你知道的事实）→ ② 配置 8 的邮箱（否则所有 `mailto:` 都是无效入口）→ ③ 1/2 的 Gumroad 与激活服务必须一起设计（激活服务要校验订单）→ ④ 5/6 决定安装包能否对外分发 → ⑤ 4 在签名完成后才有意义。

当前状态另见 `docs/test-report.md` §40 与上线前 CodeReview 报告。
