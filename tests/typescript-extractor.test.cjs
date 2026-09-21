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

test('type scope discovers supported declarations with canonical hierarchy and kinds', t => {
  const input = project(t, {
    'src/main.ts': [
      'export class A {}',
      'export abstract class B {}',
      'export interface C {}',
      'export enum D { One }',
      'export type Alias = string;',
    ].join('\n'),
  });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert.equal(graph.version, 2);
  assert.equal(graph.provenance.scope, 'type-relations');
  assert.deepEqual(
    graph.nodes.filter(node => node.id.startsWith('type:src/main.ts#')).map(node => [node.id, node.kind, node.parent, node.source.line]),
    [
      ['type:src/main.ts#A', 'class', 'file:src/main.ts', 1],
      ['type:src/main.ts#B', 'abstract-class', 'file:src/main.ts', 2],
      ['type:src/main.ts#C', 'interface', 'file:src/main.ts', 3],
      ['type:src/main.ts#D', 'enum', 'file:src/main.ts', 4],
    ],
  );
  assert.equal(graph.nodes.find(node => node.id === 'file:src/main.ts').kind, 'module');
  assert(!graph.nodes.some(node => node.label === 'Alias'));
});

test('type scope resolves aliased heritage symbols and duplicate names canonically', t => {
  const input = project(t, {
    'src/a/User.ts': 'export class User {}\nexport interface Port {}\n',
    'src/b/User.ts': 'export class User {}\nexport interface Other {}\n',
    'src/main.ts': [
      'import { User as Customer, Port as Contract } from "./a/User";',
      'import { User as OtherUser, Other } from "./b/User";',
      'export class Service extends Customer implements Contract, Other {',
      '  peer: OtherUser;',
      '}',
    ].join('\n'),
  });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert.deepEqual(graph.edges.map(edge => [edge.from, edge.to, edge.kind]), [
    ['type:src/main.ts#Service', 'type:src/a/User.ts#Port', 'implements'],
    ['type:src/main.ts#Service', 'type:src/a/User.ts#User', 'inheritance'],
    ['type:src/main.ts#Service', 'type:src/b/User.ts#Other', 'implements'],
    ['type:src/main.ts#Service', 'type:src/b/User.ts#User', 'association'],
  ]);
  assert(!graph.nodes.some(node => node.label === 'Customer'));
});

test('qualified namespace references resolve to the underlying declaration', t => {
  const input = project(t, {
    'src/model.ts': 'export interface Model {}\n',
    'src/main.ts': 'import * as domain from "./model";\nclass Service { model: domain.Model; }\n',
  });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert(graph.edges.some(edge => edge.from === 'type:src/main.ts#Service'
    && edge.to === 'type:src/model.ts#Model' && edge.kind === 'association'));
});

test('type scope extracts class and interface inheritance with independently sourced edges', t => {
  const input = project(t, {
    'src/main.ts': [
      'class A {}',
      'class B extends A {}',
      'interface X {}',
      'interface Y extends X {}',
      'interface Z extends X, Y {}',
    ].join('\n'),
  });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert.deepEqual(
    graph.edges.filter(edge => edge.kind === 'inheritance').map(edge => [edge.from, edge.to, edge.source.line]),
    [
      ['type:src/main.ts#B', 'type:src/main.ts#A', 2],
      ['type:src/main.ts#Y', 'type:src/main.ts#X', 4],
      ['type:src/main.ts#Z', 'type:src/main.ts#X', 5],
      ['type:src/main.ts#Z', 'type:src/main.ts#Y', 5],
    ],
  );
});

test('type scope maps declared properties and operations without collapsing relation kinds', t => {
  const input = project(t, {
    'src/main.ts': [
      'interface B {}',
      'interface C {}',
      'class A {',
      '  first: B;',
      '  second?: B;',
      '  nullable: B | null;',
      '  values: B[];',
      '  constructor(private owned: B, readonly visible: B, input: B) {}',
      '  run(input: B): C { throw new Error(); }',
      '}',
    ].join('\n'),
  });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert.deepEqual(graph.edges.map(edge => [edge.from, edge.to, edge.kind]), [
    ['type:src/main.ts#A', 'type:src/main.ts#B', 'association'],
    ['type:src/main.ts#A', 'type:src/main.ts#B', 'dependency'],
    ['type:src/main.ts#A', 'type:src/main.ts#C', 'dependency'],
  ]);
});

