const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

test('installed Rust command runs the real compiler and emits a reproducible Markdown artifact', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'miau-ts-cli-real-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, 'node_modules'));
  fs.symlinkSync(path.resolve(__dirname, '../tools/typescript-extractor/node_modules/typescript'),
    path.join(root, 'node_modules/typescript'), process.platform === 'win32' ? 'junction' : 'dir');
  fs.writeFileSync(path.join(root, 'tsconfig.json'), '{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler"},"files":["a.ts","b.ts"]}');
  fs.writeFileSync(path.join(root, 'a.ts'), 'export { b } from "./b";\n');
  fs.writeFileSync(path.join(root, 'b.ts'), 'export const b = 1;\n');
  const binary = process.env.MIAU_TEST_BIN || path.resolve(__dirname, '../target/debug/miau');
  const run = (...args) => spawnSync(binary, ['diagram', 'typescript', '--root', '.', '--project', 'tsconfig.json', ...args], { cwd: root, encoding: 'utf8' });
  const first = run();
  assert.equal(first.status, 0, first.stderr);
  assert(first.stdout.startsWith('# Review\n'));
  assert.equal(run().stdout, first.stdout);
  const graph = JSON.parse(first.stdout.split('```miau-graph\n')[1].split('\n```')[0]);
  assert.equal(graph.edges[0].to, 'file:b.ts');
  assert.equal(graph.provenance.extractor, 'miau-typescript');
  assert(!first.stdout.includes(root));
  assert.equal(run('--output', 'diagram.md').status, 0);
  assert.notEqual(run('--output', 'diagram.md').status, 0);
  assert.equal(fs.readFileSync(path.join(root, 'diagram.md'), 'utf8'), first.stdout);
  fs.writeFileSync(path.join(root, 'a.ts'), 'import "./missing";');
  const broken = run('--output', 'failed.md');
  assert.notEqual(broken.status, 0);
  assert.match(broken.stderr, /unresolved import/);
  assert.equal(broken.stdout, '');
  assert(!fs.existsSync(path.join(root, 'failed.md')));
  assert(!fs.existsSync(path.join(root, 'miaus')));
});

test('module scope is the default and type scope selects semantic extraction', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'miau-ts-cli-scope-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, 'node_modules'));
  fs.symlinkSync(path.resolve(__dirname, '../tools/typescript-extractor/node_modules/typescript'),
    path.join(root, 'node_modules/typescript'), process.platform === 'win32' ? 'junction' : 'dir');
  fs.writeFileSync(path.join(root, 'tsconfig.json'), '{"compilerOptions":{"module":"ESNext","moduleResolution":"Bundler"},"files":["a.ts"]}');
  fs.writeFileSync(path.join(root, 'a.ts'), 'export interface Port {}\nexport class Adapter implements Port {}\n');
  const binary = process.env.MIAU_TEST_BIN || path.resolve(__dirname, '../target/debug/miau');
  const run = (...args) => spawnSync(binary, ['diagram', 'typescript', '--root', '.', '--project', 'tsconfig.json', ...args], { cwd: root, encoding: 'utf8' });
  const implicit = run();
  const modules = run('--scope', 'modules');
  assert.equal(implicit.status, 0, implicit.stderr);
  assert.equal(modules.status, 0, modules.stderr);
  assert.equal(implicit.stdout, modules.stdout);
  const types = run('--scope', 'types');
  assert.equal(types.status, 0, types.stderr);
  const graph = JSON.parse(types.stdout.split('```miau-graph\n')[1].split('\n```')[0]);
  assert.equal(graph.version, 2);
  assert.equal(graph.provenance.scope, 'type-relations');
  assert(graph.edges.some(edge => edge.kind === 'implements'));
});
