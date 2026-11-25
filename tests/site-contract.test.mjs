/**
 * site-contract.test.mjs - 官网部署物与一键部署脚本的静态契约（Node 22 内置 node:test）。
 * 并入 npm test，让 CI 的 quality job 在构建前守住官网结构，不依赖网络或 Cloudflare 账号。
 */
import test from 'node:test';
import assert from 'node:assert/strict';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkLocalFiles, PAGES } from '../scripts/verify-cloudflare-site.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const read = (p) => fs.readFileSync(path.join(ROOT, p), 'utf8');

test('官网部署目录通过全部本地验收门禁', () => {
  const gate = checkLocalFiles();
  assert.deepEqual(gate.blockers, [], `官网存在阻断项：${gate.blockers.join(' | ')}`);
  assert.equal(gate.stats.pages, 12);
  assert.ok(gate.stats.links > 100, '站内链接总数应符合 8 页规模');
});

test('一键部署脚本默认只检查，显式 --deploy 才部署', () => {
  const src = read('scripts/deploy-cloudflare-site.mjs');
  assert.match(src, /if \(!opts\.deploy\)/, '必须存在「非 --deploy 就返回」的分支');
  assert.match(src, /wrangler login/, '未登录时必须给出 login 指引');
  assert.doesNotMatch(src, /spawnSync\([^)]*['"]login['"]/, '不得自动执行 wrangler login');
  assert.match(src, /pages/, '部署命令必须是 pages deploy');
  assert.match(src, /--project-name/, '部署命令必须指定项目名');
});

test('部署脚本识别 API Token 认证模式，且不篡改用户凭据', () => {
  const src = read('scripts/deploy-cloudflare-site.mjs');
  assert.match(src, /process\.env\.CLOUDFLARE_API_TOKEN/, '必须检测 CLOUDFLARE_API_TOKEN');
  assert.match(src, /mode: 'api-token'/, '必须区分 Token 认证与 OAuth');
  assert.match(src, /CLOUDFLARE_ACCOUNT_ID/, '必须支持显式 account-id');
  assert.match(src, /You are logged in with an API Token/, '必须说明 login 冲突的原因');
  assert.doesNotMatch(src, /process\.env\.CLOUDFLARE_API_TOKEN\s*=/, '不得覆盖用户的 API Token');
  assert.doesNotMatch(src, /spawnSync\([^)]*['"]login['"]/, '不得自动执行 wrangler login');
});

test('部署脚本复用验收门禁，且只从 site-cloudflare/ 部署', () => {
  const src = read('scripts/deploy-cloudflare-site.mjs');
  assert.match(src, /checkLocalFiles\(\)/, '部署前必须跑本地验收');
  assert.match(src, /if \(gate\.blockers\.length\)[\s\S]*?process\.exitCode = 1/, '门禁失败必须中止');
  assert.match(src, /cwd: SITE/, 'wrangler 的工作目录必须是 site-cloudflare/');
  assert.match(read('site-cloudflare/wrangler.toml'), /pages_build_output_dir = "\."/);
});

test('npm 提供 site:check / site:deploy / site:verify 三个入口', () => {
  const pkg = JSON.parse(read('package.json'));
  assert.equal(pkg.scripts['site:check'], 'node scripts/deploy-cloudflare-site.mjs --check');
  assert.equal(pkg.scripts['site:deploy'], 'node scripts/deploy-cloudflare-site.mjs --deploy');
  assert.equal(pkg.scripts['site:verify'], 'node scripts/verify-cloudflare-site.mjs');
  assert.match(pkg.scripts.test, /site-contract\.test\.mjs/, '官网契约必须进入 CI 门禁');
});

test('_redirects 只含可生效的路径规则（无 200 rewrite、无域名级死规则）', () => {
  const txt = read('site-cloudflare/_redirects');
  const rules = txt.split('\n').map((l) => l.trim()).filter((l) => l && !l.startsWith('#'));
  const rewrites = rules.filter((l) => /\s200\s*$/.test(l));
  const redirects301 = rules.filter((l) => /\s301\s*$/.test(l));
  assert.equal(rewrites.length, 0,
    'Pages 原生支持干净路径；200 rewrite 会让 /x → /x.html 与 .html→/x 的 308 互相来回');
  assert.equal(rules.filter((l) => /^https?:\/\//.test(l)).length, 0,
    'Pages 官方明确不支持域名级重定向；www→apex 必须用 Redirect Rules，不得留死规则');
  assert.ok(redirects301.length >= 2, '须保留 /index.en.html 与 /zh/ 两条旧路径 301 兼容');
});

test('首次部署只在项目明确不存在时自动创建 Pages 项目', () => {
  const src = read('scripts/deploy-cloudflare-site.mjs');
  assert.match(src, /Pages project .* does not exist/i, '必须精确匹配 Wrangler 的项目不存在报错');
  assert.match(src, /'pages', 'project', 'create'/, '首次部署须显式创建项目');
  assert.match(src, /--production-branch/, '创建项目须指定生产分支');
});

test('线上 HTTP 验收跟随重定向，而非把 Pages 的 308 误判为失败', () => {
  const src = read('scripts/verify-cloudflare-site.mjs');
  assert.match(src, /redirect: 'follow'/, '.html 308 到干净路径是 Pages 预期行为');
  assert.match(src, /疑似重定向循环/, '循环必须被明确报出');
});

test('12 页中英镜像齐全，Gumroad 保持占位且不造假直链', () => {
  assert.equal(PAGES.length, 12);
  const all = PAGES.map((p) => read(`site-cloudflare/${p}`)).join('\n');
  assert.doesNotMatch(all, /href="https:\/\/gumroad\.com\/l\//i);
  assert.equal((all.match(/aria-disabled="true"/g) || []).length, 2);
  assert.doesNotMatch(all, /\$29|¥149/, '部署物不得残留早鸟价');
});

test('隐私披露中英 12 页一致，明确告知激活提交的指纹哈希', () => {
  for (const p of PAGES) {
    const s = read(`site-cloudflare/${p}`);
    assert.match(s, /SHA-256/, `${p} 必须披露指纹哈希算法`);
    assert.match(s, /设备指纹|device fingerprint/, `${p} 必须说明提交了设备指纹`);
    assert.match(s, /不用于广告|no ads, no selling/, `${p} 必须写明邮箱用途边界`);
    assert.doesNotMatch(
      s,
      /我们不主动收集邮箱|we don't collect emails automatically|does not collect emails automatically/,
      `${p} 仍有含混的旧口径`,
    );
  }
  assert.match(read('site-cloudflare/help.html'), /激活会上传什么/);
  assert.match(read('site-cloudflare/en/help.html'), /What does activation send/);
  for (const p of ['help.html', 'en/help.html']) {
    const s = read(`site-cloudflare/${p}`);
    const m = s.match(/<script type="application\/ld\+json">([\s\S]*?)<\/script>/);
    assert.ok(m, `${p} 必须有 FAQ JSON-LD`);
    const ld = JSON.parse(m[1]);
    assert.equal(ld['@type'], 'FAQPage');
    assert.ok(
      ld.mainEntity.some((q) => /激活会上传什么|What does activation send/.test(q.name)),
      `${p} 的 JSON-LD 需同步新增激活问答`,
    );
  }
});

test('按 Grabber 口径新增的中英法务页不编造运营主体', () => {
  const legal = ['privacy.html', 'terms.html', 'en/privacy.html', 'en/terms.html'];
  for (const p of legal) {
    const s = read(`site-cloudflare/${p}`);
    assert.match(s, /rel="canonical"/, `${p} 必须有 canonical`);
    assert.match(s, /hello@payoncepdf\.com/, `${p} 必须给出唯一联系邮箱`);
    assert.match(s, /Gumroad/, `${p} 必须披露收款主体`);
    assert.match(s, /7 天|7 days|7-day/, `${p} 必须写明试用/退款期限`);
    assert.match(s, /SHA-256/, `${p} 必须披露指纹哈希`);
    assert.doesNotMatch(s, /有限公司|有限责任公司|Inc\.|LLC|统一社会信用代码|注册地址：/,
      `${p} 不得编造运营主体法定名称与注册地`);
  }
  assert.match(read('site-cloudflare/terms.html'), /\$39 \/ ¥199/);
  assert.match(read('site-cloudflare/en/terms.html'), /\$39 \/ ¥199/);
  assert.match(read('site-cloudflare/privacy.html'), /隐私政策/);
  assert.match(read('site-cloudflare/en/privacy.html'), /Privacy Policy/);
  for (const p of PAGES) {
    const s = read(`site-cloudflare/${p}`);
    assert.match(s, /href="\/(?:en\/)?privacy\.html"/, `${p} footer 缺少隐私政策链接`);
    assert.match(s, /href="\/(?:en\/)?terms\.html"/, `${p} footer 缺少服务条款链接`);
  }
});

test('下载页 SHA-256 校验器：哈希与交付文档一致，脚本纯本地无网络', () => {
  const hash = (read('README.md').match(/SHA-256\s+`([0-9a-f]{64})`/i) || [])[1];
  assert.ok(hash, 'README 必须给出当前发行包 SHA-256');
  for (const p of ['download.html', 'en/download.html']) {
    const s = read(`site-cloudflare/${p}`);
    assert.ok(s.includes(`data-hash="${hash}"`), `${p} 的校验值必须与 README 一致`);
    assert.match(s, /id="pkg-sha256"/);
    assert.match(s, /id="pkg-file"/);
    assert.match(s, /src="\/verify-sha256\.js"/);
    assert.ok(
      s.includes('https://dl.payoncepdf.com/v1.0.1/PayOnce-PDF-Setup-1.0.1.exe'),
      `${p} 必须指向真实安装包直链（R2 自定义域）`
    );
    assert.ok(
      s.includes('https://dl.payoncepdf.com/v1.0.1/SHA256SUMS.txt'),
      `${p} 必须提供官方校验清单`
    );
    assert.match(s, /未做代码签名|not code-signed|not code-signed yet/,
      `${p} 必须披露未签名与 SmartScreen 风险`);
    // 直链之外不得出现其它安装包域名，防止有人塞进未核验的镜像
    const exeLinks = [...s.matchAll(/https?:\/\/[^\s"']*\.exe/g)].map((m) => m[0]);
    assert.deepEqual([...new Set(exeLinks)],
      ['https://dl.payoncepdf.com/v1.0.1/PayOnce-PDF-Setup-1.0.1.exe'],
      `${p} 只允许一个经核验的安装包直链`);
  }
  const js = read('site-cloudflare/verify-sha256.js');
  assert.match(js, /crypto\.subtle\.digest/);
  assert.match(js, /arrayBuffer/);
  assert.doesNotMatch(js, /\b(?:fetch|XMLHttpRequest|WebSocket|sendBeacon|importScripts)\s*\(/,
    '校验脚本不得发起任何网络请求');
  assert.match(read('site-cloudflare/_headers'), /script-src 'self'/,
    'CSP 必须允许同源外部脚本，否则校验器会被拦截');
});

test('线上验收在代理环境下给出可操作提示，不把本机直连失败误判为站点故障', () => {
  const src = read('scripts/verify-cloudflare-site.mjs');
  assert.match(src, /NODE_USE_ENV_PROXY/, 'fetch failed 时须提示启用环境代理');
  assert.match(src, /fetch failed/, '提示只在 fetch failed 时触发');
  assert.match(src, /HTTPS_PROXY/, '须检测系统代理环境变量');
});

test('首页试用 CTA 指向下载页，下载页试用入口是真实直链（mailto 仅作兜底）', () => {
  // 症状回归：用户反馈「首页点立即开始 7 天试用后无法进入下载页面」。
  // 真因是下载页主按钮 href="mailto:"，未配默认邮件程序的机器点了毫无反应。
  const home = [
    ['index.html', '/download.html'],
    ['en/index.html', '/en/download.html'],
  ];
  for (const [p, target] of home) {
    const s = read(`site-cloudflare/${p}`);
    assert.ok(
      s.includes(`class="btn primary" href="${target}"`),
      `${p} 的主 CTA 必须直连下载页 ${target}`
    );
  }
  for (const p of ['download.html', 'en/download.html']) {
    const s = read(`site-cloudflare/${p}`);
    assert.ok(
      s.includes('class="btn primary" href="https://dl.payoncepdf.com/v1.0.1/PayOnce-PDF-Setup-1.0.1.exe"'),
      `${p} 的主 CTA 必须是真实下载直链`
    );
    assert.match(
      s,
      /<button type="button" class="btn ghost" data-getbuild-open data-widget="get-build">/,
      `${p} 必须保留 mailto 兜底按钮（且不是主按钮）`
    );
    assert.match(s, /class="getbuild" id="get-build"/, `${p} 必须有就地展开的登记面板`);
    assert.match(s, /data-copy="email"/, `${p} 必须提供一键复制邮箱`);
    assert.match(s, /data-copy="mail"/, `${p} 必须提供一键复制整封邮件`);
    assert.match(s, /aria-live="polite"/, `${p} 的复制反馈必须可被读屏播报`);
    assert.match(s, /src="\/get-build\.js"/, `${p} 必须加载登记脚本`);
    assert.doesNotMatch(
      s,
      /<a[^>]*class="btn primary"[^>]*href="mailto:/,
      `${p} 不得再出现整块 mailto 主按钮`
    );
  }
  const js = read('site-cloudflare/get-build.js');
  assert.match(js, /navigator\.clipboard/, '优先用 clipboard API');
  assert.match(js, /execCommand\('copy'\)/, 'clipboard 不可用时要能回退');
  assert.match(js, /aria-expanded/, '必须维护展开态，避免读屏与视觉不一致');
  assert.doesNotMatch(
    js,
    /\b(?:fetch|XMLHttpRequest|WebSocket|sendBeacon|importScripts)\s*\(/,
    '登记脚本不得发起任何网络请求'
  );
});
