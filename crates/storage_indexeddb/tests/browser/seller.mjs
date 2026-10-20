import assert from 'node:assert/strict';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {createRequire} from 'node:module';
import {resolve, sep} from 'node:path';

const [moduleRoot, packageDir, engine, previousPackageDir] = process.argv.slice(2);
assert(moduleRoot && packageDir && previousPackageDir && ['chromium','firefox','webkit'].includes(engine));
const playwright = createRequire(resolve(moduleRoot,'package.json'))('playwright');
const root = resolve(packageDir), previousRoot = resolve(previousPackageDir);
async function workflow({WorkspaceFixture}, previous) {
  const tests = [];
  const check = (condition) => { if (!condition) throw Error('seller scope assertion'); };
  const refuses = async (action, code) => {
    try { await action(); } catch (error) {
      if (String(error) !== code) throw Error('expected ' + code + ', received ' + String(error));
      return;
    }
    throw Error('expected ' + code);
  };
  const old = await previous.WorkspaceFixture.create(244);
  const oldFingerprint = old.fingerprint(), oldSelection = old.selection(), oldProof = old.proof();
  const original = Array.from(await old.admit(243, false));
  await old.close();
  const current = await WorkspaceFixture.open(244, oldFingerprint, oldSelection, oldProof);
  check(!await current.is_seller());
  check(JSON.stringify(Array.from(await current.query(243))) === JSON.stringify(original));
  check(JSON.stringify(Array.from(await current.admit(243, false))) === JSON.stringify(original));
  await current.close();
  tests.push('original published buyer preparation reopens and replays without replacement');
  const seller = await WorkspaceFixture.create_seller(245);
  check(await seller.is_seller());
  check(await seller.authenticate() === 1);
  await refuses(() => seller.wrong_proof(), 'unauthorized');
  await refuses(() => seller.admit_public(244), 'capability_unavailable');
  check((await seller.query(244)).length === 0);
  tests.push('explicit protected seller scope, exact credential and unsupported public refusal without a row');
  const fingerprint = seller.fingerprint(), selection = seller.selection(), proof = seller.proof();
  const preparation = Array.from(await seller.admit(244, false));
  await seller.close();
  const reopened = await WorkspaceFixture.open(245, fingerprint, selection, proof);
  check(await reopened.is_seller());
  check(JSON.stringify(Array.from(await reopened.query(244))) === JSON.stringify(preparation));
  tests.push('actual protected close/reopen retains seller scope and original preparation');
  reopened.set_time(2000);
  await refuses(() => reopened.authenticate(), 'grant_expired');
  reopened.set_time(1000);
  await reopened.revoke();
  await reopened.close();
  await refuses(() => WorkspaceFixture.open(245, fingerprint, selection, proof), 'grant_revoked');
  const capture = await WorkspaceFixture.open_capture(245, fingerprint, selection, proof);
  await refuses(() => capture.authenticate(), 'unauthorized');
  await capture.close();
  tests.push('expiry and persisted revocation refuse normal reopen and access');
  const buyer = await WorkspaceFixture.create(246);
  check(!await buyer.is_seller());
  check(await buyer.authenticate() === 1);
  await buyer.close();
  tests.push('independent buyer enrollment retains buyer scope');
  return tests;
}
const source = `import init, * as fixture from '/fixture.js'; import previousInit, * as previous from '/before/fixture.js';
const run = ${workflow.toString()};
self.onmessage = async () => { try { await init(); await previousInit(); self.postMessage({tests:await run(fixture, previous)}); }
catch(error) { self.postMessage({error:String(error)}); } };`;
const server = createServer(async (req,res) => {
  try {
    const name = new URL(req.url,'http://localhost').pathname;
    if (name === '/') { res.setHeader('content-type','text/html'); res.end('<!doctype html><title>Protected seller outbox</title>'); return; }
    if (name === '/worker.mjs') { res.setHeader('content-type','text/javascript'); res.end(source); return; }
    const selectedRoot = name.startsWith('/before/') ? previousRoot : root;
    const path = resolve(selectedRoot, '.' + (name.startsWith('/before/') ? name.slice(7) : name));
    assert(path.startsWith(selectedRoot + sep));
    res.setHeader('content-type',path.endsWith('.wasm')?'application/wasm':'text/javascript');
    res.end(await readFile(path));
  } catch { res.writeHead(404); res.end(); }
});
await new Promise(done => server.listen(0,'127.0.0.1',done));
const browser = await playwright[engine].launch({headless:true});
const watchdog = setTimeout(() => { void browser.close(); },180_000);
watchdog.unref();
const results = [];
try {
  for (const contextName of ['page','worker']) {
    const context = await browser.newContext();
    try {
      const page = await context.newPage();
      page.setDefaultTimeout(60_000);
      await page.goto(`http://127.0.0.1:${server.address().port}`);
      const tests = contextName === 'page'
        ? await page.evaluate(async code => { const fixture = await import('/fixture.js'); await fixture.default(); const previous = await import('/before/fixture.js'); await previous.default(); return await (0,eval)(`(${code})`)(fixture,previous); },workflow.toString())
        : await page.evaluate(() => new Promise((done,fail) => { const worker = new Worker('/worker.mjs',{type:'module'}); const timer = setTimeout(() => { worker.terminate(); fail(Error('worker timeout')); },60_000); worker.onmessage = event => { clearTimeout(timer); worker.terminate(); event.data.error ? fail(Error(event.data.error)) : done(event.data.tests); }; worker.onerror = error => { clearTimeout(timer); worker.terminate(); fail(Error(error.message)); }; worker.postMessage({run:true}); }));
      assert.equal(tests.length,5);
      results.push({context:contextName,tests});
      console.error(JSON.stringify({engine,context:contextName,passed:tests.length}));
    } finally { await context.close(); }
  }
  console.log(JSON.stringify({engine,version:browser.version(),results}));
} finally { clearTimeout(watchdog); await browser.close(); await new Promise(done => server.close(done)); }
