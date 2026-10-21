import assert from 'node:assert/strict';
import {createRequire} from 'node:module';
import {createServer} from 'node:http';
import {readFile} from 'node:fs/promises';
import {resolve} from 'node:path';

const [tools,packageDir,engine='chromium']=process.argv.slice(2);
assert(tools&&packageDir&&['chromium','firefox','webkit'].includes(engine));
const require=createRequire(resolve(tools,'package.json'));
const playwright=require('playwright');
const {wsServer:WebSocketServer}=require(resolve(tools,'node_modules/playwright-core/lib/utilsBundle.js'));
const wss=new WebSocketServer({noServer:true}),sockets=new Set(),sources=new Map(),observed=[],upgrades=[];
process.on('uncaughtExceptionMonitor',()=>console.error(JSON.stringify({engine,upgrades,observed})));
const observer=`
const ActualWebSocket=globalThis.WebSocket,actualOpen=IDBFactory.prototype.open;
const observedSockets=[],databases=[];
globalThis.WebSocket=class extends ActualWebSocket {constructor(...args){super(...args);observedSockets.push(this);}};
globalThis.browserSocketCount=()=>observedSockets.length;
IDBFactory.prototype.open=function(...args){
  const request=actualOpen.apply(this,args);
  request.addEventListener('success',()=>databases.push(request.result),{once:true});
  return request;
};
globalThis.assertStorageCallbacksClosed=()=>{
  for(const db of databases)if(db.onversionchange||db.onclose)throw Error('explicit close retained database callback');
};
async function verifyCleanup(){
  await new Promise(resolve=>setTimeout(resolve,80));
  for(const socket of observedSockets){
    if(socket.onopen||socket.onmessage||socket.onerror||socket.onclose)throw Error('retained original socket callback');
    if(socket.readyState!==ActualWebSocket.CLOSED)throw Error('original socket remains open');
  }
  assertStorageCallbacksClosed();
  return {sockets:observedSockets.length,databases:databases.length};
}`;
const execute=`
const {default:init,advertisements,verify_client,verify_publications,verify_listing,verify_listing_publications}=await import('./fixture.js');await init();
const route='/'+context,endpoint=base.replace('http:','ws:')+route;
const response=await fetch(base+'/advertisements'+route,{method:'POST',body:advertisements(endpoint)});
if(!response.ok)throw Error('fixture advertisement setup');
const result=JSON.parse(await verify_client(endpoint,'ordinary_browser_'+context));
const events=await fetch(base+'/events'+route).then(response=>response.text());
await verify_publications(events,JSON.stringify(result.messages));
const publicRoute='/listing/'+context;
const listing=JSON.parse(await verify_listing(base.replace('http:','ws:')+publicRoute,'ordinary_public_'+context));
const publicEvents=await fetch(base+'/events'+publicRoute).then(response=>response.text());
verify_listing_publications(publicEvents,listing.revision);
result.listing=listing;
result.disposed=await verifyCleanup();return result;`;
const worker=`${observer}
self.onmessage=async event=>{try{const base=event.data,context='worker';self.postMessage(await(async()=>{${execute}})());}
catch(error){self.postMessage({error:String(error)});}};`;
const server=createServer(async(req,res)=>{
  try{
    if(req.url==='/'){res.setHeader('content-type','text/html');res.end('<!doctype html><title>Ordinary Radroots browser client</title>');return;}
    if(req.url==='/worker.js'){res.setHeader('content-type','text/javascript');res.end(worker);return;}
    if(req.url==='/advertisements/main'||req.url==='/advertisements/worker'){
      assert.equal(req.method,'POST');let body='';for await(const chunk of req){body+=chunk;assert(Buffer.byteLength(body)<=64*1024);}
      const raw=JSON.parse(body);assert.equal(raw.length,2);const values=raw.map(value=>JSON.parse(value));
      for(const value of values)assert.equal(value.kind,10050);
      sources.set(req.url.replace('/advertisements',''),values);res.end('ok');return;
    }
    if(['/events/main','/events/worker','/events/listing/main','/events/listing/worker'].includes(req.url)){
      const route=req.url.replace('/events','');res.setHeader('content-type','application/json');
      res.end(JSON.stringify(observed.filter(item=>item.route===route&&item.message[0]==='EVENT').map(item=>item.rawEvent)));return;
    }
    const name=new Map([['/fixture.js','fixture.js'],['/fixture_bg.wasm','fixture_bg.wasm']]).get(req.url);
    if(!name){res.statusCode=404;res.end();return;}
    res.setHeader('content-type',name.endsWith('.wasm')?'application/wasm':'text/javascript');res.end(await readFile(resolve(packageDir,name)));
  }catch(error){res.statusCode=500;res.end('fixture setup failed');console.error(error);}
});
server.on('connection',socket=>{sockets.add(socket);socket.on('close',()=>sockets.delete(socket));});
server.on('upgrade',(req,socket,head)=>{
  const publicRoute=['/listing/main','/listing/worker'].includes(req.url);
  assert(publicRoute||sources.has(req.url));upgrades.push(req.url);
  wss.handleUpgrade(req,socket,head,ws=>{
    ws.on('error',()=>{});
    ws.on('message',(data,binary)=>{
      assert(!binary);const raw=data.toString(),message=JSON.parse(raw);
      observed.push({route:req.url,message,rawEvent:message[0]==='EVENT'?raw.slice('["EVENT",'.length,-1):undefined});
      if(message[0]==='REQ'){
        assert(!publicRoute,'public listing must not query a private route');
        assert.equal(message[2].kinds[0],10050);assert.equal(message[2].authors.length,1);
        for(const value of sources.get(req.url))if(value.pubkey===message[2].authors[0])ws.send(JSON.stringify(['EVENT',message[1],value]));
        ws.send(JSON.stringify(['EOSE',message[1]]));
      }else if(message[0]==='EVENT'){
        assert.equal(message[1].kind,publicRoute?30402:1059);
        const firstPublic=publicRoute&&observed.filter(item=>item.route===req.url&&item.message[0]==='EVENT').length===1;
        ws.send(JSON.stringify(['OK',message[1].id,!firstPublic,firstPublic?'blocked: measured loopback rejection':'']));
      }else assert.equal(message[0],'CLOSE');
    });
  });
});
await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
const base=`http://127.0.0.1:${server.address().port}`;let browser;
const watchdog=setTimeout(()=>{void browser?.close();},120000);watchdog.unref();
try{
  const automation=engine==='firefox'?'webdriver_bidi':'playwright_default';
  const options=engine==='firefox'?{headless:true,channel:'moz-firefox-nightly',executablePath:playwright.firefox.executablePath()}:{headless:true};
  browser=await playwright[engine].launch(options);let page=await browser.newPage();const errors=[];
  page.on('pageerror',error=>errors.push(error.message));await page.goto(base);await page.addScriptTag({content:observer});
  const main=await page.evaluate(new Function('base',`return(async()=>{const context='main';${execute}})();`),base);
  console.error(JSON.stringify({phase:'main_completed',engine,result:main}));
  await page.close();page=await browser.newPage();page.on('pageerror',error=>errors.push(error.message));await page.goto(base);
  const dedicated=await page.evaluate(base=>new Promise((resolve,reject)=>{
    const worker=new Worker('./worker.js',{type:'module'});
    worker.onerror=event=>{worker.terminate();reject(Error(event.message));};
    worker.onmessage=event=>{worker.terminate();event.data.error?reject(Error(event.data.error)):resolve(event.data);};worker.postMessage(base);
  }),base);
  assert.equal(main.passed.length,7);assert.deepEqual(main.passed,dedicated.passed);assert.deepEqual(errors,[]);
  assert.equal(main.listing.passed.length,4);assert.deepEqual(main.listing.passed,dedicated.listing.passed);
  for(const context of ['main','worker']){
    const publications=observed.filter(item=>item.route==='/'+context&&item.message[0]==='EVENT');
    assert.equal(publications.length,4);assert.equal(new Set(publications.map(item=>item.message[1].id)).size,4);
    assert.equal(upgrades.filter(route=>route==='/'+context).length,2);
    const publicEvents=observed.filter(item=>item.route==='/listing/'+context&&item.message[0]==='EVENT');
    assert.equal(publicEvents.length,2);assert.equal(publicEvents[0].rawEvent,publicEvents[1].rawEvent);
    assert.equal(upgrades.filter(route=>route==='/listing/'+context).length,2);
  }
  console.log(JSON.stringify({engine,automation,version:browser.version(),main,dedicated,pageErrors:errors,upgrades,observed}));
}finally{
  clearTimeout(watchdog);await browser?.close();for(const client of wss.clients)client.terminate();for(const socket of sockets)socket.destroy();
  await new Promise(resolve=>wss.close(resolve));await new Promise(resolve=>server.close(resolve));
}
