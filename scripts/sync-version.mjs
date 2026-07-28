// 把 package.json 的 version 同步进 src-tauri/tauri.conf.json，
// 让 package.json 成为打包/发版的单一版本来源。
// 仅在版本不一致时才写回，避免每次 dev/build 都产生无谓的 git 改动。
import { readFileSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const pkg = JSON.parse(readFileSync(resolve(root, 'package.json'), 'utf-8'));
const confPath = resolve(root, 'src-tauri', 'tauri.conf.json');
const conf = JSON.parse(readFileSync(confPath, 'utf-8'));

if (conf.version !== pkg.version) {
  conf.version = pkg.version;
  writeFileSync(confPath, JSON.stringify(conf, null, 2) + '\n', 'utf-8');
  console.log(`[sync-version] tauri.conf.json version -> ${pkg.version}`);
} else {
  console.log(`[sync-version] version 已是最新 (${pkg.version})`);
}
