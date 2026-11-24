#!/usr/bin/env node
/**
 * deploy-cloudflare-site.mjs — PayOncePDF 官网一键部署（零依赖，Node 22+）
 *
 *   node scripts/deploy-cloudflare-site.mjs             # 只做部署前检查（默认，不会上线）
 *   node scripts/deploy-cloudflare-site.mjs --deploy    # 检查通过后执行 wrangler pages deploy
 *   node scripts/deploy-cloudflare-site.mjs --deploy --account-id <ID>
 *   node scripts/deploy-cloudflare-site.mjs --help
 *
 * 安全边界：
 *   - 默认不部署；只有显式 --deploy 才会调用 wrangler。
 *   - 不打开浏览器、不自动执行 wrangler login、不读写 Cloudflare 凭据文件、
 *     不修改用户级环境变量、不绑定自定义域、不修改 DNS。
 *   - 部署前强制跑 scripts/verify-cloudflare-site.mjs 的本地文件门禁，失败即中止。
 *
 * 认证模式（Wrangler 二选一，不能并存）：
 *   A. CLOUDFLARE_API_TOKEN 已设置 → Token 认证，已登录，**不要再执行 wrangler login**
 *      （会报 "You are logged in with an API Token"）。如需写权限，到 Cloudflare
 *      Dashboard → Profile → API Tokens 给该 Token 加 `Cloudflare Pages:Edit`。
 *   B. 未设置该变量 → OAuth，由你本人执行 npx wrangler login 浏览器授权。
 *   脚本只读检测模式，绝不打印 Token 内容，也不会 unset / 覆盖它。
 *
 * 前置条件（仅域名接入仍待用户操作）：
 *   域名 payoncepdf.com 的 NS 指向 Cloudflare（阿里云万网控制台操作）。
 */
import fs from 'node:fs';
import path from 'node:path';
import { spawnSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';
import { checkLocalFiles } from './verify-cloudflare-site.mjs';

const ROOT = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const SITE = path.join(ROOT, 'site-cloudflare');
const WIN = process.platform === 'win32';

function parseArgs(argv) {
  const out = {
    deploy: false,
    project: 'payoncepdf',
    branch: 'main',
    accountId: process.env.CLOUDFLARE_ACCOUNT_ID || '',
    help: false,
  };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--deploy') out.deploy = true;
    else if (a === '--check') out.deploy = false;
    else if (a === '--project') out.project = argv[++i] || out.project;
    else if (a === '--branch') out.branch = argv[++i] || out.branch;
    else if (a === '--account-id') out.accountId = argv[++i] || '';
    else if (a === '--help' || a === '-h') out.help = true;
    else throw new Error(`无法识别的参数：${a}`);
  }
  return out;
}

/** 优先用项目内安装的 wrangler；否则交给 npx --yes 获取。 */
function resolveWrangler() {
  const bins = WIN ? ['wrangler.cmd', 'wrangler.ps1', 'wrangler'] : ['wrangler'];
  for (const b of bins) {
    const p = path.join(ROOT, 'node_modules', '.bin', b);
    if (fs.existsSync(p)) {
      return { cmd: p, prefix: [], source: `项目内 ${path.relative(ROOT, p)}` };
    }
  }
  return {
    cmd: WIN ? 'npx.cmd' : 'npx',
    prefix: ['--yes', 'wrangler'],
    source: 'npx --yes wrangler（首次运行会自动下载，可能需要几分钟）',
  };
}

/**
 * 只读检测 Wrangler 认证模式，绝不读取或输出 Token 内容。
 * 环境变量 CLOUDFLARE_API_TOKEN 优先级最高：Wrangler 会强制走 Token 认证，
 * 此时执行 `wrangler login` 必然报 "You are logged in with an API Token"。
 * 精确判定（Token 是否有效、有无 Pages 权限）仍由 whoami 负责。
 */
