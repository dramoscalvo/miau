const { test } = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const os = require('node:os');
const path = require('node:path');
const { extract } = require('../src/runs/infrastructure/typescript.cjs');
const ts = require('../tools/typescript-extractor/node_modules/typescript');

function project(t, extra = {}) {
  const root = fs.mkdtempSync(path.join(os.tmpdir(), 'miau-ts-'));
  t.after(() => fs.rmSync(root, { recursive: true, force: true }));
  const files = {
    'tsconfig.json': JSON.stringify({ compilerOptions: {
      target: 'ES2022', module: 'ESNext', moduleResolution: 'Bundler',
      baseUrl: '.', paths: { '@app/*': ['src/*'] }, jsx: 'preserve',
    }, include: ['src/**/*'] }),
    'src/main.ts': 'import type { Port } from "@app/port";\nexport { adapter } from "./adapter";\nconst lazy = import("./lazy");\n',
    'src/port.ts': 'export interface Port {}\n',
    'src/adapter.ts': 'import type { Port } from "./port";\nexport const adapter: Port = {};\n',
    'src/lazy.ts': 'export const lazy = 1;\n',
    ...extra,
  };
  for (const [name, text] of Object.entries(files)) {
    fs.mkdirSync(path.dirname(path.join(root, name)), { recursive: true });
    fs.writeFileSync(path.join(root, name), text);
  }
  return { root, config: path.join(root, 'tsconfig.json') };
}

test('compiler extracts imports, re-exports, aliases and literal dynamic imports with evidence', t => {
  const graph = extract(ts, project(t));
  assert.equal(graph.status, 'observed');
  assert.deepEqual(graph.edges.filter(e => e.from === 'file:src/main.ts').map(e => [e.to, e.label, e.source.line]), [
    ['file:src/adapter.ts', 're-export', 2],
    ['file:src/lazy.ts', 'dynamic import', 3],
    ['file:src/port.ts', 'type import', 1],
  ]);
  assert.equal(graph.nodes.find(n => n.id === 'file:src/port.ts').parent, 'dir:src');
  assert.equal(graph.provenance.typescript, ts.version);
});

test('identical inputs produce identical bytes across runs and checkout locations', t => {
  const first = project(t);
  const second = project(t);
  assert.equal(JSON.stringify(extract(ts, first)), JSON.stringify(extract(ts, first)));
  assert.equal(JSON.stringify(extract(ts, first)), JSON.stringify(extract(ts, second)));
  const before = extract(ts, first).provenance.fingerprint;
  fs.appendFileSync(path.join(first.root, 'src/port.ts'), '// changed\n');
  assert.notEqual(extract(ts, first).provenance.fingerprint, before);
});

test('syntax errors and unresolved imports fail instead of silently dropping edges', t => {
  for (const text of ['import x from "./missing";', 'const x = ;', 'import x from "missing-package";']) {
    const input = project(t, { 'src/main.ts': text });
    assert.throws(() => extract(ts, input), /unresolved|syntax/i);
  }
});

test('nonliteral imports and CommonJS calls are explicit limitations', t => {
  const graph = extract(ts, project(t, { 'src/main.ts': 'import(name);\nrequire("./port");\n' }));
  assert(graph.provenance.warnings.some(w => w.includes('nonliteral')));
  assert(graph.provenance.warnings.some(w => w.includes('CommonJS')));
});

test('project references fail explicitly and declared files are included', t => {
  assert.throws(() => extract(ts, project(t, { 'tsconfig.json': '{"files":[],"references":[{"path":"./child"}]}' })), /references/i);
  const input = project(t, { 'src/types.d.ts': 'export interface Extra {}' });
  assert(extract(ts, input).nodes.some(n => n.id === 'file:src/types.d.ts'));
});

test('builtins are external nodes, comments do not create dependencies', t => {
  const input = project(t, { 'src/main.ts': '// import "./not-real";\nimport fs from "node:fs";\n' });
  const graph = extract(ts, input);
  assert.equal(graph.edges.length, 2); // main builtin + adapter -> port
  assert(graph.nodes.some(n => n.id === 'external:node:fs'));
});

test('resolved dependencies outside the root are rejected', t => {
  const input = project(t);
  fs.writeFileSync(path.join(input.root, 'tsconfig.json'), '{"files":["../outside.ts"]}');
  assert.throws(() => extract(ts, input), /outside|missing/i);
});

test('compiler honors package export conditions for ESM and import-equals', t => {
  const input = project(t, {
    'tsconfig.json': '{"compilerOptions":{"module":"NodeNext","moduleResolution":"NodeNext"},"files":["src/main.mts","src/main.cts"]}',
    'src/main.mts': 'import "pkg";\n',
    'src/main.cts': 'import pkg = require("pkg");\n',
    'node_modules/pkg/package.json': JSON.stringify({ name: 'pkg', version: '1.0.0', exports: { '.': { import: './import.d.mts', require: './require.d.cts' } } }),
    'node_modules/pkg/import.d.mts': 'export const value: string;',
    'node_modules/pkg/require.d.cts': 'declare const value: string; export = value;',
  });
  const graph = extract(ts, input);
  assert.equal(graph.edges.length, 2);
  assert(graph.edges.every(e => e.to === 'external:pkg'));
  assert.deepEqual(graph.edges.map(e => e.label).sort(), ['import', 'import equals']);
});

test('extends configuration and imported files excluded from roots still resolve deterministically', t => {
  const input = project(t, {
    'base.json': '{"compilerOptions":{"baseUrl":".","paths":{"@app/*":["src/*"]}}}',
    'tsconfig.json': '{"extends":"./base.json","compilerOptions":{"module":"ESNext","moduleResolution":"Bundler"},"files":["src/main.ts"]}',
  });
  const graph = extract(ts, input);
  assert(graph.nodes.some(n => n.id === 'file:src/port.ts'));
  const before = graph.provenance.fingerprint;
  fs.appendFileSync(path.join(input.root, 'base.json'), '\n');
  assert.notEqual(extract(ts, input).provenance.fingerprint, before);
});

test('TSX and import type expressions are parsed without a language model', t => {
  const graph = extract(ts, project(t, { 'src/view.tsx': 'type P = import("./port").Port;\nexport const view = <div/>;\n' }));
  assert(graph.edges.some(e => e.from === 'file:src/view.tsx' && e.to === 'file:src/port.ts' && e.label === 'type import'));
});

test('ESM package-level await and side-effect imports are supported', t => {
  const graph = extract(ts, project(t, { 'src/main.ts': 'import "./lazy";\nawait Promise.resolve();\n' }));
  assert(graph.edges.some(e => e.from === 'file:src/main.ts' && e.to === 'file:src/lazy.ts'));
});

test('import and require resolution modes select the correct local package-import targets', t => {
  const graph = extract(ts, project(t, {
    'package.json': JSON.stringify({ type: 'module', imports: { '#target': { import: './src/esm.ts', require: './src/cjs.cts' } } }),
    'tsconfig.json': '{"compilerOptions":{"module":"NodeNext","moduleResolution":"NodeNext"},"files":["src/main.mts","src/main.cts"]}',
    'src/main.mts': 'import "#target";',
    'src/main.cts': 'import target = require("#target");',
    'src/esm.ts': 'export const value = 1;',
    'src/cjs.cts': 'export = 1;',
  }));
  assert.deepEqual(graph.edges.map(e => [e.from, e.to]), [
    ['file:src/main.cts', 'file:src/cjs.cts'],
    ['file:src/main.mts', 'file:src/esm.ts'],
  ]);
});
