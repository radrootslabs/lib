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
  let phase='legacy setup';
  try {
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
  const originalPlanWire = Uint8Array.from(await old.new_plan(243));
  const originalPlan = Array.from(await old.bind_plan(243, originalPlanWire));
  const obtained = await old.obtain(243, 0, false);
  const originalClaim = Array.from(obtained.original_claim());
  const originalCapture = Array.from(await old.capture(obtained));
  await old.close();
  phase='published private reopen';
  const current = await WorkspaceFixture.open(244, oldFingerprint, oldSelection, oldProof);
  check(!await current.is_seller());
  check(JSON.stringify(Array.from(await current.query(243))) === JSON.stringify(original));
  check(JSON.stringify(Array.from(await current.admit(243, false))) === JSON.stringify(original));
  check(JSON.stringify(Array.from(await current.plan_query(243))) === JSON.stringify(originalPlan));
  check(JSON.stringify(Array.from(await current.bind_plan(243, originalPlanWire))) === JSON.stringify(originalPlan));
  check(JSON.stringify(Array.from(await current.attempt(243, 0))) === JSON.stringify(originalClaim));
  check(JSON.stringify(Array.from(await current.captured(243, 0))) === JSON.stringify(originalCapture));
  await refuses(() => current.obtain(243, 0, false), 'retained_no_dispatch');
  await current.close();
  tests.push('original published buyer T0, T1, claim and obtained capture reopen without replacement or reissue');
  phase='seller public T0/T1';
  const seller = await WorkspaceFixture.create_seller(245);
  check(await seller.is_seller());
  check(await seller.authenticate() === 1);
  await refuses(() => seller.wrong_proof(), 'unauthorized');
  const preparation = Array.from(await seller.admit(242, false));
  const publicPreparation=Array.from(await seller.admit_public(244));
  const publicPlan=Array.from(await seller.bind_public(244));
  check(JSON.stringify(Array.from(await seller.admit_public(244)))===JSON.stringify(publicPreparation));
  check(JSON.stringify(Array.from(await seller.bind_public(244)))===JSON.stringify(publicPlan));
  await refuses(()=>seller.admit_public(245),'stale_state');
  check((await seller.query(245)).length===0);
  phase='public signing';
  const publicOutput=await seller.obtain(244,2,false);
  check(publicOutput.malformed_output_refused());
  seller.set_time(2000);
  phase='late public capture';
  const publicCapture=Array.from(await seller.capture(publicOutput));
  seller.set_time(1500);
  phase='late public installation';
  const publicInstallation=await seller.install_public(244);
  check(await seller.install_public(244)===publicInstallation);
  check(JSON.parse(publicInstallation).observed===null);
  check(JSON.parse(publicInstallation).installed_at==='1500');
  check(JSON.parse(publicInstallation).capture===publicCapture.slice(0,32).map(value=>value.toString(16).padStart(2,'0')).join(''));
  tests.push('public T0/T1 replay, coordinate refusal, real verified S, late null-time capture and exactly-once T2 after signing expiry');
  const fingerprint = seller.fingerprint(), selection = seller.selection(), proof = seller.proof();
  await seller.close();
  phase='retained public reopen';
  let reopened = await WorkspaceFixture.open(245, fingerprint, selection, proof);
  reopened.set_time(1500);
  check(await reopened.is_seller());
  check(JSON.stringify(Array.from(await reopened.query(242))) === JSON.stringify(preparation));
  check(JSON.stringify(Array.from(await reopened.query(244)))===JSON.stringify(publicPreparation));
  check(JSON.stringify(Array.from(await reopened.public_plan_query(244)))===JSON.stringify(publicPlan));
  check(await reopened.install_public(244)===publicInstallation);
  check(JSON.stringify(Array.from(await reopened.captured(244,2)))===JSON.stringify(publicCapture));
  await refuses(()=>reopened.obtain(244,2,false),'retained_no_dispatch');
  tests.push('actual protected close/reopen retains seller scope and original private/public preparation and plan');
  const originalSnapshot=await reopened.public_snapshot(244);
  const namespace=reopened.data_namespace(),coordinate=Array.from(await reopened.public_coordinate_key(244));
  const openRaw=()=>new Promise((resolve,reject)=>{const request=indexedDB.open(namespace,1);request.onsuccess=()=>resolve(request.result);request.onerror=reject;request.onupgradeneeded=()=>request.transaction.abort();});
  const readRows=async()=>{const db=await openRaw();try{return await new Promise((resolve,reject)=>{const tx=db.transaction('records','readonly'),store=tx.objectStore('records');let keys,values;store.getAllKeys().onsuccess=event=>keys=event.target.result;store.getAll().onsuccess=event=>values=event.target.result;tx.oncomplete=()=>resolve({keys,values});tx.onabort=reject;});}finally{db.close();}};
  const change=async callback=>{const db=await openRaw();try{await new Promise((resolve,reject)=>{const tx=db.transaction('records','readwrite');callback(tx.objectStore('records'));tx.oncomplete=resolve;tx.onabort=reject;});}finally{db.close();}};
  const bytes=value=>Array.from(new Uint8Array(value));
  const before=await readRows(),coordinateIndex=before.keys.findIndex(key=>JSON.stringify(bytes(key))===JSON.stringify(coordinate));
  check(coordinateIndex>=0);
  const prefixInput=new Uint8Array([...new TextEncoder().encode('radroots.browser.local_outbox.operation.prefix.v2\0'),...new Uint8Array(32).fill(244)]);
  const prefix=Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',prefixInput)).slice(0,16));
  const at=kind=>before.keys.findIndex(key=>{const raw=bytes(key);return raw[16]===kind&&JSON.stringify(raw.slice(0,16))===JSON.stringify(prefix);});
  const claim=at(3),evidence=at(4),installed=at(10),work=at(11);
  check([claim,evidence,installed,work].every(index=>index>=0));
  await reopened.close();
  const faults=[... [coordinateIndex,claim,evidence,installed,work].map(index=>({index})),{index:installed,replacement:work},{index:work,replacement:installed},{index:coordinateIndex,flip:true,fresh:true},{index:claim,move:true}];
  for(const fault of faults){
    phase='public corruption '+JSON.stringify(fault);
    const key=before.keys[fault.index];let moved;
    await change(store=>{
      if(fault.move){moved=new Uint8Array(key).slice();moved[17]=0;store.delete(key);store.put(before.values[fault.index],moved.buffer);}
      else if(fault.flip){const value=new Uint8Array(before.values[fault.index]).slice();value[value.length-1]^=1;store.put(value.buffer,key);}
      else if(fault.replacement!==undefined)store.put(before.values[fault.replacement],key);
      else store.delete(key);
    });
    const corrupted=await readRows();
    reopened=await WorkspaceFixture.open(245,fingerprint,selection,proof);reopened.set_time(fault.fresh?1000:1500);
    await refuses(()=>fault.fresh?reopened.admit_public(245):reopened.public_snapshot(244),'recovery_required');
    await reopened.close();
    const unchanged=await readRows();
    check(JSON.stringify(unchanged.keys.map(bytes))===JSON.stringify(corrupted.keys.map(bytes)));
    check(JSON.stringify(unchanged.values.map(bytes))===JSON.stringify(corrupted.values.map(bytes)));
    phase+=' restore';
    await change(store=>{if(moved)store.delete(moved.buffer);store.put(before.values[fault.index],key);});
    reopened=await WorkspaceFixture.open(245,fingerprint,selection,proof);reopened.set_time(1500);
    check(await reopened.public_snapshot(244)===originalSnapshot);await reopened.close();
  }
  reopened=await WorkspaceFixture.open(245,fingerprint,selection,proof);reopened.set_time(1500);
  tests.push('actual protected public predecessor deletion, cross-family replacement, corrupt coordinate and wrong-target records refuse without rewriting evidence');
  phase='expiry and revocation';
  reopened.set_time(2000);
  await refuses(() => reopened.authenticate(), 'grant_expired');
  reopened.set_time(1500);
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
  } catch(error) { throw Error(phase+': '+String(error)); }
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
      assert.equal(tests.length,6);
      results.push({context:contextName,tests});
      console.error(JSON.stringify({engine,context:contextName,passed:tests.length}));
    } finally { await context.close(); }
  }
  console.log(JSON.stringify({engine,version:browser.version(),results}));
} finally { clearTimeout(watchdog); await browser.close(); await new Promise(done => server.close(done)); }