function detectAuth() {
  if ((process.env.CLOUDFLARE_API_TOKEN || '').trim()) {
    return {
      mode: 'api-token',
      detail: 'API Token 认证（CLOUDFLARE_API_TOKEN 已设置，无需 wrangler login）',
    };
  }
  const home = process.env.USERPROFILE || process.env.HOME || '';
  const candidates = [
    path.join(home, '.wrangler', 'config', 'default.toml'),
    path.join(home, '.config', '.wrangler', 'config', 'default.toml'),
    process.env.APPDATA
      ? path.join(process.env.APPDATA, 'xdg.config', '.wrangler', 'config', 'default.toml')
      : null,
    process.env.XDG_CONFIG_HOME
      ? path.join(process.env.XDG_CONFIG_HOME, '.wrangler', 'config', 'default.toml')
      : null,
  ].filter(Boolean);
  const hit = candidates.find((p) => fs.existsSync(p));
  return hit
    ? { mode: 'oauth', detail: `OAuth 凭据文件 ${hit}（以 whoami 为准）` }
    : { mode: 'none', detail: '未检测到 Wrangler 凭据' };
}

function spawnWrangler(w, args, { inherit = false, cwd = ROOT, timeout, accountId = '' } = {}) {
  const env = {
    ...process.env,
    WRANGLER_SEND_METRICS: 'false',
    NO_UPDATE_NOTIFIER: '1',
    // 仅注入到 wrangler 子进程，不写用户级/系统级环境变量。
    ...(accountId ? { CLOUDFLARE_ACCOUNT_ID: accountId } : {}),
  };
  return spawnSync(w.cmd, [...w.prefix, ...args], {
    cwd,
    env,
    shell: WIN,
    encoding: 'utf8',
    stdio: inherit ? 'inherit' : ['ignore', 'pipe', 'pipe'],
    ...(timeout ? { timeout } : {}),
  });
}

function printLocalGate(gate) {
  console.log('=== 部署前门禁：本地文件验收 ===');
  for (const r of gate.results) {
    const mark = r.ok ? '\x1b[32mPASS\x1b[0m' : '\x1b[31mFAIL\x1b[0m';
    console.log(`  ${mark}  ${r.name}: ${r.detail}`);
  }
  for (const w of gate.warnings) console.log(`  \x1b[33mWARN\x1b[0m  ${w}`);
}

function printNextSteps(w, opts, auth) {
  console.log('\n下一步（脚本不会代做账号授权与 DNS 修改）：');
  if (auth.mode === 'api-token') {
    console.log('  1. 认证已完成：当前使用 CLOUDFLARE_API_TOKEN，**不要执行 wrangler login**');
    console.log('     （会报 "You are logged in with an API Token"）。若部署报权限不足，到');
    console.log('     Dashboard → Profile → API Tokens 给该 Token 添加 `Cloudflare Pages:Edit`。');
  } else {
    console.log('  1. npx wrangler login                         # 未设置 API Token 时才需要浏览器授权');
    console.log('     若已设置 CLOUDFLARE_API_TOKEN，请先在当前窗口清除它再 login，或直接走 Token 认证。');
  }
  console.log('  2. node scripts/deploy-cloudflare-site.mjs --deploy');
  console.log(`     → 实际执行：${w.cmd} ${[...w.prefix, 'pages', 'deploy', '.', '--project-name', opts.project, '--branch', opts.branch].join(' ')}`);
  console.log('     （工作目录 site-cloudflare/，首次部署会返回 *.pages.dev 地址）');
  console.log('  3. node scripts/verify-cloudflare-site.mjs https://<项目名>.pages.dev');
  console.log('  4. Cloudflare Pages → Settings → Domains 绑定 payoncepdf.com；');
  console.log('     阿里云万网把域名 NS 改为 Cloudflare 分配的两个 NS，等待 Universal SSL 变 Active。');
  console.log('\n提示：本脚本不绑定自定义域、不修改 DNS、不修改用户级环境变量、不触碰凭据。');
}

