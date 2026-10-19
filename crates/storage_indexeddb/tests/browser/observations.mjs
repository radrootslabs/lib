import assert from 'node:assert/strict';

export async function runObservations(page,profile) {
  const result=await page.evaluate(async profile=>{
    const {default:init,WorkspaceFixture}=await import('./fixture.js');await init();
    const tests=[],passed=name=>{tests.push(name);console.log('PASS '+name);};
    const same=(a,b)=>{if(JSON.stringify(a)!==JSON.stringify(b))throw Error('assertion mismatch: '+JSON.stringify(a)+' / '+JSON.stringify(b));};
    const rejects=async(fn,code)=>{try{await fn();}catch(error){if(String(error)!==code)throw Error('expected '+code+', received '+String(error));return;}throw Error('expected refusal: '+code);};
    const kb=key=>Array.from(new Uint8Array(key));
    const rawOpen=r=>new Promise((resolve,reject)=>{const req=indexedDB.open(r.namespace,1);req.onupgradeneeded=()=>req.transaction.abort();req.onsuccess=()=>resolve(req.result);req.onerror=reject;});
    const rows=async r=>{const db=await rawOpen(r);try{return await new Promise((resolve,reject)=>{const tx=db.transaction('records','readonly'),store=tx.objectStore('records');let keys,values;store.getAllKeys().onsuccess=e=>keys=e.target.result;store.getAll().onsuccess=e=>values=e.target.result;tx.oncomplete=()=>resolve({keys,values});tx.onabort=reject;});}finally{db.close();}};
    const change=async(r,fn)=>{const db=await rawOpen(r);try{await new Promise((resolve,reject)=>{const tx=db.transaction('records','readwrite');fn(tx.objectStore('records'));tx.oncomplete=resolve;tx.onabort=reject;});}finally{db.close();}};
    const sameRows=(a,b)=>{same(a.keys.map(kb),b.keys.map(kb));same(a.values.map(kb),b.values.map(kb));};
    const restore=async(r,before)=>change(r,store=>{store.clear();before.keys.forEach((key,i)=>store.put(before.values[i],key));});
    const open=(r,capture=false)=>(capture?WorkspaceFixture.open_capture:WorkspaceFixture.open)(r.n,new Uint8Array(r.fingerprint),new Uint8Array(r.selection),new Uint8Array(r.proof));
    const receipt=(owner,n)=>({n,fingerprint:Array.from(owner.fingerprint()),selection:Array.from(owner.selection()),proof:Array.from(owner.proof()),namespace:owner.data_namespace()});
    const setup=async(n,selection=1)=>{const owner=await WorkspaceFixture.create(n+1);await owner.admit(n,false);await owner.bind_plan(n,await owner.new_delivery_plan(n));const seal=await owner.obtain(n,0,false);await owner.capture(seal);await owner.install(n);const envelope=await owner.envelope(n,0,0);await owner.store_envelope(n,1,selection,0,envelope);return{owner,envelope,receipt:receipt(owner,n+1)};};
    const query=async(owner,n,id=1)=>JSON.parse(await owner.query_observations(n,0,id));
    const retain=async(s,original)=>JSON.parse(await s.owner.retain_observation(original,false));
    const put=IDBObjectStore.prototype.put,add=IDBObjectStore.prototype.add,abort=IDBTransaction.prototype.abort;
    const reset=()=>{IDBObjectStore.prototype.put=put;IDBTransaction.prototype.abort=abort;};
    const inject=(namespace,target,redirect)=>{
      IDBObjectStore.prototype.put=function(bytes,at){if(this.transaction.db.name===namespace&&JSON.stringify(kb(at))===JSON.stringify(kb(target))){const req=add.call(this,bytes,redirect??at);req.addEventListener('error',event=>event.preventDefault());return req;}return put.call(this,bytes,at);};
      IDBTransaction.prototype.abort=function(){if(this.db.name===namespace)return;return abort.call(this);};
    };
    try {
      if(profile==='observations') {
        const n=230,s=await setup(n,3),empty=await rows(s.receipt);same(await query(s.owner,n),[]);sameRows(await rows(s.receipt),empty);
        passed('actual target observation query preserves absence without inventing an acknowledgement');
        const first=await s.owner.observation(n,0,1,0,1,0),later=await s.owner.observation(n,0,1,0,1,1000100),expected=JSON.parse(first.original());
        await rejects(()=>s.owner.retain_observation(first,true),'unauthorized');sameRows(await rows(s.receipt),empty);
        const saved=await retain(s,first);same(saved.kind,'committed');same(saved.report,expected);same(expected.at,null);same(expected.ordinal,1);same(expected.attempted,true);same(await query(s.owner,n),[expected]);
        passed('actual observation commit requires original proof and preserves exact linkage, absent time and normalized details');
        const before=await rows(s.receipt),random=Crypto.prototype.getRandomValues;
        try{Crypto.prototype.getRandomValues=()=>{throw Error('exact observation replay used new entropy');};same((await retain(s,later)).report,expected);}finally{Crypto.prototype.getRandomValues=random;}
        sameRows(await rows(s.receipt),before);
        passed('first original class observation remains byte-identical when replay supplies a different time and details');
        for(const kind of [2,3,4,5,6]){const value=await s.owner.observation(n,0,1,0,kind,1000000+kind);same((await retain(s,value)).report,JSON.parse(value.original()));}
        const classes=await query(s.owner,n);same(classes.length,6);same(classes.map(value=>value.class),[1,2,3,4,5,6]);same(classes[5].attempted,false);same(classes[0],expected);
        passed('all six actual protected outcome classes append independently without erasing earlier acceptance');
        const accepted=await s.owner.observation(n,0,1,1,1,1000100);await retain(s,accepted);const snapshot=JSON.parse(await s.owner.snapshot(n));same(snapshot.observations,7);same(snapshot.deliveries,1);same(snapshot.publications.length,1);
        passed('coherent actual snapshot retains per-target observations alongside the original message, work and D attempt');
        const alternate=s.owner.alternate_envelope(s.envelope),stable=await rows(s.receipt);same(alternate.event(),s.envelope.event());if(alternate.raw()===s.envelope.raw())throw Error('alternate fixture signature unchanged');await rejects(()=>s.owner.store_envelope(n,2,1,0,alternate),'idempotency_conflict');sameRows(await rows(s.receipt),stable);
        passed('independently valid alternate signed outer representation with the same event ID cannot replace retained bytes');
        const golden=await query(s.owner,n);await s.owner.close();return{tests,receipt:s.receipt,operation:n,index:0,id:1,golden};
      }
      if(profile==='observation_faults') {
        const n=232,s=await setup(n),original=await s.owner.observation(n,0,1,0,2,0),expected=JSON.parse(original.original()),before=await rows(s.receipt);
        let written=false;
        try{IDBObjectStore.prototype.put=function(bytes,key){const req=put.call(this,bytes,key);if(this.transaction.db.name===s.receipt.namespace&&kb(key)[16]===8)req.addEventListener('success',()=>{written=true;this.transaction.abort();},{once:true});return req;};await rejects(()=>retain(s,original),'storage_failure');}finally{reset();}
        same(written,true);sameRows(await rows(s.receipt),before);same((await retain(s,original)).report,expected);
        passed('actual observation write success followed by abort rolls back and preserves the caller original for retry');
        const late=await s.owner.observation(n,0,1,0,3,1000200),captureOnly=await s.owner.observation(n,0,1,0,5,0);s.owner.set_time(2500);same((await retain(s,late)).kind,'committed');await rejects(()=>s.owner.query_observation_scope(late),'grant_expired');await s.owner.close();
        s.owner=await open(s.receipt,true);s.owner.set_time(0);same((await retain(s,captureOnly)).report,JSON.parse(captureOnly.original()));const lateUnknown=await s.owner.retain_observation(late,false);same(JSON.parse(lateUnknown).report,JSON.parse(late.original()));await rejects(()=>s.owner.query_observation_scope(late),'unauthorized');await rejects(()=>s.owner.authenticate(),'unauthorized');await s.owner.close();
        passed('expired and unavailable-clock capture-only owners retain original obtained target evidence without fresh access');
        const revokedN=234,revoked=await setup(revokedN),observed=await revoked.owner.observation(revokedN,0,1,0,4,1000300);await revoked.owner.revoke();same((await retain(revoked,observed)).kind,'committed');await revoked.owner.close();revoked.owner=await open(revoked.receipt,true);same((await retain(revoked,observed)).report,JSON.parse(observed.original()));await rejects(()=>revoked.owner.authenticate(),'unauthorized');await revoked.owner.close();
        passed('revocation cannot discard obtained evidence authenticated against the actual original D attempt');
        const unknownN=236,unknown=await setup(unknownN),value=await unknown.owner.observation(unknownN,0,1,0,5,0),all=await rows(unknown.receipt),unchanged=all.keys.find(k=>kb(k)[16]===1&&kb(k)[20]===1);let result;
        try{result=JSON.parse(await unknown.owner.retain_observation_unknown(value,()=>inject(unknown.receipt.namespace,unchanged)));}finally{reset();}
        same(result.kind,'unknown');same(result.report,JSON.parse(value.original()));await unknown.owner.close();let reopened=await open(unknown.receipt);same(await query(reopened,unknownN),[result.report]);await reopened.close();
        passed('actual unexpected transaction completion returns the original uncertain observation and reconciliation reads the actual retained fact');
        const partialN=242,partial=await setup(partialN),partialValue=await partial.owner.observation(partialN,0,1,0,5,0),partialRows=await rows(partial.receipt),old=partialRows.keys.find(k=>kb(k)[16]===1&&kb(k)[20]===1),target=new Uint8Array(partialRows.keys.find(k=>kb(k)[16]===7&&kb(k)[20]===0));target[16]=8;target[21]=0;target[22]=5;
        try{result=JSON.parse(await partial.owner.retain_observation_unknown(partialValue,()=>inject(partial.receipt.namespace,target,old)));}finally{reset();}
        same(result.kind,'unknown');same(result.report,JSON.parse(partialValue.original()));await partial.owner.close();reopened=await open(partial.receipt);same(await query(reopened,partialN),[]);same(JSON.parse(await reopened.retain_observation(partialValue,false)).report,result.report);same(await query(reopened,partialN),[result.report]);await reopened.close();
        passed('actual incomplete observation retention stays unknown until exact query and recapture confirm the caller original without redispatch');
        const cancelN=238,cancel=await setup(cancelN),cancelled=await cancel.owner.observation(cancelN,0,1,0,6,0),golden=JSON.parse(cancelled.original());cancel.owner.abandon_observation(cancelled);cancelled.free();cancel.owner.abandon_close();await cancel.owner.close();reopened=await open(cancel.receipt);same(await query(reopened,cancelN),[golden]);await reopened.close();
        passed('accepted observation owns bounded input through lost observer, original disposal and cancelled close');
        return{tests};
      }
      if(profile==='observation_corruption') {
        const n=240,s=await setup(n);await s.owner.store_envelope(n,2,2,16,s.envelope);const a=await s.owner.observation(n,0,1,0,1,0),b=await s.owner.observation(n,0,2,1,2,1000000);await retain(s,a);await retain(s,b);
        await rejects(()=>s.owner.observation(n,0,1,1,1,0),'invalid_evidence');await rejects(()=>s.owner.observation(n,0,1,0,7,0),'invalid_evidence');await s.owner.close();const stable=await rows(s.receipt);
        passed('shared checked normalized data refuses unselected targets and recipient-delivered meaning for relay outcomes');
        const found=(family,ordinal)=>stable.keys.find(k=>kb(k)[16]===family&&kb(k)[19]===ordinal),observation=found(8,1),other=found(8,2),original=stable.values[stable.keys.indexOf(observation)];
        for(const mode of ['tampered','foreign_attempt','unselected_target','changed_class','missing_D','missing_T2','missing_S']) {
          await change(s.receipt,store=>{
            if(mode==='tampered'){const bytes=new Uint8Array(original);bytes[bytes.length-1]^=1;store.put(bytes,observation);}
            if(mode==='foreign_attempt')store.put(stable.values[stable.keys.indexOf(other)],observation);
            if(mode==='unselected_target'||mode==='changed_class'){const key=new Uint8Array(observation);key[mode==='changed_class'?22:21]=mode==='changed_class'?2:1;store.delete(observation);store.put(original,key);}
            if(mode==='missing_D')stable.keys.filter(k=>[7,9].includes(kb(k)[16])&&kb(k)[19]===1).forEach(k=>store.delete(k));
            if(mode==='missing_T2'||mode==='missing_S')stable.keys.filter(k=>(mode==='missing_T2'?[5,6]:[3,4]).includes(kb(k)[16])).forEach(k=>store.delete(k));
          });
          const damaged=await rows(s.receipt);let owner=await open(s.receipt);await rejects(()=>query(owner,n),'recovery_required');await owner.close();owner=await open(s.receipt);await rejects(()=>owner.install(n),'recovery_required');await owner.close();owner=await open(s.receipt);await rejects(()=>owner.retain_observation(a,false),'recovery_required');await owner.close();if(mode==='missing_S'){owner=await open(s.receipt);await rejects(()=>owner.claim(n,0),'recovery_required');await owner.close();}sameRows(await rows(s.receipt),damaged);await restore(s.receipt,stable);
          passed('actual '+mode+' observation damage refuses query, replacement and capture while preserving damaged bytes');
        }
        return{tests};
      }
      throw Error('unknown observation profile');
    } finally {reset();}
  },profile);
  if(result.receipt) {
    const message={kind:'observation_query',...result.receipt,operation:result.operation,index:result.index,id:result.id};
    const worker=await page.evaluate(message=>new Promise((resolve,reject)=>{const worker=new Worker('./worker.js',{type:'module'});worker.onmessage=e=>{if(e.data.ready){worker.postMessage(message);return;}worker.terminate();resolve(e.data);};worker.onerror=error=>{worker.terminate();reject(Error(error.message));};}),message);
    assert.deepEqual(worker,{reports:result.golden});result.tests.push('actual worker termination and fresh WASM retain original target observations');
    await page.reload();
    const reports=await page.evaluate(async r=>{const{default:init,WorkspaceFixture}=await import('./fixture.js');await init();const owner=await WorkspaceFixture.open(r.receipt.n,new Uint8Array(r.receipt.fingerprint),new Uint8Array(r.receipt.selection),new Uint8Array(r.receipt.proof));const reports=JSON.parse(await owner.query_observations(r.operation,r.index,r.id));await owner.close();return reports;},result);
    assert.deepEqual(reports,result.golden);result.tests.push('actual page and WASM restart preserve immutable original outcome classes, details and optional times');delete result.receipt;delete result.golden;
  }
  return result;
}
