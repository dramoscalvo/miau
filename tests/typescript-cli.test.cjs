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

test('--changed selects edited and new files with directly related types', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'miau-ts-cli-changed-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  fs.mkdirSync(path.join(root, 'node_modules'));
  fs.symlinkSync(path.resolve(__dirname, '../tools/typescript-extractor/node_modules/typescript'),
    path.join(root, 'node_modules/typescript'), process.platform === 'win32' ? 'junction' : 'dir');
  fs.mkdirSync(path.join(root, 'src'));
  fs.writeFileSync(path.join(root, 'tsconfig.json'), JSON.stringify({
    compilerOptions: { module: 'ESNext', moduleResolution: 'Bundler' },
    include: ['src/**/*.ts'],
  }));
  fs.writeFileSync(path.join(root, 'src/changed.ts'), 'import { Port } from "./port";\nexport class Adapter implements Port {}\n');
  fs.writeFileSync(path.join(root, 'src/port.ts'), 'import { Adapter } from "./changed";\nexport interface Port { adapter: Adapter; }\n');
  fs.writeFileSync(path.join(root, 'src/helper.ts'), 'export class Helper {}\n');
  fs.writeFileSync(path.join(root, 'src/unrelated.ts'), 'export class Unrelated {}\n');
  fs.writeFileSync(path.join(root, 'src/unused.ts'), 'export class Unused {}\n');
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  git('init', '-q');
  git('config', 'user.email', 'miau-test@example.invalid');
  git('config', 'user.name', 'miau test');
  git('add', '.');
  git('commit', '-qm', 'baseline');

  fs.appendFileSync(path.join(root, 'src/changed.ts'), 'export class Added {}\n');
  fs.writeFileSync(path.join(root, 'src/new.ts'), 'import { Unrelated } from "./unrelated";\nexport class NewType { value: Unrelated; }\n');
  const binary = process.env.MIAU_TEST_BIN || path.resolve(__dirname, '../target/debug/miau');
  const run = () => spawnSync(binary, ['diagram', 'typescript', '--root', '.', '--project', 'tsconfig.json', '--scope', 'types', '--changed'], { cwd: root, encoding: 'utf8' });
  const first = run();
  assert.equal(first.status, 0, first.stderr);
  assert.equal(run().stdout, first.stdout);
  const graph = JSON.parse(first.stdout.split('```miau-graph\n')[1].split('\n```')[0]);
  assert.deepEqual(graph.nodes.filter(node => node.kind === 'module').map(node => node.source.file), [
    'src/changed.ts', 'src/new.ts', 'src/port.ts', 'src/unrelated.ts',
  ]);
  assert(graph.edges.some(edge => edge.from === 'type:src/new.ts#NewType'
    && edge.to === 'type:src/unrelated.ts#Unrelated'));
  assert(!graph.nodes.some(node => node.id === 'file:src/unused.ts'));
});

test('--changed emits a no-diagram artifact when no TypeScript files changed', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'miau-ts-cli-empty-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const initialized = spawnSync('git', ['init', '-q'], { cwd: root, encoding: 'utf8' });
  assert.equal(initialized.status, 0, initialized.stderr);
  const binary = process.env.MIAU_TEST_BIN || path.resolve(__dirname, '../target/debug/miau');
  const result = spawnSync(binary, [
    'diagram', 'typescript', '--root', '.', '--project', 'missing-tsconfig.json', '--scope', 'types', '--changed',
  ], { cwd: root, encoding: 'utf8' });

  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /No new or edited TypeScript files were found/);
  assert(!result.stdout.includes('```miau-graph'));
});

function changedProject(t, nested = false) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'miau-changed-regression-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const project = nested ? path.join(root, 'packages/app') : root;
  fs.mkdirSync(project, { recursive: true });
  fs.mkdirSync(path.join(project, 'node_modules'));
  fs.symlinkSync(path.resolve(__dirname, '../tools/typescript-extractor/node_modules/typescript'),
    path.join(project, 'node_modules/typescript'), 'dir');
  fs.writeFileSync(path.join(root, '.gitignore'), 'node_modules/\n');
  fs.writeFileSync(path.join(project, 'tsconfig.json'), '{"include":["*.ts"]}');
  const git = (...args) => {
    const result = spawnSync('git', args, { cwd: root, encoding: 'utf8' });
    assert.equal(result.status, 0, result.stderr);
  };
  git('init', '-q');
  const commit = () => {
    git('add', '.');
    git('-c', 'user.name=Test', '-c', 'user.email=test@example.invalid',
      '-c', 'commit.gpgsign=false', 'commit', '-qm', 'baseline');
  };
  const run = () => spawnSync(process.env.MIAU_TEST_BIN || path.resolve(__dirname, '../target/debug/miau'),
    ['diagram', 'typescript', '--scope', 'types', '--changed'], { cwd: project, encoding: 'utf8' });
  return { root, project, git, commit, run };
}

test('changed extraction normalizes nested project paths and excludes sibling changes', t => {
  const fixture = changedProject(t, true);
  fs.writeFileSync(path.join(fixture.project, 'a.ts'), 'export class A {}\n');
  fixture.commit();
  fs.appendFileSync(path.join(fixture.project, 'a.ts'), '// edited\n');
  fs.writeFileSync(path.join(fixture.root, 'sibling.ts'), 'export class Sibling {}\n');
  const result = fixture.run();
  assert.equal(result.status, 0, result.stderr);
  const graph = JSON.parse(result.stdout.split('```miau-graph\n')[1].split('\n```')[0]);
  assert.deepEqual(graph.nodes.filter(n => n.kind === 'module').map(n => n.source.file), ['a.ts']);
});

test('changed extraction consumes the original filename in rename records', t => {
  const fixture = changedProject(t);
  fs.writeFileSync(path.join(fixture.project, 'ab old.ts'), 'export class A {}\n');
  fixture.commit();
  fixture.git('mv', 'ab old.ts', 'new.ts');
  const result = fixture.run();
  assert.equal(result.status, 0, result.stderr);
  const graph = JSON.parse(result.stdout.split('```miau-graph\n')[1].split('\n```')[0]);
  assert.deepEqual(graph.nodes.filter(n => n.kind === 'module').map(n => n.source.file), ['new.ts']);
});

test('changed extraction reports unavailable Git scope without failing non-Git workflows', t => {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'miau-non-git-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const result = spawnSync(process.env.MIAU_TEST_BIN || path.resolve(__dirname, '../target/debug/miau'),
    ['diagram', 'typescript', '--scope', 'types', '--changed'], { cwd: root, encoding: 'utf8' });
  assert.equal(result.status, 0, result.stderr);
  assert.match(result.stdout, /not a Git worktree/);
  assert(!result.stdout.includes('```miau-graph'));
  assert(!result.stdout.includes('No new or edited'));
});

test('changed extraction still fails on Git errors inside a worktree', t => {
  const fixture = changedProject(t);
  fs.writeFileSync(path.join(fixture.root, '.git/index'), 'corrupt index');
  const result = fixture.run();
  assert.notEqual(result.status, 0);
  assert.match(result.stderr, /cannot inspect changed files/);
  assert.equal(result.stdout, '');
});
