import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';

const [tools, packageDir, engine = 'chromium'] = process.argv.slice(2);
assert(tools && packageDir && ['chromium', 'firefox', 'webkit'].includes(engine));
const require = createRequire(resolve(tools, 'package.json'));
const playwright = require('playwright');
// Use the already locked browser harness's server implementation; no new
// product dependency, relay implementation or protocol client is introduced.
const {wsServer: WebSocketServer} = require(resolve(tools, 'node_modules/playwright-core/lib/utilsBundle.js'));
const wss = new WebSocketServer({noServer: true});
const sockets = new Set();
const upgrades = [];
const observed = [];
process.on('uncaughtExceptionMonitor',()=>console.error(JSON.stringify({engine,upgrades,observed})));
const vectors = JSON.parse(await readFile('contracts/conformance/vectors/event/verified_admission.v1.json','utf8'));
const event = vectors.vectors[1].input.event;

const observer = `
const ActualWebSocket=globalThis.WebSocket, observedSockets=[];
const actualTimeout=globalThis.setTimeout,actualClear=globalThis.clearTimeout,activeTimers=new Set();
globalThis.setTimeout=(callback,delay,...args)=>{
  let id;id=actualTimeout.call(globalThis,(...values)=>{activeTimers.delete(id);callback(...values);},delay,...args);activeTimers.add(id);return id;
};
globalThis.clearTimeout=id=>{activeTimers.delete(id);return actualClear.call(globalThis,id);};
globalThis.WebSocket=class extends ActualWebSocket {
  constructor(...args){super(...args);observedSockets.push(this);}
};
globalThis.waitForBrowserSocketClose=async route=>{
  const socket=observedSockets.findLast(socket=>socket.url.endsWith('/'+route));
  if(!socket)throw Error('missing actual socket: '+route);
  const deadline=performance.now()+5000;
  while(socket.readyState!==ActualWebSocket.CLOSED){
    if(performance.now()>=deadline)throw Error('socket did not close: '+route);
    await new Promise(resolve=>setTimeout(resolve,10));
  }
};
globalThis.forceBrowserSocketBackpressure=()=>{
  Object.defineProperty(observedSockets.at(-1),'bufferedAmount',{get(){return 4*1024*1024;}});
};
async function verifyCleanup(checkTimers=true,expectedStalls=2){
  await new Promise(resolve=>setTimeout(resolve,80));
  for(const socket of observedSockets){
    if(socket.onopen||socket.onmessage||socket.onerror||socket.onclose)throw Error('retained socket callback');
    if(socket.readyState!==ActualWebSocket.CLOSED)throw Error('retained open socket: '+socket.url+' state='+socket.readyState);
  }
  const inert=['unpolled','cancelled-owner','revoked-owner','expired-owner','closed-owner'];
  if(observedSockets.some(socket=>inert.some(route=>socket.url.endsWith('/'+route))))throw Error('inert or expired connection performed I/O');
  if(observedSockets.filter(socket=>socket.url.endsWith('/stall')).length!==expectedStalls)throw Error('missing original pending handshake attempts');
  if(checkTimers && activeTimers.size)throw Error('retained browser socket timer: '+activeTimers.size);
  return observedSockets.length;
}`;
const execute = `
const {default:init,verify_socket,verify_scope,verify_shared_session,fixture_event}=await import('./fixture.js');await init();
const passed=Array.from(await verify_socket(base));const directDisposed=await verifyCleanup();
const scope=Array.from(await verify_scope(base));const scopeDisposed=await verifyCleanup(true,3);
const shared=Array.from(await verify_shared_session(base));const disposed=await verifyCleanup(false,3);
return {passed,scope,shared,directDisposed,scopeDisposed,disposed,expectedRaw:fixture_event()};`;
const worker = `${observer}
self.onmessage=async event=>{try{const base=event.data;const run=async()=>{${execute}};self.postMessage(await run());}
catch(error){self.postMessage({error:String(error)});}};`;
const server = createServer(async (req, res) => {
  try {
    if(req.url==='/'){res.setHeader('content-type','text/html');res.end('<!doctype html><title>Browser socket fixture</title>');return;}
    if(req.url==='/worker.js'){res.setHeader('content-type','text/javascript');res.end(worker);return;}
    const name = new Map([['/fixture.js','fixture.js'],['/fixture_bg.wasm','fixture_bg.wasm']]).get(req.url);
    if(!name){res.statusCode=404;res.end();return;}
    res.setHeader('content-type',name.endsWith('.wasm')?'application/wasm':'text/javascript');
    res.end(await readFile(resolve(packageDir,name)));
  } catch {res.statusCode=500;res.end();}
});
server.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket));});
server.on('upgrade',(req,socket,head)=>{
  upgrades.push(req.url);
  if(req.url==='/stall')return; // Original pending HTTP upgrade, closed by its owner.
  wss.handleUpgrade(req,socket,head,ws=>{
    ws.on('error',()=>{});
    if(req.url==='/echo')ws.on('message',(data,binary)=>{observed.push({route:req.url,binary,bytes:data.length});ws.send(data,{binary});});
    else if(req.url==='/oversize')ws.send('a'.repeat(1025));
    else if(req.url==='/unicode')ws.send('é'.repeat(800));
    else if(req.url==='/binary-large')ws.send(Buffer.alloc(1025));
    else if(req.url==='/items')for(let i=0;i<257;i++)ws.send('');
    else if(req.url==='/bytes')for(let i=0;i<9;i++)ws.send(Buffer.alloc(512*1024));
    else if(req.url==='/disconnect'){ws.send('before close');ws.close(1000,'peer done');}
    else if(req.url.startsWith('/shared-'))ws.on('message',(data,binary)=>{
      if(binary)return;
      const wire=data.toString(),message=JSON.parse(wire);
      observed.push({route:req.url,wire});
      if(message[0]==='REQ'){
        if(req.url==='/shared-disconnect'){ws.close(1000,'history interrupted');return;}
        if(req.url==='/shared-observer')return;
        ws.send(JSON.stringify(['EVENT',message[1],event]));
        ws.send(JSON.stringify(['EOSE',message[1]]));
      }else if(message[0]==='EVENT'){
        if(req.url==='/shared-noack')return;
        if(req.url==='/shared-badack'){ws.send(JSON.stringify(['OK',message[1].id+'00',true,'']));return;}
        ws.send(JSON.stringify(['OK','11'.repeat(32),true,'unrelated']));
        ws.send(JSON.stringify(['OK',message[1].id,req.url!=='/shared-negative','']));
      }
    });
  });
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`ws://127.0.0.1:${server.address().port}`;
let browser;
const deadline=setTimeout(()=>{void browser?.close();},120000);deadline.unref();
try {
  // The locked Juggler driver asserts when worker WebSocket-open events lack
  // its page request record. Use its supported BiDi channel with the exact same
  // cached Firefox executable; retain every real worker/product assertion.
  const automation=engine==='firefox'?'webdriver_bidi':'playwright_default';
  const options=engine==='firefox'?{headless:true,channel:'moz-firefox-nightly',executablePath:playwright.firefox.executablePath()}:{headless:true};
  browser=await playwright[engine].launch(options);
  let page=await browser.newPage();
  const errors=[],expectedUserAgentDiagnostics=[];
  const expectedDiagnostics=new Set([
    `Firefox can’t establish a connection to the server at ${base}/stall.`,
    `The connection to ${base}/stall was interrupted while the page was loading.`,
  ]);
  const onPageError=error=>{
    // BiDi reports these user-agent diagnostics as script errors for the two
    // deliberately interrupted handshakes. Retain the exact messages separately;
    // every other error still fails, including exceptions from product code.
    if(engine==='firefox' && automation==='webdriver_bidi' && expectedDiagnostics.has(error.message))expectedUserAgentDiagnostics.push(error.message);
    else errors.push(error.message);
  };
  page.on('pageerror',onPageError);
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  await page.addScriptTag({content:observer});
  const main=await page.evaluate(new Function('base',`return (async()=>{${execute}})();`),base);
  console.error(JSON.stringify({phase:'main_completed',engine,result:main}));
  // Each context first proves disposal on its original objects. WebKit can
  // retain the network tasks for cancelled HTTP upgrades after DOM close; six
  // such deliberately stalled tasks otherwise block the next connection.
  // Isolate page and worker fixture resources without renewing any deadline or
  // claiming that local disposal proves a remote/network-task close handshake.
  await page.close();
  page=await browser.newPage();
  page.on('pageerror',onPageError);
  await page.goto(`http://127.0.0.1:${server.address().port}/`);
  const dedicated=await page.evaluate(base=>new Promise((resolve,reject)=>{
    const worker=new Worker('./worker.js',{type:'module'});
    worker.onerror=event=>{worker.terminate();reject(Error(event.message));};
    worker.onmessage=event=>{worker.terminate();event.data.error?reject(Error(event.data.error)):resolve(event.data);};
    worker.postMessage(base);
  }),base);
  assert.equal(main.passed.length,19);
  assert.equal(main.shared.length,7);
  assert.equal(main.scope.length,4);
  assert.deepEqual(main,dedicated);
  assert.deepEqual(errors,[]);
  assert(!upgrades.includes('/unpolled'));
  const publications=observed.filter(item=>item.wire?.startsWith('["EVENT",'));
  assert.equal(publications.length,8);
  for(const publication of publications)assert.equal(publication.wire,`["EVENT",${main.expectedRaw}]`);
  // A deadline may expire before the user agent transmits an HTTP upgrade.
  assert(upgrades.filter(path=>path==='/stall').length<=6);
  assert(expectedUserAgentDiagnostics.length<=12);
  console.log(JSON.stringify({engine,automation,version:browser.version(),main,dedicated,pageErrors:errors,expectedUserAgentDiagnostics,upgrades,observed}));
} finally {
  clearTimeout(deadline);
  await browser?.close();
  for(const client of wss.clients)client.terminate();
  for(const socket of sockets)socket.destroy();
  await new Promise(resolve=>wss.close(resolve));
  await new Promise(resolve=>server.close(resolve));
}
