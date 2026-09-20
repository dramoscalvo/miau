// Read-only TypeScript module extraction. No project code or compiler plugins are executed.
const fs = require('node:fs');
const path = require('node:path');
const { createHash } = require('node:crypto');
const { builtinModules, createRequire } = require('node:module');

const MODULE_VERSION = 1;
const TYPE_VERSION = 2;
const compare = (a, b) => a < b ? -1 : a > b ? 1 : 0;
const slash = value => value.split(path.sep).join('/');
const digest = value => createHash('sha256').update(value).digest('hex');

function extractModules(ts, { root, config, title = 'TypeScript module dependencies' }) {
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
    extractor: 'miau-typescript', version: MODULE_VERSION, typescript: ts.version,
    project: configName, scope: 'module-dependencies',
    fingerprint: digest(JSON.stringify({ version: MODULE_VERSION, typescript: ts.version, project: configName, inputs })),
    warnings: [...warnings].sort(compare),
  };
  const graph = { version: 1, title, status: 'observed', provenance,
    nodes: [...nodes.values()].sort((a, b) => compare(a.id, b.id)),
    edges: edges.sort((a, b) => compare(a.from, b.from) || compare(a.to, b.to) ||
      a.source.line - b.source.line || compare(a.label, b.label)) };
  if (Buffer.byteLength(JSON.stringify(graph, null, 2)) > 1048576) throw new Error('Diagram exceeds 1 MiB');
  return graph;
}

