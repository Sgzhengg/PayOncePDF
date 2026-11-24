#!/usr/bin/env node
/**
 * deploy-cloudflare-site.mjs — PayOncePDF 官网一键部署（零依赖，Node 22+）
 *
 *   node scripts/deploy-cloudflare-site.mjs             # 只做部署前检查（默认，不会上线）
 *   node scripts/deploy-cloudflare-site.mjs --deploy    # 检查通过后执行 wrangler pages deploy
 *   node scripts/deploy-cloudflare-site.mjs --help
 *
 * 安全边界：
 *   - 默认不部署；只有显式 --deploy 才会调用 wrangler。
 *   - 不打开浏览器、不自动执行 wrangler login、不读写 Cloudflare 凭据文件、
 *     不绑定自定义域、不修改 DNS。这些必须由你在控制台完成。
 *   - 部署前强制跑 scripts/verify-cloudflare-site.mjs 的本地文件门禁，失败即中止。
 *
 * 前置条件（本机当前均未满足）：
 *   1. npx wrangler login                # 浏览器授权 Cloudflare 账号
 *   2. 域名 payoncepdf.com 的 NS 已指向 Cloudflare（阿里云万网控制台操作）
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
  const out = { deploy: false, project: 'payoncepdf', branch: 'main', help: false };
  for (let i = 0; i < argv.length; i += 1) {
    const a = argv[i];
    if (a === '--deploy') out.deploy = true;
    else if (a === '--check') out.deploy = false;
    else if (a === '--project') out.project = argv[++i] || out.project;
    else if (a === '--branch') out.branch = argv[++i] || out.branch;
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

/** 只读粗检：是否存在 Wrangler 登录凭据文件。精确判定靠 whoami。 */
function credentialHint() {
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
  return hit ? `发现凭据文件 ${hit}（以 whoami 为准）` : '未检测到 Wrangler 登录凭据';
}

function spawnWrangler(w, args, { inherit = false, cwd = ROOT, timeout } = {}) {
  const env = {
    ...process.env,
    WRANGLER_SEND_METRICS: 'false',
    NO_UPDATE_NOTIFIER: '1',
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

function printNextSteps(w, opts) {
  console.log('\n下一步（需要你本人操作，脚本不会代做）：');
  console.log('  1. npx wrangler login                         # 浏览器授权 Cloudflare 账号');
  console.log('  2. node scripts/deploy-cloudflare-site.mjs --deploy');
  console.log(`     → 实际执行：${w.cmd} ${[...w.prefix, 'pages', 'deploy', '.', '--project-name', opts.project, '--branch', opts.branch].join(' ')}`);
  console.log('     （工作目录 site-cloudflare/，首次部署会返回 *.pages.dev 地址）');
  console.log('  3. node scripts/verify-cloudflare-site.mjs https://<项目名>.pages.dev');
  console.log('  4. Cloudflare Pages → Settings → Domains 绑定 payoncepdf.com；');
  console.log('     阿里云万网把域名 NS 改为 Cloudflare 分配的两个 NS，等待 Universal SSL 变 Active。');
  console.log('\n提示：本脚本不绑定自定义域、不修改 DNS、不触碰你的 Cloudflare 凭据。');
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
    console.log('用法: node scripts/deploy-cloudflare-site.mjs [--check|--deploy] [--project 名称] [--branch 分支]');
    console.log('默认 --check：只做门禁检查与状态报告，不部署。');
    console.log('--deploy：门禁通过后执行 wrangler pages deploy（需已 wrangler login）。');
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
  console.log('\n=== 环境状态（只读）===');
  console.log(`  Wrangler        : ${w.source}`);
  console.log(`  Cloudflare 登录 : ${credentialHint()}`);
  console.log(`  部署目录        : ${SITE}`);
  console.log(`  项目 / 分支     : ${opts.project} / ${opts.branch}`);

  if (!opts.deploy) {
    console.log('\n=> 当前为 --check 模式，未执行任何部署、未发起任何网络写操作。');
    printNextSteps(w, opts);
    return;
  }

  // --- 以下只有显式 --deploy 才会执行 ---
  console.log('\n=== 检查 Cloudflare 登录状态（wrangler whoami）===');
  const who = spawnWrangler(w, ['whoami'], { timeout: 120_000 });
  const whoOut = `${who.stdout || ''}${who.stderr || ''}`;
  if (who.error || who.status !== 0 || /not logged in|not authenticated/i.test(whoOut)) {
    console.error('  未登录或无法验证登录状态。请先在本机执行：');
    console.error('    npx wrangler login');
    console.error('  授权完成后重新运行 --deploy。脚本不会自动打开浏览器或写入凭据。');
    if (whoOut.trim()) console.error(`  whoami 输出：${whoOut.trim().split('\n').slice(0, 4).join(' | ')}`);
    process.exitCode = 1;
    return;
  }
  console.log('  已登录 Cloudflare。');

  const args = ['pages', 'deploy', '.', '--project-name', opts.project, '--branch', opts.branch];
  console.log(`\n=== 部署: ${w.cmd} ${[...w.prefix, ...args].join(' ')} ===`);
  const r = spawnWrangler(w, args, { inherit: true, cwd: SITE, timeout: 600_000 });
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
