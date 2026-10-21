// Actual generated-language execution against the real shared native host.
import { execFileSync, spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import { dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const fixture = dirname(fileURLToPath(import.meta.url));
const root = resolve(fixture, '../../../..');
const options = new Map();
for (let index = 2; index < process.argv.length; index += 2) {
  const key = process.argv[index], value = process.argv[index + 1];
  if (!['--out', '--kotlinc', '--jna', '--java'].includes(key) || !value || options.has(key)) throw new Error('invalid fixture arguments');
  options.set(key, resolve(value));
}
if (options.size !== 4 || !process.env.CARGO_TARGET_DIR) throw new Error('requires extbuild and --out/--kotlinc/--jna/--java');
if (process.platform !== 'darwin') throw new Error('this Swift/Kotlin lane requires native macOS');
const out = options.get('--out');
if (existsSync(out)) throw new Error('output directory must be new');
mkdirSync(out, { recursive: true });
const hash = path => createHash('sha256').update(readFileSync(path)).digest('hex');
function files(directory, prefix = '') {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const name = join(prefix, entry.name);
    if (entry.isDirectory()) return files(join(directory, entry.name), name);
    if (!entry.isFile()) throw new Error('unexpected non-file input');
    return [name];
  }).sort();
}
const git = (...args) => execFileSync('git', args, { cwd: root, encoding: 'utf8' });
const head = git('rev-parse', 'HEAD').trim();
const dirty = [...git('diff', '--name-only', '-z', 'HEAD').split('\0'), ...git('ls-files', '--others', '--exclude-standard', '-z').split('\0')].filter(Boolean);
const sourceFiles = [...new Set(['Cargo.toml', 'Cargo.lock', ...dirty, ...files(join(root, 'crates/radroots_ffi')).map(name => `crates/radroots_ffi/${name}`)])].sort().filter(name => existsSync(join(root, name)));
const sources = Object.fromEntries(sourceFiles.map(name => [name, hash(join(root, name))]));
const target = process.env.CARGO_TARGET_DIR;
const library = join(target, 'native_binding_fixture/libradroots_native_binding_fixture.dylib');
const generator = join(target, 'debug/radroots_ffi_bindgen');
const report = { state: 'running', head, dirty_paths: dirty.sort(), started_at: new Date().toISOString(), scope: 'actual native macOS Swift/Kotlin shared-client fixture; no iOS/custody/factory or consumer-adoption qualification', sources, library: { path: library, sha256: hash(library) }, generator: { path: generator, sha256: hash(generator) }, commands: [] };
function persist() { writeFileSync(join(out, 'report.json'), JSON.stringify(report, null, 2) + '\n'); }
function run(command, args, { expectedPanic = false } = {}) {
  const started = performance.now();
  const result = spawnSync(command, args, { cwd: root, encoding: 'utf8', timeout: 240_000, maxBuffer: 16 * 1024 * 1024 });
  const log = join(out, `${report.commands.length}.log`);
  writeFileSync(log, (result.stdout || '') + (result.stderr || ''));
  const output = readFileSync(log, 'utf8');
  const passed = expectedPanic
    ? !result.error && ['SIGILL', 'SIGTRAP', 'SIGABRT'].includes(result.signal) && output.includes('synthetic native binding panic probe')
    : !result.error && result.status === 0;
  report.commands.push({ command: [command, ...args], exit_code: result.status, signal: result.signal, error: result.error?.message, elapsed_seconds: (performance.now() - started) / 1000, log, sha256: hash(log), expected_plain_swift_panic: expectedPanic, passed });
  persist();
  if (!passed) throw new Error(`native fixture command failed: ${command}\n${output.slice(-6000)}`);
  console.log(JSON.stringify({ command, state: 'passed', log }));
}
try {
  run(generator, ['--version']);
  run('/usr/bin/swiftc', ['--version']);
  run(options.get('--kotlinc'), ['-version']);
  run(options.get('--java'), ['-version']);
  const swift = join(out, 'swift'), kotlin = join(out, 'kotlin');
  for (const [language, destination] of [['swift', swift], ['kotlin', kotlin]]) {
    run(generator, ['generate', '--library', library, '--language', language, '--no-format', '--out-dir', destination]);
    const repeat = destination + '_repeat';
    run(generator, ['generate', '--library', library, '--language', language, '--no-format', '--out-dir', repeat]);
    const names = files(destination);
    if (JSON.stringify(names) !== JSON.stringify(files(repeat)) || names.some(name => hash(join(destination, name)) !== hash(join(repeat, name)))) throw new Error('generation is not byte deterministic');
    report[language] = Object.fromEntries(names.map(name => [name, hash(join(destination, name))]));
  }
  const executable = join(out, 'swift_smoke');
  run('/usr/bin/swiftc', ['-swift-version', '6', '-warnings-as-errors', '-parse-as-library', '-I', swift,
    ...['radroots_ffi', 'radroots_native_binding_fixture'].flatMap(name => ['-Xcc', `-fmodule-map-file=${join(swift, name + 'FFI.modulemap')}`]),
    ...files(swift).filter(name => name.endsWith('.swift')).map(name => join(swift, name)), join(fixture, 'Smoke.swift'), library, '-o', executable]);
  run(executable, []);
  run('/bin/sh', ['-c', 'ulimit -c 0; exec "$@"', 'native-fixture-panic', executable, '--plain-panic'], { expectedPanic: true });
  const coroutine = resolve(options.get('--kotlinc'), '../../lib/kotlinx-coroutines-core-jvm.jar');
  report.runtime_jars = Object.fromEntries([options.get('--jna'), coroutine].map(path => [path, hash(path)]));
  const classpath = [options.get('--jna'), coroutine].join(':');
  const jar = join(out, 'kotlin_smoke.jar');
  run(options.get('--kotlinc'), [...files(kotlin).filter(name => name.endsWith('.kt')).map(name => join(kotlin, name)), join(fixture, 'Smoke.kt'), '-Werror', '-classpath', classpath, '-include-runtime', '-d', jar]);
  const java = ['-Duniffi.component.radroots_ffi.libraryOverride=' + library, '-Duniffi.component.radroots_native_binding_fixture.libraryOverride=' + library, '-cp', jar + ':' + classpath, 'SmokeKt'];
  run(options.get('--java'), java);
  run(options.get('--java'), [...java, '--plain-panic']);
  const currentDirty = [...git('diff', '--name-only', '-z', 'HEAD').split('\0'), ...git('ls-files', '--others', '--exclude-standard', '-z').split('\0')].filter(Boolean).sort();
  if (git('rev-parse', 'HEAD').trim() !== head || JSON.stringify(currentDirty) !== JSON.stringify(report.dirty_paths) || sourceFiles.some(name => !existsSync(join(root, name)) || hash(join(root, name)) !== sources[name]) || hash(library) !== report.library.sha256 || hash(generator) !== report.generator.sha256) throw new Error('qualification inputs changed');
  report.state = 'passed';
} catch (error) {
  report.state = 'failed'; report.failure = error.message; throw error;
} finally { persist(); }