function main() {
  let opts;
  try {
    opts = parseArgs(process.argv.slice(2));
  } catch (e) {
    console.error(e.message);
    process.exitCode = 1;
    return;
  }

  if (opts.help) {
    console.log('用法: node scripts/deploy-cloudflare-site.mjs [--check|--deploy] [--project 名称] [--branch 分支] [--account-id ID]');
    console.log('默认 --check：只做门禁检查与状态报告，不部署。');
    console.log('--deploy：门禁通过后执行 wrangler pages deploy（需已通过 API Token 或 OAuth 认证）。');
    console.log('--account-id：仅注入到 wrangler 子进程；不传则用 CLOUDFLARE_ACCOUNT_ID，都没有时由 wrangler 自动推断。');
    console.log('注意：已设置 CLOUDFLARE_API_TOKEN 时 Wrangler 强制 Token 认证，执行 wrangler login 会报错；此时无需 login。');
    return;
  }

  const gate = checkLocalFiles();
  printLocalGate(gate);
  if (gate.blockers.length) {
    console.error(`\n=> ${gate.blockers.length} 项阻断，已中止（不会部署）`);
    process.exitCode = 1;
    return;
  }

  const w = resolveWrangler();
  const auth = detectAuth();
  console.log('\n=== 环境状态（只读，不显示任何凭据内容）===');
  console.log(`  Wrangler        : ${w.source}`);
  console.log(`  Cloudflare 认证 : ${auth.detail}`);
  console.log(`  Account ID      : ${opts.accountId ? `已提供（len=${opts.accountId.length}）` : '未指定，由 wrangler 从凭据推断'}`);
  console.log(`  部署目录        : ${SITE}`);
  console.log(`  项目 / 分支     : ${opts.project} / ${opts.branch}`);

  if (!opts.deploy) {
    console.log('\n=> 当前为 --check 模式，未执行任何部署、未发起任何网络写操作。');
    printNextSteps(w, opts, auth);
    return;
  }

  // --- 以下只有显式 --deploy 才会执行 ---
  console.log('\n=== 检查 Cloudflare 登录状态（wrangler whoami）===');
  const who = spawnWrangler(w, ['whoami'], { timeout: 120_000 });
  const whoOut = `${who.stdout || ''}${who.stderr || ''}`;
  if (who.error || who.status !== 0 || /not logged in|not authenticated/i.test(whoOut)) {
    if (auth.mode === 'api-token') {
      console.error('  API Token 认证未通过。脚本不会 unset 或覆盖该 Token，请检查：');
      console.error('   - Dashboard → Profile → API Tokens：Token 是否过期、是否具备账号/Pages 读权限；');
      console.error('   - 或临时清空当前窗口的 CLOUDFLARE_API_TOKEN 后改用 npx wrangler login。');
    } else {
      console.error('  未登录或无法验证登录状态。请先在本机执行：');
      console.error('    npx wrangler login');
      console.error('  授权完成后重新运行 --deploy。脚本不会自动打开浏览器或写入凭据。');
    }
    if (whoOut.trim()) console.error(`  whoami 输出：${whoOut.trim().split('\n').slice(0, 4).join(' | ')}`);
    process.exitCode = 1;
    return;
  }
  console.log(`  认证通过（${auth.mode === 'api-token' ? 'API Token' : 'OAuth'}）。`);

  const args = ['pages', 'deploy', '.', '--project-name', opts.project, '--branch', opts.branch];
  console.log(`\n=== 部署: ${w.cmd} ${[...w.prefix, ...args].join(' ')} ===`);
  const r = spawnWrangler(w, args, {
    inherit: true,
    cwd: SITE,
    timeout: 600_000,
    accountId: opts.accountId,
  });
  if (r.error || r.status !== 0) {
    console.error(`\n=> 部署失败（exit ${r.status ?? 'unknown'}）：${r.error?.message || '见上方 wrangler 输出'}`);
    console.error('   常见原因：未登录、项目名无权限、网络不通、或 Pages 项目需先在控制台创建。');
    process.exitCode = 1;
    return;
  }

  console.log('\n=> 部署命令执行完成。请从上方 wrangler 输出里记下 *.pages.dev 地址，然后：');
  console.log('   node scripts/verify-cloudflare-site.mjs https://<项目名>.pages.dev');
  console.log('   验收通过后，再在 Pages → Domains 绑定 payoncepdf.com，并在阿里云把 NS 指向 Cloudflare。');
}

main();
