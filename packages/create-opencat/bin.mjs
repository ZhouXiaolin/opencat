#!/usr/bin/env node
// create-opencat: 拷贝 template/ 到目标目录并改写项目名。
// 用法: node bin.mjs [project-name]   （默认 my-video）
import { cpSync, existsSync, readdirSync, readFileSync, writeFileSync } from 'node:fs';
import { basename, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const templateDir = join(dirname(fileURLToPath(import.meta.url)), 'template');
const name = process.argv[2] || 'my-video';
if (!/^[A-Za-z0-9._-]+$/.test(basename(name))) {
    console.error(`✗ 项目目录名只能含字母、数字、点、下划线、连字符：${basename(name)}`);
    process.exit(1);
}
const target = resolve(process.cwd(), name);
if (existsSync(target) && readdirSync(target).length > 0) {
    console.error(`✗ 目标目录非空：${target}`);
    process.exit(1);
}

cpSync(templateDir, target, { recursive: true });

const pkgPath = join(target, 'package.json');
const pkg = JSON.parse(readFileSync(pkgPath, 'utf8'));
pkg.name = basename(name);
writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + '\n');

console.log(`✓ 已创建 ${target}

下一步：
  cd ${name}
  bun install      # 或 npm install
  npm run build    # 编译 tailwind.css + 调 opencat render，产出 dist/video.mp4

渲染前确保 opencat 可执行文件可用：设置 OPENCAT_BIN，
或把 target/release（含 opencat.exe / opencat）加入 PATH。`);
