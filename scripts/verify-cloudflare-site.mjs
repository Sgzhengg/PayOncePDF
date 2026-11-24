#!/usr/bin/env node
/**
 * verify-cloudflare-site.mjs — PayOncePDF 官网部署前后验收（零依赖，Node 22+）
 *
 *   node scripts/verify-cloudflare-site.mjs                          # 只查本地文件
 *   node scripts/verify-cloudflare-site.mjs http://127.0.0.1:8902     # 查本地预览
 *   node scripts/verify-cloudflare-site.mjs https://xxx.pages.dev      # 查线上站
 *
 * 退出码 0 = 无阻断项；1 = 存在阻断项。WARN 不阻断（例如本地普通静态服务器
 * 不支持 _redirects 干净路径）。
 *
 * 本地检查同时被 deploy-cloudflare-site.mjs 复用，作为部署前的门禁。
 */
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SITE = path.join(ROOT, 'site-cloudflare');

export const PAGES = [
  'index.html',
  'download.html',
  'pricing.html',
  'help.html',
  'en/index.html',
  'en/download.html',
  'en/pricing.html',
  'en/help.html',
];

const REQUIRED = [
  ...PAGES,
  'styles.css',
  '_headers',
  '_redirects',
  'wrangler.toml',
  'README.md',
];

// 干净路径只在 Cloudflare Pages / 专用预览脚本下生效，页面内链接必须直连 .html
const CLEAN_PATH = /^\/(?:download|pricing|help|en|en\/(?:download|pricing|help))\/?$/;
const VOID_TAGS = new Set([
  'meta', 'link', 'br', 'hr', 'img', 'input', 'source', 'wbr',
  'area', 'base', 'col', 'embed', 'param', 'track',
]);

const OK = (s) => `\x1b[32m${s}\x1b[0m`;
const BAD = (s) => `\x1b[31m${s}\x1b[0m`;
const WARN = (s) => `\x1b[33m${s}\x1b[0m`;

function tagBalance(html) {
  // 去掉注释与 script/style 内容，避免 JSON-LD / CSS 里的 "<" 干扰栈检查
  const cleaned = html
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<script\b[^>]*>[\s\S]*?<\/script>/gi, '')
    .replace(/<style\b[^>]*>[\s\S]*?<\/style>/gi, '');
  const stack = [];
  const errors = [];
  const re = /<\/?([a-zA-Z][a-zA-Z0-9-]*)\b[^>]*>/g;
  let m;
  while ((m = re.exec(cleaned)) !== null) {
    const raw = m[0];
    const tag = m[1].toLowerCase();
    if (raw.startsWith('<!')) continue;
    if (raw.endsWith('/>') || VOID_TAGS.has(tag)) continue;
    if (raw.startsWith('</')) {
      if (stack.length === 0) errors.push(`多余 </${tag}>`);
      else if (stack[stack.length - 1] === tag) stack.pop();
      else errors.push(`期望 </${stack[stack.length - 1]}> 却是 </${tag}>`);
    } else {
      stack.push(tag);
    }
  }
  if (stack.length) errors.push(`未闭合 ${stack.join(',')}`);
  return errors;
}

function printResults(results, blockers, warnings) {
  for (const r of results) {
    console.log(`  ${r.ok ? OK('PASS') : BAD('FAIL')}  ${r.name}: ${r.detail}`);
  }
  for (const w of warnings) console.log(`  ${WARN('WARN')}  ${w}`);
  if (blockers.length) {
    console.log(`\n=> 存在 ${blockers.length} 项阻断，禁止部署 / 上线`);
  } else {
    console.log('\n=> 官网验收通过（无阻断项）');
  }
}

