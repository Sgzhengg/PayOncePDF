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
| Cloudflare Pages 项目 | **尚未创建 / 未部署**，没有 `*.pages.dev` 地址 | 无部署记录 |
| 结论 | DNS 无记录 → 浏览器必然打不开；工具侧独立网络访问 `https://payoncepdf.com` 同样失败 | 已排除「只有本机代理异常」的可能 |

注意域名拼写是 **`payoncepdf.com`**（pdf 在前）；`payoncepfd.com` 是早期需求里的笔误，从未注册或使用。

### 让 `payoncepdf.com` 真正可访问的三步（需用户账号授权，仓库内无法代做）

1. **接入 Cloudflare**：Dashboard → *Add a site* → `payoncepdf.com`，拿到分配的两个 nameserver；到阿里云万网把该域名的 DNS 服务器改成这两个 NS，等待生效（10 分钟～数小时）。
2. **建 Pages 项目**（二选一）：
   - CLI：`npx wrangler login` → `cd site-cloudflare` → `npx wrangler pages deploy . --project-name payoncepdf --branch main`；
   - Git 集成：仓库 `Sgzhengg/PayOncePDF` / 分支 `main`，Framework preset **None**、Build command **留空**、Build output directory **`site-cloudflare`**。
   首次部署会得到 `*.pages.dev` 地址，**可先用它验证中英 8 页**，不必等自定义域生效。
3. **绑自定义域**：Pages → *Settings* → *Domains* → Add `payoncepdf.com`（可另加 `www.payoncepdf.com`，`_redirects` 已做 www→主域 301），等 Universal SSL 状态变 **Active**；根域用 CNAME flattening，**不要在阿里云手工填猜测的 A 记录**。

Cloudflare 账号授权与阿里云域名 DNS 控制权都属于用户本人；当前环境也没有可直接操作 Cloudflare 的连接器，因此这一步只能由用户在控制台完成。

## 上线前置（质量门禁）

1. GitHub Actions `quality` job（`npm run typecheck` / `npm test` / `npm run build` / `python scripts/verify-public-key.py`）必须绿；
2. Windows `build` job 在供给 sidecar 后、`tauri build` 前会跑 `cargo test --lib`，打包后跑「发行包公钥收录复核」，两者都必须绿；
3. **Cloudflare 的 Git 集成是独立 webhook，不受上述 CI 约束**。上线前需在 Cloudflare 侧关闭「推 main 即自动部署」，或把部署动作改为由通过门禁的 workflow 触发 / 人工确认，否则任何回归仍可直达生产。

## 仍属外部依赖（代码层无法单独完成）

真实激活端点与生产激活服务、生产 minisign 密钥与 update manifest、Gumroad 商品与结账链接、自定义域 DNS/证书、macOS 双架构产物、隐私政策与服务条款、安装包代码签名与公证。当前状态见 `docs/test-report.md` §40 与上线前 CodeReview 报告。