function extractTypes(ts, { root, config, title = 'TypeScript type relationships' }) {
  const [major, minor] = ts.version.split('.').map(Number);
  if (!(major === 6 || (major === 5 && minor >= 6))) {
    throw new Error('The extractor requires the TypeScript 5.6–6.x JavaScript compiler API');
  }
  root = fs.realpathSync(root);
  config = fs.realpathSync(config);
  const relative = file => {
    file = fs.realpathSync(file);
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
  function trackedRead(file) {
    file = path.resolve(file);
    if (contents.has(file)) return contents.get(file);
    const text = ts.sys.readFile(file);
    contents.set(file, text);
    if (text !== undefined) {
      const name = slash(path.relative(root, file));
      const modules = slash(file).lastIndexOf('/node_modules/');
      const identity = name.startsWith('../') && modules >= 0
        ? `dependency:${slash(file).slice(modules + 14)}` : relative(file);
      reads.set(identity, digest(text));
    }
    return text;
  }
  const configHost = {
    ...ts.sys,
    getCurrentDirectory: () => root,
    readFile: trackedRead,
    onUnRecoverableConfigFileDiagnostic(diagnostic) {
      throw new Error(ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n'));
    },
    trace() {},
  };
  const parsed = ts.getParsedCommandLineOfConfigFile(config, {}, configHost);
  if (!parsed) throw new Error('Cannot read tsconfig');
  if (parsed.projectReferences?.length) {
    throw new Error('Project references are not supported yet; select a leaf tsconfig');
  }
  if (parsed.errors.length) {
    throw new Error(parsed.errors.map(diagnostic => ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')).join('\n'));
  }
  if (!parsed.fileNames.length) throw new Error('No source files in the selected tsconfig');

  const compilerHost = ts.createCompilerHost(parsed.options, true);
  compilerHost.getCurrentDirectory = () => root;
  compilerHost.readFile = trackedRead;
  const compilerGetSourceFile = compilerHost.getSourceFile.bind(compilerHost);
  compilerHost.getSourceFile = (fileName, ...args) => {
    trackedRead(fileName);
    return compilerGetSourceFile(fileName, ...args);
  };
  const program = ts.createProgram({ rootNames: parsed.fileNames, options: parsed.options, host: compilerHost });
  const checker = program.getTypeChecker();
  const syntaxErrors = program.getSyntacticDiagnostics();
  if (syntaxErrors.length) {
    const diagnostic = syntaxErrors.slice().sort((a, b) => {
      const file = compare(a.file?.fileName ?? '', b.file?.fileName ?? '');
      return file || (a.start ?? 0) - (b.start ?? 0);
    })[0];
    const location = diagnostic.file && diagnostic.start !== undefined
      ? `${relative(diagnostic.file.fileName)}:${diagnostic.file.getLineAndCharacterOfPosition(diagnostic.start).line + 1}: ` : '';
    throw new Error(`Syntax error in ${location}${ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n')}`);
  }

  const nodes = new Map();
  const semanticBySymbol = new Map();
  const semanticById = new Map();
  const declarations = [];
  const warnings = new Set();
  function isProjectFile(file) {
    file = fs.realpathSync(file);
    const name = slash(path.relative(root, file));
    return name && name !== '..' && !name.startsWith('../') && !path.isAbsolute(name)
      && !name.split('/').includes('node_modules');
  }
  function addNode(node) {
    nodes.set(node.id, node);
    if (nodes.size > 1000) throw new Error('Diagram exceeds 1000 nodes; select a smaller tsconfig');
  }
  function fileNode(file) {
    const name = relative(file);
    const segments = name.split('/');
    for (let index = 1; index < segments.length; index++) {
      const directory = segments.slice(0, index).join('/');
      addNode({ id: `dir:${directory}`, label: segments[index - 1], kind: 'directory',
        ...(index > 1 ? { parent: `dir:${segments.slice(0, index - 1).join('/')}` } : {}) });
    }
    const id = `file:${name}`;
    addNode({ id, label: segments.at(-1), kind: 'module',
      ...(segments.length > 1 ? { parent: `dir:${segments.slice(0, -1).join('/')}` } : {}),
      source: { file: name, line: 1 } });
    return id;
  }
  const sourceFiles = program.getSourceFiles()
    .filter(source => {
      if (isProjectFile(source.fileName)) return true;
      if (!program.isSourceFileFromExternalLibrary(source) && !program.isSourceFileDefaultLibrary(source)) {
        throw new Error(`Source file is outside the project root: ${source.fileName}`);
      }
      return false;
    })
    .sort((a, b) => compare(relative(a.fileName), relative(b.fileName)));
  for (const source of sourceFiles) fileNode(source.fileName);

  function declarationKind(node) {
    if (ts.isClassDeclaration(node)) {
      return node.modifiers?.some(modifier => modifier.kind === ts.SyntaxKind.AbstractKeyword)
        ? 'abstract-class' : 'class';
    }
    if (ts.isInterfaceDeclaration(node)) return 'interface';
    if (ts.isEnumDeclaration(node)) return 'enum';
    return undefined;
  }
  for (const source of sourceFiles) {
    const file = relative(source.fileName);
    function discover(node) {
      const kind = declarationKind(node);
      if (kind) {
        const line = source.getLineAndCharacterOfPosition(node.getStart(source)).line + 1;
        if (node.parent !== source) {
          warnings.add(`${file}:${line}: nested semantic declaration omitted`);
        } else if (!node.name) {
          warnings.add(`${file}:${line}: anonymous semantic declaration omitted`);
        } else {
          const symbol = checker.getSymbolAtLocation(node.name);
          if (!symbol) {
            warnings.add(`${file}:${line}: unresolved declaration ${node.name.text} omitted`);
          } else {
            const id = `type:${file}#${node.name.text}`;
            if (semanticById.has(id) || semanticBySymbol.has(symbol)) {
              throw new Error(`${file}:${line}: declaration merging is not supported for ${node.name.text}`);
            }
            const semantic = { id, declaration: node, symbol, file, position: node.getStart(source) };
            semanticById.set(id, semantic);
            semanticBySymbol.set(symbol, semantic);
            declarations.push(semantic);
            addNode({ id, label: node.name.text, kind, parent: `file:${file}`, source: { file, line } });
          }
        }
      }
      ts.forEachChild(node, discover);
    }
    discover(source);
  }

  function unalias(symbol) {
    if (symbol && (symbol.flags & ts.SymbolFlags.Alias)) {
      try { return checker.getAliasedSymbol(symbol); }
      catch { return undefined; }
    }
    return symbol;
  }
  function targetForLocation(location) {
    return semanticBySymbol.get(unalias(checker.getSymbolAtLocation(location)));
  }
  function warning(typeNode, message) {
    const source = typeNode.getSourceFile();
    const file = relative(source.fileName);
    const line = source.getLineAndCharacterOfPosition(typeNode.getStart(source)).line + 1;
    warnings.add(`${file}:${line}: ${message}`);
  }
  function namedTypeText(node) {
    if (ts.isIdentifier(node)) return node.text;
    if (ts.isQualifiedName(node)) return node.right.text;
    return undefined;
  }
  function resolveTypeTargets(typeNode, options = {}) {
    if (!typeNode) return [];
    if (ts.isParenthesizedTypeNode(typeNode) || ts.isTypeOperatorNode(typeNode)) {
      return resolveTypeTargets(typeNode.type, options);
    }
    if (ts.isUnionTypeNode(typeNode)) {
      return uniqueTargets(typeNode.types.flatMap(member => resolveTypeTargets(member, options)));
    }
    if (ts.isArrayTypeNode(typeNode)) return resolveTypeTargets(typeNode.elementType, options);
    if (!ts.isTypeReferenceNode(typeNode)) return [];
    const direct = targetForLocation(typeNode.typeName);
    if (direct) return [direct];
    const symbol = unalias(checker.getSymbolAtLocation(typeNode.typeName));
    if (symbol?.flags & ts.SymbolFlags.TypeParameter) return [];
    const name = namedTypeText(typeNode.typeName);
    if ((name === 'Array' || name === 'ReadonlyArray' || (options.unwrapPromise && name === 'Promise'))
      && typeNode.typeArguments?.length) {
      return resolveTypeTargets(typeNode.typeArguments[0], options);
    }
    const declarations = symbol?.declarations ?? [];
    if (name && !['string', 'number', 'boolean', 'bigint', 'symbol', 'unknown', 'any', 'never', 'void', 'null', 'undefined'].includes(name)) {
      if (!symbol || declarations.length === 0) {
        warning(typeNode, `unresolved type ${name} omitted`);
      } else if (declarations.some(declaration => isProjectFile(declaration.getSourceFile().fileName))) {
        warning(typeNode, `unsupported project type ${name} omitted`);
      }
    }
    return [];
  }
  function uniqueTargets(targets) {
    return [...new Map(targets.map(target => [target.id, target])).values()];
  }
  function inferredPropertyTargets(node) {
    const type = checker.getTypeAtLocation(node);
    const symbol = unalias(type.aliasSymbol ?? type.symbol);
    const target = semanticBySymbol.get(symbol);
    return target ? [target] : [];
  }
  function hasModifier(node, kind) {
    return node.modifiers?.some(modifier => modifier.kind === kind) ?? false;
  }
  function isParameterProperty(parameter, constructor) {
    if (typeof ts.isParameterPropertyDeclaration === 'function') {
      return ts.isParameterPropertyDeclaration(parameter, constructor);
    }
    return parameter.modifiers?.some(modifier => [
      ts.SyntaxKind.PublicKeyword, ts.SyntaxKind.ProtectedKeyword, ts.SyntaxKind.PrivateKeyword,
      ts.SyntaxKind.ReadonlyKeyword,
    ].includes(modifier.kind)) ?? false;
  }

  const relations = new Map();
  function addRelation(from, target, kind, evidence) {
    if (!target) return;
    const source = evidence.getSourceFile();
    const candidate = {
      from: from.id, to: target.id, kind,
      source: { file: relative(source.fileName), line: source.getLineAndCharacterOfPosition(evidence.getStart(source)).line + 1 },
      position: evidence.getStart(source),
    };
    const key = `${candidate.from}\0${candidate.to}\0${candidate.kind}`;
    const current = relations.get(key);
    if (!current || compare(candidate.source.file, current.source.file) < 0
      || (candidate.source.file === current.source.file && candidate.position < current.position)) {
      relations.set(key, candidate);
    }
    if (relations.size > 5000) throw new Error('Diagram exceeds 5000 edges');
  }
  function addTypeRelations(from, typeNode, kind, evidence = typeNode, options) {
    for (const target of resolveTypeTargets(typeNode, options)) addRelation(from, target, kind, evidence);
  }
  for (const semantic of declarations.sort((a, b) => compare(a.id, b.id))) {
    const declaration = semantic.declaration;
    for (const clause of declaration.heritageClauses ?? []) {
      const kind = clause.token === ts.SyntaxKind.ImplementsKeyword ? 'implements' : 'inheritance';
      for (const type of clause.types) {
        const target = targetForLocation(type.expression);
        if (target) addRelation(semantic, target, kind, type);
        else {
          const symbol = unalias(checker.getSymbolAtLocation(type.expression));
          const symbolDeclarations = symbol?.declarations ?? [];
          if (!symbol || symbolDeclarations.length === 0) {
            warning(type, 'unresolved heritage type omitted');
          } else if (symbolDeclarations.some(item => isProjectFile(item.getSourceFile().fileName))) {
            warning(type, 'unsupported project heritage type omitted');
          }
        }
      }
    }
    for (const member of declaration.members ?? []) {
      if ((ts.isPropertyDeclaration(member) || ts.isPropertySignature(member))
        && !hasModifier(member, ts.SyntaxKind.StaticKeyword)) {
        const targets = member.type ? resolveTypeTargets(member.type) : inferredPropertyTargets(member);
        for (const target of targets) addRelation(semantic, target, 'association', member.type ?? member);
      } else if (ts.isConstructorDeclaration(member)) {
        for (const parameter of member.parameters) {
          const kind = isParameterProperty(parameter, member) ? 'association' : 'dependency';
          addTypeRelations(semantic, parameter.type, kind, parameter.type ?? parameter);
        }
      } else if (ts.isMethodDeclaration(member) || ts.isMethodSignature(member)
        || ts.isConstructSignatureDeclaration(member)) {
        for (const parameter of member.parameters) {
          addTypeRelations(semantic, parameter.type, 'dependency', parameter.type ?? parameter);
        }
        addTypeRelations(semantic, member.type, 'dependency', member.type ?? member, { unwrapPromise: true });
      }
    }
  }

  const inputs = [...reads].sort(([a], [b]) => compare(a, b));
  const provenance = {
    extractor: 'miau-typescript', version: TYPE_VERSION, typescript: ts.version,
    project: configName, scope: 'type-relations',
    fingerprint: digest(JSON.stringify({ version: TYPE_VERSION, scope: 'type-relations', typescript: ts.version, project: configName, inputs })),
    warnings: [...warnings].sort(compare),
  };
  const edges = [...relations.values()]
    .sort((a, b) => compare(a.from, b.from) || compare(a.to, b.to) || compare(a.kind, b.kind)
      || compare(a.source.file, b.source.file) || a.source.line - b.source.line || a.position - b.position)
    .map(({ position: _, ...edge }) => edge);
  const graph = { version: 2, title, status: 'observed', provenance,
    nodes: [...nodes.values()].sort((a, b) => compare(a.id, b.id)), edges };
  if (Buffer.byteLength(JSON.stringify(graph, null, 2)) > 1048576) throw new Error('Diagram exceeds 1 MiB');
  return graph;
}

function extract(ts, options) {
  const scope = options.scope ?? 'modules';
  if (scope === 'modules') return extractModules(ts, options);
  if (scope === 'types') return extractTypes(ts, options);
  throw new Error(`Unsupported TypeScript extraction scope: ${scope}`);
}

module.exports = { extract };

// Rust embeds this source and invokes node -e, so the installed binary needs no checkout.
if (process.env.MIAU_TYPESCRIPT_EXTRACT === '1') {
  try {
    const [config, root, title, scope = 'modules'] = process.argv.slice(1);
    const projectRequire = createRequire(path.join(path.dirname(config), 'package.json'));
    let ts;
    try { ts = projectRequire('typescript'); }
    catch { throw new Error('TypeScript is not installed for this project. Install its locked dependencies first.'); }
    process.stdout.write(JSON.stringify(extract(ts, { config, root, title, scope })));
  } catch (error) {
    process.stderr.write(`TypeScript extraction failed: ${error.message}\n`);
    process.exitCode = 1;
  }
}
