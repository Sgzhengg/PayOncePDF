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

## 上线前置（质量门禁）

1. GitHub Actions `quality` job（`npm run typecheck` / `npm test` / `npm run build` / `python scripts/verify-public-key.py`）必须绿；
2. Windows `build` job 在供给 sidecar 后、`tauri build` 前会跑 `cargo test --lib`，打包后跑「发行包公钥收录复核」，两者都必须绿；
3. **Cloudflare 的 Git 集成是独立 webhook，不受上述 CI 约束**。上线前需在 Cloudflare 侧关闭「推 main 即自动部署」，或把部署动作改为由通过门禁的 workflow 触发 / 人工确认，否则任何回归仍可直达生产。

## 仍属外部依赖（代码层无法单独完成）

真实激活端点与生产激活服务、生产 minisign 密钥与 update manifest、Gumroad 商品与结账链接、自定义域 DNS/证书、macOS 双架构产物、隐私政策与服务条款、安装包代码签名与公证。当前状态见 `docs/test-report.md` §40 与上线前 CodeReview 报告。