test('type normalization is conservative for generics, wrappers, builtins and runtime expressions', t => {
  const input = project(t, {
    'src/main.ts': [
      'declare function decorator(value?: unknown): ClassDecorator;',
      'class Order {}',
      'class Result {}',
      'class Engine {}',
      'class Repository<T> {}',
      '@decorator(Order)',
      'class Decorated {}',
      'class Service<T> {',
      '  repo: Repository<Order>;',
      '  engines: ReadonlyArray<Engine>;',
      '  private readonly inferred = new Engine();',
      '  save(value: T): Promise<Result> {',
      '    const local: Order = new Order();',
      '    local.toString();',
      '    throw new Error();',
      '  }',
      '}',
    ].join('\n'),
  });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert.deepEqual(graph.edges.map(edge => [edge.from, edge.to, edge.kind]), [
    ['type:src/main.ts#Service', 'type:src/main.ts#Engine', 'association'],
    ['type:src/main.ts#Service', 'type:src/main.ts#Repository', 'association'],
    ['type:src/main.ts#Service', 'type:src/main.ts#Result', 'dependency'],
  ]);
  assert(!graph.edges.some(edge => ['aggregation', 'composition'].includes(edge.kind)));
  assert(!graph.edges.some(edge => edge.from === 'type:src/main.ts#Decorated'));
  assert(!graph.nodes.some(node => ['Promise', 'T'].includes(node.label)));
});

test('interface operations create dependencies and property ownership syntax remains association', t => {
  const input = project(t, {
    'src/main.ts': [
      'abstract class Entity {}',
      'class Engine {}',
      'interface Repository { save(entity: Entity): Promise<void>; }',
      'class Car { private readonly engine: Engine = new Engine(); }',
      'class Service { run(entity: Entity): Repository { throw new Error(); } }',
    ].join('\n'),
  });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert.deepEqual(graph.edges.map(edge => [edge.from, edge.to, edge.kind]), [
    ['type:src/main.ts#Car', 'type:src/main.ts#Engine', 'association'],
    ['type:src/main.ts#Repository', 'type:src/main.ts#Entity', 'dependency'],
    ['type:src/main.ts#Service', 'type:src/main.ts#Entity', 'dependency'],
    ['type:src/main.ts#Service', 'type:src/main.ts#Repository', 'dependency'],
  ]);
  assert(!graph.edges.some(edge => ['aggregation', 'composition'].includes(edge.kind)));
});

test('declared self relationships are retained', t => {
  const input = project(t, { 'src/main.ts': 'class LinkNode { next?: LinkNode; compare(other: LinkNode): LinkNode { return other; } }\n' });
  const graph = extract(ts, { ...input, scope: 'types' });
  assert.deepEqual(graph.edges.map(edge => [edge.from, edge.to, edge.kind]), [
    ['type:src/main.ts#LinkNode', 'type:src/main.ts#LinkNode', 'association'],
    ['type:src/main.ts#LinkNode', 'type:src/main.ts#LinkNode', 'dependency'],
  ]);
});

test('type scope is deterministic across runs and relocated checkouts', t => {
  const first = project(t, { 'src/main.ts': 'interface B {}\nclass A { value: B | undefined; }\n' });
  const second = project(t, { 'src/main.ts': 'interface B {}\nclass A { value: B | undefined; }\n' });
  const options = input => ({ ...input, scope: 'types', title: 'Types' });
  assert.equal(JSON.stringify(extract(ts, options(first))), JSON.stringify(extract(ts, options(first))));
  assert.equal(JSON.stringify(extract(ts, options(first))), JSON.stringify(extract(ts, options(second))));
  const fingerprint = extract(ts, options(first)).provenance.fingerprint;
  fs.appendFileSync(path.join(first.root, 'src/main.ts'), '// changed\n');
  assert.notEqual(extract(ts, options(first)).provenance.fingerprint, fingerprint);
});

