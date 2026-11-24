# 生产激活服务实施设计（参考 Grabber，尚未实现 / 未部署）

> 状态：**设计稿**。本文件不表示服务已上线。当前仅 `scripts/license-server.mjs` 本地 mock 可跑，
> 客户端默认激活端点仍是占位域名，官网购买按钮仍是 `aria-disabled` 占位。
> 2026-11-24 按用户「参考 Grabber 及官网的做法」整理，作为 `DEPLOYMENT.md` 决策清单第 1/2 项的可执行方案。

## 0. 参考来源（均为本地只读调研，未复制任何密钥）

| 来源 | 可复用的做法 |
| --- | --- |
| `E:/grabber/site/functions/api/desktop-activate.ts` | Gumroad license key 校验 → 查/补订单 → 名额原子控制 → Ed25519 签发 → 返回 `license.json` |
| `E:/grabber/site/functions/api/pay/gumroad.ts` | Gumroad Ping **不带签名**，必须用 `GUMROAD_ACCESS_TOKEN` 回查 `/v2/sales/{id}`，未配 token 时失败关闭 |
| `E:/grabber/site/schema.sql` / `migrations/001_init.sql` | `orders / licenses / mobile_codes / events` 四表 + D1 绑定 |
| `E:/grabber/site/functions/api/_lib/license.ts` | PKCS#8 PEM 末 32 字节作 Ed25519 种子；载荷 `machine\|edition\|issuedAt\|updatesUntil\|note` |
| `E:/grabber/site/functions/_middleware.ts` | Tauri 的 `tauri://localhost` / `http://tauri.localhost` 发 JSON 会先 OPTIONS 预检，CORS 必须先于鉴权 |
| `E:/grabber/site/functions/buy/[plan].ts` + `content/stage.json` | 购买入口统一走 `/buy/<plan>` 中间跳转，`private_beta` / `live` 单一开关，内测态禁止出现 Gumroad 购买直链 |
| `E:/grabber/site/functions/api/device-report.ts` | 服务端 `firstSeen` / `serverNow` 让卸载重装、改系统时间都无法重置试用期 |

PayOncePDF 与 Grabber 的差异：只有单一档 `$39 / ¥199`、每把 key 最多 **3 台**、无手机端、无 8 位激活码、无国内支付回调。因此只取「订单校验 + 名额 + 离线签发 + 开关」四块，不照搬多席位与移动端。

## 1. 目标链路

1. 用户在价格页点购买 → `/buy/personal`（或直连 Gumroad）→ 结账 → 拿到 license key / 订单号。
2. 桌面端「设置 → 授权」输入 key → `POST https://payoncepdf.com/api/activate`。
3. 服务端向 Gumroad `POST /v2/licenses/verify` 验真（**必须再检查 `refunded` / `chargebacked` / `disputed` / `subscription_*`，`success:true` 只说明 key 存在**）。
4. 查 D1 `orders`（无则凭真实购买数据补单），校验邮箱、金额、状态 `paid`。
5. 按 `licenses` 表 `revoked_at IS NULL` 计数，`INSERT … SELECT … WHERE COUNT < 3` 原子防超发；同机重复激活返回已有授权（`reused: true`）。
6. 用生产 Ed25519 私钥签发 `activation.json`，客户端 `src-tauri/src/license.rs` 用编译期内嵌公钥离线验签。

Gumroad Ping 只用于**自动补单**：收到 `sale_id` 后必须回查 Gumroad API，禁止信任 Ping 正文；未配置 `GUMROAD_ACCESS_TOKEN` 时返回 503 且不建单。

## 2. 数据模型（D1 / SQLite，参考 schema.sql 裁剪）

```sql
CREATE TABLE IF NOT EXISTS orders (
  id TEXT PRIMARY KEY,             -- 用户可见订单号
  provider TEXT NOT NULL,          -- gumroad / manual
  provider_ref TEXT,               -- Gumroad sale_id（幂等键）
  email TEXT, plan TEXT NOT NULL DEFAULT 'personal',
  amount_cents INTEGER, currency TEXT DEFAULT 'USD',
  status TEXT DEFAULT 'pending',   -- pending / paid / refunded / cancelled
  seats INTEGER DEFAULT 3,
  created_at INTEGER, paid_at INTEGER, note TEXT
);
CREATE TABLE IF NOT EXISTS licenses (
  id TEXT PRIMARY KEY, order_id TEXT, machine TEXT,
  payload TEXT,                    -- 签发的 activation.json 全文
  edition TEXT DEFAULT 'Standard',
  created_at INTEGER, revoked_at INTEGER
);
CREATE TABLE IF NOT EXISTS events (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  kind TEXT, order_id TEXT, machine TEXT, ip TEXT, ua TEXT, at INTEGER
);
```

- `machine` 是**设备指纹的 SHA-256 哈希**，不是原始硬件信息（与官网隐私政策披露一致）。
- 退款 / 拒付时把 `orders.status` 置 `refunded` 并给对应 `licenses.revoked_at` 打时间戳。
- 设备台账与审计流水的**留存期限尚未确定**，上线前必须写进 `/privacy`，不得默认无限期保留。

