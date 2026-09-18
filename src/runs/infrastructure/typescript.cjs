// Read-only TypeScript module extraction. No project code or compiler plugins are executed.
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { builtinModules, createRequire } = require('node:module');

const VERSION = 1;
const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
const slash = value => value.split(path.sep).join('/');
const digest = value => createHash('sha256').update(value).digest('hex');

function extract(ts, { root, config, title = 'TypeScript module dependencies' }) {
  const [major, minor] = ts.version.split('.').map(Number);
  if (!(major === 6 || (major === 5 && minor >= 6)) || typeof ts.getImpliedNodeFormatForFile !== 'function') {
    throw new Error('The extractor requires the TypeScript 5.6–6.x JavaScript compiler API');
  }
  root = fs.realpathSync(root);
  config = fs.realpathSync(config);
  const relative = file => {
    const name = slash(path.relative(root, file));
    if (!name || name === '..' || name.startsWith('../') || path.isAbsolute(name)) {
      throw new Error(`File is outside the project root: ${file}`);
    }
    if (/[\\:\x00-\x1f\x7f]/.test(name)) throw new Error(`Unsupported source path: ${name}`);
    return name;
  };
  const configName = relative(config);
  const reads = new Map();
  const contents = new Map();
  const host = {
    ...ts.sys,
    getCurrentDirectory: () => root,
    readFile(file) {
      file = path.resolve(file);
      if (contents.has(file)) return contents.get(file);
      const text = ts.sys.readFile(file);
      contents.set(file, text);
      if (text !== undefined) {
        const name = slash(path.relative(root, file));
        // Hoisted dependencies use installation-independent identities.
        const modules = slash(file).lastIndexOf('/node_modules/');
        const identity = name.startsWith('../') && modules >= 0
          ? `dependency:${slash(file).slice(modules + 14)}` : relative(file);
        reads.set(identity, digest(text));
      }
      return text;
    },
    onUnRecoverableConfigFileDiagnostic(diagnostic) {
      throw new Error(ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'));
    },
    // traceResolution in a project's config must not corrupt the JSON protocol.
    trace() {},
  };
  const parsed = ts.getParsedCommandLineOfConfigFile(config, {}, host);
  if (!parsed) throw new Error('Cannot read tsconfig');
  if (parsed.projectReferences?.length) {
    throw new Error('Project references are not supported yet; select a leaf tsconfig');
  }
  if (parsed.errors.length) {
    throw new Error(parsed.errors.map(d => ts.flattenDiagnosticMessageText(d.messageText, '\n')).join('\n'));
  }
  if (!parsed.fileNames.length) throw new Error('No source files in the selected tsconfig');

  const nodes = new Map();
  const edges = [];
  const warnings = new Set();
  const queue = [...parsed.fileNames].sort(compare);
  const visited = new Set();
  const cache = ts.createModuleResolutionCache(root, file => file, parsed.options);
  const builtins = new Set(builtinModules.map(name => name.replace(/^node:/, '')));

  function addNode(node) {
    nodes.set(node.id, node);
    if (nodes.size > 1000) throw new Error('Diagram exceeds 1000 nodes; select a smaller tsconfig');
  }
  function fileNode(file) {
    const name = relative(file);
    const segments = name.split('/');
    for (let i = 1; i < segments.length; i++) {
      const dir = segments.slice(0, i).join('/');
      addNode({ id: `dir:${dir}`, label: segments[i - 1],
        ...(i > 1 ? { parent: `dir:${segments.slice(0, i - 1).join('/')}` } : {}) });
    }
    const id = `file:${name}`;
    addNode({ id, label: segments.at(-1),
      ...(segments.length > 1 ? { parent: `dir:${segments.slice(0, -1).join('/')}` } : {}),
      source: { file: name, line: 1 } });
    return id;
  }

  while (queue.length) {
    const file = fs.realpathSync(queue.shift());
    if (visited.has(file)) continue;
    visited.add(file);
    const name = relative(file);
    if (!/\.(?:[cm]?ts|tsx|[cm]?js|jsx|json)$/.test(file)) {
      throw new Error(`Unsupported source file: ${name}`);
    }
    const from = fileNode(file);
    const text = host.readFile(file);
    if (text === undefined) throw new Error(`Missing source: ${name}`);
    if (file.endsWith('.json')) continue;
    const source = ts.createSourceFile(file, text, parsed.options.target ?? ts.ScriptTarget.Latest, true);
    source.impliedNodeFormat = ts.getImpliedNodeFormatForFile(file, undefined, host, parsed.options);
    if (source.parseDiagnostics.length) {
      throw new Error(`Syntax error in ${name}: ${ts.flattenDiagnosticMessageText(source.parseDiagnostics[0].messageText, '\n')}`);
    }
    const lineOf = node => source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;

    function dependency(specifier, label, origin) {
      const line = lineOf(origin);
      if (!specifier || !ts.isStringLiteralLike(specifier)) {
        warnings.add(`${name}:${line}: nonliteral import omitted`);
        return;
      }
      const requested = specifier.text;
      let to;
      const mode = ts.getModeForUsageLocation(source, specifier, parsed.options);
      const resolved = ts.resolveModuleName(requested, file, parsed.options, host, cache, undefined, mode).resolvedModule;
      if (resolved?.isExternalLibraryImport) {
        // Keep package identities, without pretending declaration files are project implementation.
        const packageName = resolved.packageId?.name ?? (requested.startsWith('@')
          ? requested.split('/').slice(0, 2).join('/') : requested.split('/')[0]);
        to = `external:${packageName}`;
        addNode({ id: to, label: packageName });
      } else if (resolved) {
        const target = fs.realpathSync(resolved.resolvedFileName);
        to = fileNode(target);
        queue.push(target);
      } else if (builtins.has(requested.replace(/^node:/, ''))) {
        const name = `node:${requested.replace(/^node:/, '')}`;
        to = `external:${name}`;
        addNode({ id: to, label: name });
      } else {
        throw new Error(`${name}:${line}: unresolved import ${JSON.stringify(requested)}`);
      }
      edges.push({ from, to, kind: 'dependency', label, source: { file: name, line } });
      if (edges.length > 5000) throw new Error('Diagram exceeds 5000 edges');
    }

    function visit(node) {
      if (ts.isImportDeclaration(node)) {
        const clause = node.importClause;
        const elements = clause?.namedBindings && ts.isNamedImports(clause.namedBindings) ? clause.namedBindings.elements : [];
        const typeOnly = clause?.isTypeOnly || (!clause?.name && elements.length > 0 && elements.every(e => e.isTypeOnly));
        dependency(node.moduleSpecifier, typeOnly ? 'type import' : 'import', node);
      } else if (ts.isExportDeclaration(node) && node.moduleSpecifier) {
        dependency(node.moduleSpecifier, node.isTypeOnly ? 'type re-export' : 're-export', node);
      } else if (ts.isImportEqualsDeclaration(node) && ts.isExternalModuleReference(node.moduleReference)) {
        dependency(node.moduleReference.expression, node.isTypeOnly ? 'type import' : 'import equals', node);
      } else if (ts.isImportTypeNode(node) && ts.isLiteralTypeNode(node.argument)) {
        dependency(node.argument.literal, 'type import', node);
      } else if (ts.isCallExpression(node) && node.expression.kind === ts.SyntaxKind.ImportKeyword) {
        dependency(node.arguments[0], 'dynamic import', node);
      } else if (ts.isCallExpression(node) && ts.isIdentifier(node.expression) && node.expression.text === 'require') {
        warnings.add(`${name}:${lineOf(node)}: CommonJS require call omitted (binding not analyzed)`);
      }
      ts.forEachChild(node, visit);
    }
    visit(source);
    if (source.referencedFiles.length || source.typeReferenceDirectives.length) {
      warnings.add(`${name}: triple-slash references omitted`);
    }
  }

  const inputs = [...reads].sort(([a], [b]) => compare(a, b));
  const provenance = {
    extractor: 'miau-typescript', version: VERSION, typescript: ts.version,
    project: configName, scope: 'module-dependencies',
    fingerprint: digest(JSON.stringify({ version: VERSION, typescript: ts.version, project: configName, inputs })),
    warnings: [...warnings].sort(compare),
  };
  const graph = { version: 1, title, status: 'observed', provenance,
    nodes: [...nodes.values()].sort((a, b) => compare(a.id, b.id)),
    edges: edges.sort((a, b) => compare(a.from, b.from) || compare(a.to, b.to) ||
      a.source.line - b.source.line || compare(a.label, b.label)) };
  if (Buffer.byteLength(JSON.stringify(graph, null, 2)) > 1048576) throw new Error('Diagram exceeds 1 MiB');
  return graph;
}

module.exports = { extract };

// Rust embeds this source and invokes node -e, so the installed binary needs no checkout.
if (process.env.MIAU_TYPESCRIPT_EXTRACT === '1') {
  try {
    const [config, root, title] = process.argv.slice(1);
    const projectRequire = createRequire(path.join(path.dirname(config), 'package.json'));
    let ts;
    try { ts = projectRequire('typescript'); }
    catch { throw new Error('TypeScript is not installed for this project. Install its locked dependencies first.'); }
    process.stdout.write(JSON.stringify(extract(ts, { config, root, title })));
  } catch (error) {
    process.stderr.write(`TypeScript extraction failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