test('impacted type scope includes changed files and directly related types only', t => {
  const input = project(t, {
    'src/changed.ts': [
      'import { Helper } from "./helper";',
      'import { Port } from "./port";',
      'export class Adapter implements Port {',
      '  value: Helper;',
      '}',
    ].join('\n'),
    'src/port.ts': 'import { Adapter } from "./changed";\nexport interface Port { adapter: Adapter; }\n',
    'src/helper.ts': 'export class Helper {}\n',
    'src/unrelated.ts': 'export class Unrelated {}\n',
  });
  const graph = extract(ts, {
    ...input,
    scope: 'types',
    changedFiles: ['src/changed.ts'],
  });

  assert.deepEqual(graph.nodes.filter(node => node.kind === 'module').map(node => node.source.file), [
    'src/changed.ts', 'src/helper.ts', 'src/port.ts',
  ]);
  assert.deepEqual(graph.edges.map(edge => [edge.from, edge.to, edge.kind]), [
    ['type:src/changed.ts#Adapter', 'type:src/helper.ts#Helper', 'association'],
    ['type:src/changed.ts#Adapter', 'type:src/port.ts#Port', 'implements'],
    ['type:src/port.ts#Port', 'type:src/changed.ts#Adapter', 'association'],
  ]);
});

test('recoverable semantic errors warn while syntax errors remain fatal', t => {
  const recoverable = project(t, { 'src/main.ts': 'class A { value: Missing; }\n' });
  const graph = extract(ts, { ...recoverable, scope: 'types' });
  assert(graph.provenance.warnings.some(warning => warning.includes('unresolved type')));
  const broken = project(t, { 'src/main.ts': 'class A { value: ; }\n' });
  assert.throws(() => extract(ts, { ...broken, scope: 'types' }), /syntax/i);
});

test('ambiguous declaration merging fails instead of guessing a semantic identity', t => {
  const input = project(t, { 'src/main.ts': 'interface A { one: string; }\ninterface A { two: string; }\n' });
  assert.throws(() => extract(ts, { ...input, scope: 'types' }), /declaration merging/i);
});

test('changed graphs apply node and edge limits after excluding unrelated types', t => {
  const input = project(t, {
    'src/main.ts': 'export class Changed {}\n',
    'src/large.ts': Array.from({ length: 1000 }, (_, i) => `export class Unrelated${i} {}`).join('\n'),
    'src/dense.ts': Array.from({ length: 72 }, (_, i) =>
      `export class Dense${i} { ${Array.from({ length: 72 }, (_, j) => `p${j}: Dense${j};`).join(' ')} }`).join('\n'),
  });
  const graph = extract(ts, { ...input, scope: 'types', changedFiles: ['src/main.ts'] });
  assert.deepEqual(graph.nodes.map(node => node.id), ['dir:src', 'file:src/main.ts', 'type:src/main.ts#Changed']);
  assert.deepEqual(graph.edges, []);
  assert.throws(() => extract(ts, { ...input, scope: 'types', changedFiles: ['src/large.ts'] }), /1000 nodes/);
  assert.throws(() => extract(ts, { ...input, scope: 'types', changedFiles: ['src/dense.ts'] }), /5000 edges/);
});

test('UML compartments include visibility, signatures, parameter properties and enum literals', t => {
  const input = project(t, { 'src/main.ts': [
    'class Service {',
    ' private name: string;',
    ' protected static count = 1;',
    ' constructor(public readonly port: Port, input: string) {}',
    ' async run(value?: number): Promise<string> { return ""; }',
    '}',
    'interface Port { readonly id: string; save(value: string): void; }',
    'enum State { Ready, Done }',
  ].join('\n') });
  const graph = extract(ts, { ...input, scope: 'types' });
  const service = graph.nodes.find(node => node.label === 'Service');
  assert.deepEqual(service.attributes, ['- name: string', '# count: number {static}', '+ port: Port {readOnly}']);
  assert.deepEqual(service.operations, ['+ constructor(port: Port, input: string)', '+ run(value?: number): Promise<string>']);
  const port = graph.nodes.find(node => node.label === 'Port');
  assert.deepEqual(port.attributes, ['+ id: string {readOnly}']);
  assert.deepEqual(port.operations, ['+ save(value: string): void']);
  assert.deepEqual(graph.nodes.find(node => node.label === 'State').attributes, ['Ready', 'Done']);
});
