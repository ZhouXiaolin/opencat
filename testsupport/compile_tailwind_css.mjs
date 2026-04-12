import fs from 'node:fs/promises';
import process from 'node:process';
import { compile } from '@tailwindcss/node';

const fixturePath = process.argv[2];

if (!fixturePath) {
  console.error('usage: node compile_tailwind_css.mjs <fixture.json>');
  process.exit(1);
}

const fixture = JSON.parse(await fs.readFile(fixturePath, 'utf8'));
const candidates = Array.from(new Set(fixture.candidates ?? []));
const compiler = await compile('@import "tailwindcss";', {
  base: process.cwd(),
  onDependency() {},
});

process.stdout.write(compiler.build(candidates));
