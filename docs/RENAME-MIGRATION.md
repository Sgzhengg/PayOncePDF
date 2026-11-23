# 品牌改名迁移说明（OncePDF → PayOnce PDF）

> **现行价格（2026-11-24 更新）**：**$39 / ¥199 单一标价，不设早鸟 / 划线折扣**（用户已取消「首发 14 天早鸟 $29 / ¥149」，`site-cloudflare/` 官网 8 页按此执行）。本文写于 2026-11-16 定价决策当时，文中的 `$29 / ¥169`（早鸟 `$19 / ¥109`）是**历史价格**，论证与数字按原文保留；一把钥匙最多 3 台设备。依据 `docs/updf-parity-decision.md` §1、`config/product.json` 与 PRD §22.10。

> **本文件是改名任务的唯一历史记录**。按验收标准「允许出现在 CHANGELOG 或迁移说明中」，下面的旧品牌名、旧域名、旧前缀、旧路径都是**有意保留**的历史值，不代表当前生效配置。除此之外，源码 / 配置 / 文档中不应再出现旧名。

- 执行日期：2026-11-16
- 新名称：**PayOnce PDF**（中文：**一次付费 PDF**）
- 官网：`https://payoncepdf.com`
- 联系邮箱：`hello@payoncepdf.com`
- 价格口径集中文件：`config/product.json`（**冲突已于 2026-11-16 22:13 裁决**：`29 / 169 / 19 / 109`，`pending_confirmation: false`，详见 §3）

## 1. 映射表（历史 → 当前）

| 类别 | 历史值 | 当前值 |
|---|---|---|
| 产品名（英） | `OncePDF` / `Once PDF` | `PayOnce PDF` |
| 产品名（中） | `一次 PDF` | `一次付费 PDF` |
| 包名 / 小写标识 | `oncepdf` | `payoncepdf` |
| 应用 identifier | `app.oncepdf.desktop` | `app.payoncepdf.desktop` |
| Cargo package / lib | `oncepdf` / `oncepdf_lib` | `payoncepdf` / `payoncepdf_lib` |
| npm package | `oncepdf` | `payoncepdf` |
| 主可执行文件 | `oncepdf.exe` | `payoncepdf.exe` |
| 域名 | `oncepdf.com` / `oncepdf.app` | `payoncepdf.com` |
| 激活端点 | `https://oncepdf.app/api/activate` | `https://payoncepdf.com/api/activate` |
| 更新 feed | `https://oncepdf.app/api/update/...` | `https://payoncepdf.com/api/update/...`（路径语义不变） |
| 事件名（伪协议 handler） | `oncepdf://progress` / `oncepdf://finished` | `payoncepdf://progress` / `payoncepdf://finished` |
| 临时目录 / 日志 | `oncepdf-<jobId>/`、`oncepdf.log` | `payoncepdf-<jobId>/`、`payoncepdf.log` |
| 环境变量 | `ONCEPDF_ACTIVATE_URL`、`ONCEPDF_MOCK_PORT`、`ONCEPDF_PROVISION_ROOT` | `PAYONCEPDF_*` 同名后缀 |
| 测试样本标记 | `ONCEPDF_PAGE_NNNN`、`ONCEPDF-CANARY-8F3A91` | `PAYONCEPDF_PAGE_NNNN`、`PAYONCEPDF-CANARY-8F3A91` |
| 买断密钥前缀 | `ONCE-XXXX-XXXX-XXXX`（19 字符） | `PAYONCE-XXXX-XXXX-XXXX`（22 字符） |
| Windows 安装包 | `OncePDF_1.0.0_x64-setup.exe` | `PayOnce-PDF-Setup-1.0.0.exe` |
| macOS 磁盘映像 | `OncePDF-<arch>.dmg` / `OncePDF-universal.dmg` | `PayOnce-PDF-1.0.0-<arch>.dmg` / `PayOnce-PDF-1.0.0.dmg` |
| macOS 应用包 | `OncePDF.app` | `PayOnce PDF.app` |
| 支持邮箱 | `support@example.com` / `refunds@example.com` 等占位 | `hello@payoncepdf.com` |
| 仓库路径 | `E:\PainRadar\oncepdf` → `E:\OncePDF` → `E:\PayOncePDF` | 2026-11-16 21:35 完成第二次目录重命名：先停掉 vite dev server 与 esbuild 子进程，同盘符原子移动目录，再于新路径重启 1420 预览服务。文档正文统一写作 `<repo>`；`src-tauri/target/**/*.d` 与 `tests/out/*.log` 里的旧绝对路径是历史编译缓存 / 构建日志，保留原值不改，口径见 `tests/out/MIGRATION.md` 顶部注记 |

