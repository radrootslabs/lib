import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';

const [moduleRoot, packageDir, engine = 'chromium'] = process.argv.slice(2);
assert(moduleRoot && packageDir, 'usage: node run.mjs PLAYWRIGHT_PACKAGE WASM_DIRECTORY ENGINE');
assert(['chromium', 'firefox', 'webkit'].includes(engine));
const playwright = createRequire(resolve(moduleRoot, 'package.json'))('playwright');
// Observe the actual host timer request, without a tight elapsed-time assertion.
const delayedCheck = `async function delayedCheck(verify) {
  const original=globalThis.setTimeout,delays=[];
  globalThis.setTimeout=function(callback,delay,...args){delays.push(delay);return original.call(globalThis,callback,delay,...args);};
  try {await verify();} finally {globalThis.setTimeout=original;}
  if(delays.length>1 || delays.some(delay=>delay!==0))throw Error('delayed owner renewed timer budget: '+JSON.stringify(delays));
}`;
const longCheck = `async function longCheck(verify) {
  const original=globalThis.setTimeout,clear=globalThis.clearTimeout,delays=[],active=new Set();
  globalThis.setTimeout=function(callback,delay,...args){
    delays.push(delay);let id;id=original.call(globalThis,(...values)=>{active.delete(id);callback(...values);},delay,...args);active.add(id);return id;
  };
  globalThis.clearTimeout=function(id){active.delete(id);return clear.call(globalThis,id);};
  try {await verify();} finally {globalThis.setTimeout=original;globalThis.clearTimeout=clear;}
  if(delays.some(value=>value<0 || value>2147483647) || !delays.includes(2147483647))throw Error('invalid long timer delay: '+JSON.stringify(delays));
  if(active.size!==0)throw Error('long timer leaked after cancellation');
}`;
const worker = `import init,{verify_host_time,verify_delayed_timer_owner,verify_long_timer_owner,verify_extreme_timer_owner,verify_fractional_deadlines} from './fixture.js';
${delayedCheck}
${longCheck}
self.onmessage=async()=>{try{await init();await delayedCheck(verify_delayed_timer_owner);await longCheck(verify_long_timer_owner);await longCheck(verify_extreme_timer_owner);await verify_fractional_deadlines();self.postMessage({passed:['delayed timer owner','long timer cancellation','extreme timer cancellation','fractional deadline',...Array.from(await verify_host_time())]});}
catch(error){self.postMessage({error:String(error)});}};`;
const server = createServer(async (req, res) => {
  try {
    if (req.url === '/') {res.setHeader('content-type', 'text/html');res.end('<!doctype html><title>Host time fixture</title>');return;}
    if (req.url === '/worker.js') {res.setHeader('content-type', 'text/javascript');res.end(worker);return;}
    const name = new Map([['/fixture.js', 'fixture.js'], ['/fixture_bg.wasm', 'fixture_bg.wasm']]).get(req.url);
    if (!name) {res.statusCode = 404;res.end();return;}
    res.setHeader('content-type', name.endsWith('.wasm') ? 'application/wasm' : 'text/javascript');
    res.end(await readFile(resolve(packageDir, name)));
  } catch {res.statusCode = 500;res.end();}
});
await new Promise(resolve => server.listen(0, '127.0.0.1', resolve));
let browser;
const deadline = setTimeout(() => {void browser?.close();}, 60000);
deadline.unref();
try {
  browser = await playwright[engine].launch({headless: true});
  const page = await browser.newPage();
  const errors = [];
  page.on('pageerror', error => errors.push(error.message));
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.addScriptTag({content: delayedCheck});
  await page.addScriptTag({content: longCheck});
  const main = await page.evaluate(async () => {
    const {default: init, verify_host_time, verify_delayed_timer_owner,verify_long_timer_owner,verify_extreme_timer_owner,verify_fractional_deadlines} = await import('./fixture.js');
    await init();await delayedCheck(verify_delayed_timer_owner);
    await longCheck(verify_long_timer_owner);await longCheck(verify_extreme_timer_owner);
    await verify_fractional_deadlines();
    return ['delayed timer owner','long timer cancellation','extreme timer cancellation','fractional deadline',...Array.from(await verify_host_time())];
  });
  const dedicated = await page.evaluate(() => new Promise((resolve, reject) => {
    const worker = new Worker('./worker.js', {type: 'module'});
    worker.onerror = event => {worker.terminate();reject(Error(event.message));};
    worker.onmessage = event => {
      worker.terminate();event.data.error ? reject(Error(event.data.error)) : resolve(event.data.passed);
    };
    worker.postMessage('run');
  }));
  assert.equal(main.length, 12);
  assert.deepEqual(dedicated, main);
  assert.deepEqual(errors, []);
  console.log(JSON.stringify({engine, version: browser.version(), main, dedicated, pageErrors: errors}));
} finally {
  clearTimeout(deadline);
  await browser?.close();
  await new Promise(resolve => server.close(resolve));
}