## 3. 端点契约（设计，未实现）

| 方法 / 路径 | 请求 | 成功响应 | 失败 |
| --- | --- | --- | --- |
| `POST /api/activate` | `{key, machine, email?}` | `{ok:true, license:{...}, reused?}` | 403 已退款/争议、402 未支付、409 名额已满、429 限流 |
| `GET /api/license?machine=` | — | `{ok:true, license, issuedAt}` | 404 未激活 |
| `POST /api/license/unbind` | `{machine, order}` | `{ok:true, unbound}` | 403 超出换机次数 |
| `POST /api/pay/gumroad` | Gumroad Ping 表单 | `{ok:true, id}` | 503 缺 token（失败关闭）、403 回查不通过 |
| `GET /buy/personal` | — | 302 → Gumroad（或内测态 302 → `/beta` 说明页） | — |

签名载荷沿用客户端现有 `payload_matches(payload, key, fp)` 的字段约定；**改动载荷格式必须同步重编译客户端**，因为 `include_str!("../keys/public.key")` 把公钥钉死在二进制里。

## 4. 必须守住的安全点

1. 私钥只在 `wrangler pages secret put LICENSE_PRIVATE_KEY`，绝不入库、不进前端、不进 CI 日志。
2. Gumroad `verify` 的 `increment_uses_count` 传 `false`；每次启动联网校验会破坏「断网可用」卖点，授权文件以离线验签为准。
3. CORS 只允许站点自身需要的来源，`OPTIONS` 预检先于任何鉴权；`/api/admin/*` 单独 Basic Auth。
4. 每个 IP 对激活 / 补单做限流（Grabber 用 `events` 表近似计数，60 次 / 小时）。
5. 名额控制必须是**一条原子 SQL**，不能先 SELECT 再 INSERT。
6. 生产 minisign 与生产 Ed25519 是两把不同的密钥，分别管更新包与授权文件，不要混用。
7. 私钥泄露 = 全部授权失效，需轮换公钥、重新编译发行包、重新打包并更新官网哈希。

## 5. 官网开关（参考 stage.json）

- 新增 `site-cloudflare/content/stage.json`（或等价单一事实源）：`stage: private_beta | live`、`salesOpen`、`GUMROAD_*_URL` 由 Pages 环境变量提供，**不写死在 HTML**。
- 内测态：价格页按钮指向「购买与激活流程」说明页，禁止出现任何 Gumroad 购买直链（Grabber 的 `apply_stage.py` 就是这么做的）。
- 开卖态：按钮 → `/buy/personal` 302 → Gumroad；同时把 `src/lib/commerce.ts` 的 `PAYMENT_URL` 改成同一地址，并移除 `aria-disabled` 与 `#gumroad-pending` 说明框。
- 每次切换都要跑 `npm test` 与 `npm run site:verify -- https://payoncepdf.com`，契约测试需同步更新「占位数量」断言。

## 6. 需要用户先给的东西（我无法代做）

| 项 | 说明 |
| --- | --- |
| Gumroad 商品 URL 与 product_id | 定价 `$39 / ¥199`，不设折扣码 |
| `GUMROAD_ACCESS_TOKEN` | 具备 `view_sales` 权限，用于 Ping 回查；由用户保管为 Pages secret |
| 生产 Ed25519 密钥对 | 私钥存 secret，公钥替换 `src-tauri/keys/public.key` 后重新编译 |
| D1 数据库 | 创建并绑定；`wrangler.toml` 里写 `database_id` |
| 退款期限与留存期限 | 官网已按 Grabber 口径写 7 天无理由；数据留存期限仍待确认，须写进隐私政策 |
| 订单邮箱匹配规则 | 是否强制 `email` 精确匹配（Grabber 在 v>=2 时强制） |

## 7. 验收清单（信息齐备后逐条跑）

- [ ] 有效 key + 新机器 → 返回签名合法的 `activation.json`，客户端 `activation_is_valid()` 为真
- [ ] 同一 key 第 4 台机器 → 409「名额已用完（3 台）」
- [ ] 同机重复激活 → `reused: true`，不额外占名额
- [ ] 改掉授权文件一个字符 → 客户端报「授权签名无效」
- [ ] A 机器的授权拿到 B 机器 → 报「授权与本机不匹配」
- [ ] 已退款 / chargeback 的 key → 403，且无法激活
- [ ] 伪造 Gumroad Ping（无有效 sale_id）→ 不建单
- [ ] 断网状态下导入授权文件 → 仍可激活
- [ ] OPTIONS 预检通过，Tauri 内真实 POST 成功
- [ ] 退款后 `orders.status` 与 `licenses.revoked_at` 同步
- [ ] 官网阶段切换前后，按钮、占位标记、CSP、契约测试全部一致

> 在这 11 条跑完并留证之前，激活服务、Gumroad 与自动发货一律保持 NO-GO，不会以 mock 冒充生产。