> identifier 说明：任务映射写的是 `com.oncepdf.* → com.payoncepdf.*`，但仓库实际值是 `app.oncepdf.desktop`。为最小化不必要变更，保留 `app.` 前缀，仅替换品牌段，得到 `app.payoncepdf.desktop`。若你要求严格改成 `com.payoncepdf.desktop`，需另行批准（会改变应用数据目录）。

## 2. 尚未发布，因此不做兼容

- **许可证**：旧前缀 `ONCE-XXXX-XXXX-XXXX` 一律拒绝。Rust `validate_key_format()` 已改为 22 字符 / `PAYONCE-` 前缀，并保留一条 `legacy_prefix_is_rejected` 迁移用例（只证伪，不兼容）；TS 正则、mock 激活服务、示例密钥、客服与手册文案同步。
- **试用状态**：`TRIAL_SALT` 从 `OncePDF/trial/v1` 改为 `PayOnce PDF/trial/v1`。这会让改名前的本机试用/激活缓存摘要失效；因产品尚未发布，无真实用户受影响。
- **更新签名**：`.toolchain/oncepdf-updater.key` 已改名为 `.toolchain/payoncepdf-updater.key`（及其 `.pub`）。**密钥内容与口令未变**，历史开发口令为 `oncepdf-dev-only`；发布前仍必须换成生产密钥对并替换 `tauri.conf.json` 的 `pubkey`。
- **旧安装包**：历史产物 `OncePDF_1.0.0_x64-setup.exe` / `.sig` 及 26.98 MiB 首轮包仍是内部测试件，不得对外分发；新包见 `docs/test-report.md` §15。

## 3. 价格冲突（已裁决，2026-11-16 22:13）

用户确认「参考资料 `$19 / ¥149` 改成 `$29 / ¥169`」。`config/product.json` 的四个字段 `price_usd` / `price_cny` / `early_bird_usd` / `early_bird_cny`（`29 / 169 / 19 / 109`）现在是唯一来源，`pending_confirmation` 已置为 `false`。

- PRD / 项目内落地页 / 官网：`$29 / ¥169`，首发 14 天早鸟 `$19 / ¥109`（16:45 已同步）。
- `E:\PainRadar\docs\pdf-launch\` 的 `landing.html`、`landing.zh.html`、`copy.md`、`posts.md`、`preorder.md`：22:13 已把决策前正式价 `$19 / ¥149` 统一改为 `$29 / ¥169`，共 27 处（26 处批量精确替换，每条规则断言命中 1 次；另 1 处为 `posts.md` 顶部状态注记）。
- **有意保留不变**：`$9 / ¥59` 可退定金（预约金，不是售价）、`$19 / ¥109` 首发 14 天早鸟价、`$19 / $29 / $39` 预购 A/B 三档、竞品价、以及 `docs/pain-and-competitive-analysis.md` §2–§10 与 `docs/pricing-decision.md` 中的决策前历史基线；`workbuddy-rename-prompt.md` 是改名任务原文，仅追加状态注记。

裁决详情见 `config/product.json` 的 `_resolution`，复验见 `docs/test-report.md` §15 V-15.15。

## 4. 关于验收 grep 的技术说明

任务要求的 `rg -n -i "oncepdf|once[ _-]?pdf"` 会**命中新名称本身**：`PayOnce PDF` 内含 `Once PDF`，`payoncepdf.com` 内含 `oncepdf`。因此「字面改后 0 命中」不可能成立。本仓库实际使用的残留检测正则是：

```bash
rg -n -i '\boncepdf|\bonce[ _-]?pdf\b|\bONCE-' .
```

该正则在 `PayOnce PDF` / `payoncepdf` 上不命中（缺少词边界），可准确检出真旧名。本文件与 `E:\PainRadar\docs\pdf-launch\workbuddy-rename-prompt.md`（改名任务原始说明）是两处有意保留的历史/规范文档。