/** 本地文件层验收。返回 { results, blockers, warnings, stats }。 */
export function checkLocalFiles() {
  const results = [];
  const blockers = [];
  const warnings = [];
  const add = (name, ok, detail) => {
    results.push({ name, ok, detail });
    if (!ok) blockers.push(`${name}: ${detail}`);
  };
  const read = (p) => fs.readFileSync(path.join(SITE, p), 'utf8');

  const missing = REQUIRED.filter((p) => !fs.existsSync(path.join(SITE, p)));
  add('必需文件齐全', missing.length === 0,
    missing.length ? `缺少 ${missing.join(', ')}` : `${REQUIRED.length} 个文件`);

  // 部署目录卫生：Cloudflare 会把目录内所有文件当静态资源发布
  const junk = [];
  const walk = (dir) => {
    for (const ent of fs.readdirSync(dir, { withFileTypes: true })) {
      const p = path.join(dir, ent.name);
      if (ent.isDirectory()) {
        if (ent.name === '__pycache__') junk.push(path.relative(SITE, p));
        walk(p);
      } else if (/\.(py|pyc)$/i.test(ent.name)) {
        junk.push(path.relative(SITE, p));
      }
    }
  };
  if (fs.existsSync(SITE)) walk(SITE);
  add('部署目录卫生', junk.length === 0,
    junk.length ? `混入 ${junk.join(', ')}` : '无 .py / .pyc / __pycache__');

  const balanceIssues = [];
  const navIssues = [];
  const linkIssues = [];
  const seoIssues = [];
  const priceIssues = [];
  let linkCount = 0;
  let ariaDisabled = 0;
  let fakeGumroad = 0;

  for (const p of PAGES) {
    if (!fs.existsSync(path.join(SITE, p))) continue;
    const html = read(p);

    const bal = tagBalance(html);
    if (bal.length) balanceIssues.push(`${p}: ${bal.slice(0, 2).join('; ')}`);

    const nav = html.match(/<nav\s+class="links"[^>]*>([\s\S]*?)<\/nav>/i);
    const anchors = nav ? [...nav[1].matchAll(/<a\b[^>]*>/gi)] : [];
    const active = anchors.filter((a) => /class="[^"]*\bactive\b/i.test(a[0])).length;
    if (anchors.length !== 4 || active !== 1) {
      navIssues.push(`${p}: ${anchors.length} 项 / ${active} active`);
    }

    const hrefs = [...html.matchAll(/<a\b[^>]*\bhref="([^"]*)"/gi)].map((m) => m[1]);
    linkCount += hrefs.length;
    for (const h of hrefs) {
      if (/^(https?:|mailto:|#)/i.test(h)) continue;
      const t = h.split('#')[0].split('?')[0];
      if (!t) continue;
      if (CLEAN_PATH.test(t)) linkIssues.push(`${p}: 干净路径残留 ${h}`);
      if (!fs.existsSync(path.join(SITE, t.replace(/^\//, '')))) {
        linkIssues.push(`${p}: 死链 ${h}`);
      }
    }

    const canonical = /rel="canonical"/i.test(html);
    const langs = ['en', 'zh-CN', 'x-default'].every((l) => html.includes(`hreflang="${l}"`));
    if (!canonical || !langs) seoIssues.push(`${p}: canonical=${canonical} hreflang=${langs}`);

    if (p.endsWith('pricing.html')) {
      if (!html.includes('$39') || !html.includes('¥199')) {
        priceIssues.push(`${p}: 未标注 $39 / ¥199 单一标价`);
      }
    }
    if (/\$29|¥149/.test(html)) priceIssues.push(`${p}: 残留早鸟价 $29 / ¥149`);
    ariaDisabled += (html.match(/aria-disabled="true"/g) || []).length;
    fakeGumroad += (html.match(/href="https:\/\/gumroad\.com\/l\//gi) || []).length;
  }

  add('8 页标签配平', balanceIssues.length === 0,
    balanceIssues.length ? balanceIssues.join(' | ') : '全部配平');
  add('8 页导航 4 项 / 1 active', navIssues.length === 0,
    navIssues.length ? navIssues.join(' | ') : '全部正确');
  add('站内链接可达', linkIssues.length === 0,
    linkIssues.length ? linkIssues.slice(0, 4).join(' | ') : `${linkCount} 条全部直连 .html`);
  add('canonical / hreflang', seoIssues.length === 0,
    seoIssues.length ? seoIssues.join(' | ') : '8 页齐全');
  add('价格口径', priceIssues.length === 0,
    priceIssues.length ? priceIssues.join(' | ') : '$39 / ¥199 单一标价');
  add('Gumroad 占位', ariaDisabled === 2 && fakeGumroad === 0,
    `aria-disabled=${ariaDisabled} / 假直链=${fakeGumroad}`);

  if (fs.existsSync(path.join(SITE, '_headers'))) {
    const h = read('_headers');
    add('_headers 安全头', h.includes('Content-Security-Policy') && h.includes('Strict-Transport-Security'),
      h.includes('Content-Security-Policy') && h.includes('Strict-Transport-Security')
        ? 'CSP / HSTS 齐备' : '缺少 CSP 或 HSTS');
  }
  if (fs.existsSync(path.join(SITE, '_redirects'))) {
    const r = read('_redirects');
    const rules = r.split('\n').map((l) => l.trim())
      .filter((l) => l && !l.startsWith('#'));
    const rewrites = rules.filter((l) => /\s200\s*$/.test(l));
    const redirects301 = rules.filter((l) => /\s301\s*$/.test(l));
    const hasWww = redirects301.some((l) => l.includes('www.payoncepdf.com'));
    const legacy = redirects301.some((l) => l.includes('/index.en.html'))
      && redirects301.some((l) => l.includes('/zh/'));
    add('_redirects 无循环 rewrite',
      rewrites.length === 0 && redirects301.length >= 3 && hasWww && legacy,
      rewrites.length
        ? `含 ${rewrites.length} 条 200 rewrite，会与 Pages 的 .html→无扩展名 308 形成无限循环`
        : `301 跳转 ${redirects301.length} 条 / 200 rewrite 0 条（干净路径由 Pages 原生提供）`);
  }

  return { results, blockers, warnings, stats: { pages: PAGES.length, links: linkCount } };
}

/** HTTP 层验收（本地预览或线上站）。返回 { results, blockers, warnings }。 */
export async function checkHttp(base) {
  const results = [];
  const blockers = [];
  const warnings = [];
  const add = (name, ok, detail) => {
    results.push({ name, ok, detail });
    if (!ok) blockers.push(`${name}: ${detail}`);
  };
  const isLocal = /127\.0\.0\.1|localhost/.test(base);
  const origin = base.replace(/\/$/, '');
  const paths = [
    ...PAGES.map((p) => `/${p}`),
    '/download', '/pricing', '/help', '/en/', '/en/pricing',
  ];

  const failures = [];
  for (const p of paths) {
    const isClean = CLEAN_PATH.test(p);
    try {
      // 跟随重定向：Cloudflare Pages 会把 /x.html 308 到干净路径 /x，属于预期行为；
      // 若存在循环（例如 200 rewrite 与 308 互相来回），fetch 会抛错并被记为失败。
      const r = await fetch(origin + p, {
        redirect: 'follow',
        signal: AbortSignal.timeout(12_000),
      });
      if (r.status !== 200) {
        if (isClean && isLocal) {
          warnings.push(`${p} → ${r.status}：普通静态服务器不解析 _redirects，请改用 scripts/serve-cloudflare-site.py`);
        } else {
          failures.push(`${p} → HTTP ${r.status}`);
        }
        continue;
      }
      const body = await r.text();
      const title = body.match(/<title>([^<]*)<\/title>/i);
      if (!title || !title[1].trim()) failures.push(`${p} → <title> 缺失`);
      if (!/PayOnce PDF/i.test(body)) failures.push(`${p} → 页面未标识 PayOnce PDF`);
    } catch (e) {
      const msg = e?.message || String(e);
      if (isClean && isLocal) warnings.push(`${p} → ${msg}`);
      else failures.push(`${p} → 请求失败：${msg}${/redirect/i.test(msg) ? '（疑似重定向循环）' : ''}`);
    }
  }
  add('逐页 HTTP 验收', failures.length === 0,
    failures.length
      ? failures.slice(0, 5).join(' | ') + (failures.length > 5 ? ` …共 ${failures.length} 项` : '')
      : `${paths.length} 条路径均可达（允许 Pages 的 .html→干净路径 308），标题与产品标识齐全`);

  if (!isLocal) {
    try {
      const r = await fetch(origin + '/', { signal: AbortSignal.timeout(12_000) });
      const csp = r.headers.get('content-security-policy');
      const hsts = r.headers.get('strict-transport-security');
      add('线上安全响应头', Boolean(csp && hsts),
        `CSP=${csp ? 'HIT' : 'MISS'} HSTS=${hsts ? 'HIT' : 'MISS'}`);
    } catch (e) {
      add('线上安全响应头', false, `无法读取：${e?.message || e}`);
    }
  } else {
    warnings.push('本地预览不检查 CSP/HSTS（_headers 只在 Cloudflare Pages 生效）');
  }

  return { results, blockers, warnings };
}

async function main() {
  const arg = process.argv[2];
  if (arg === '--help' || arg === '-h') {
    console.log('用法: node scripts/verify-cloudflare-site.mjs [http(s)://站点地址]');
    console.log('不带参数只做本地文件验收；带地址会追加 HTTP 逐页验收。');
    return;
  }
  console.log('=== 本地文件验收 ===');
  const local = checkLocalFiles();
  printResults(local.results, local.blockers, local.warnings);

  let httpBlockers = [];
  if (arg && /^https?:\/\//i.test(arg)) {
    console.log(`\n=== HTTP 验收: ${arg} ===`);
    const http = await checkHttp(arg);
    printResults(http.results, http.blockers, http.warnings);
    httpBlockers = http.blockers;
  } else if (arg) {
    console.log(`\n忽略无法识别的参数 ${arg}（应为 http:// 或 https:// 地址）`);
  }

  const total = local.blockers.length + httpBlockers.length;
  if (total) {
    console.log(`\n共 ${total} 项阻断。`);
    process.exitCode = 1;
  }
}

const isMain = process.argv[1]
  && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href;
if (isMain) await main();
