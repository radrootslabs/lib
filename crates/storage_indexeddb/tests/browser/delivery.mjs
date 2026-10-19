import assert from 'node:assert/strict';

export async function runDelivery(page, profile) {
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
    const open=r=>WorkspaceFixture.open(r.n,new Uint8Array(r.fingerprint),new Uint8Array(r.selection),new Uint8Array(r.proof));
    const receipt=(owner,n)=>({n,fingerprint:Array.from(owner.fingerprint()),selection:Array.from(owner.selection()),proof:Array.from(owner.proof()),namespace:owner.data_namespace()});
    const setup=async(n,index=0,large=false)=>{const owner=await WorkspaceFixture.create(n+1);await owner.admit(n,false);const wire=await(large?owner.new_large_delivery_plan(n):owner.new_delivery_plan(n));await owner.bind_plan(n,wire);const seal=await owner.obtain(n,index,false);await owner.capture(seal);await owner.install(n);const envelope=await owner.envelope(n,index,0);return{owner,envelope,receipt:receipt(owner,n+1)};};
    const query=async(owner,n,index,id)=>JSON.parse(await owner.query_envelope(n,index,id));
    const store=async(s,n,id,selection=1,mode=0,envelope=s.envelope)=>JSON.parse(await s.owner.store_envelope(n,id,selection,mode,envelope));
    const key=async(n,kind,recipient=0,ordinal=0,part=0)=>{const digest=new Uint8Array(await crypto.subtle.digest('SHA-256',new Uint8Array([...new TextEncoder().encode('radroots.browser.local_outbox.operation.prefix.v2\0'),...new Uint8Array(32).fill(n)])));const raw=new Uint8Array(32);raw.set(digest.subarray(0,16));raw[16]=kind;raw[17]=recipient;new DataView(raw.buffer).setUint16(18,ordinal);raw[20]=part;return raw;};
    const put=IDBObjectStore.prototype.put,add=IDBObjectStore.prototype.add,abort=IDBTransaction.prototype.abort;
    const inject=(namespace,target,redirect)=>{
      IDBObjectStore.prototype.put=function(bytes,at){if(this.transaction.db.name===namespace&&JSON.stringify(kb(at))===JSON.stringify(kb(target))){const r=add.call(this,bytes,redirect??at);r.addEventListener('error',event=>event.preventDefault());return r;}return put.call(this,bytes,at);};
      IDBTransaction.prototype.abort=function(){if(this.db.name===namespace)return;return abort.call(this);};
    };
    const reset=()=>{IDBObjectStore.prototype.put=put;IDBTransaction.prototype.abort=abort;};
    try {
      if(profile==='delivery') {
        const n=210,s=await setup(n),before=await rows(s.receipt);
        const originalClaim=Array.from(await s.owner.attempt(n,0));same(await query(s.owner,n,0,1),null);await rejects(()=>store(s,n,1,1,16),'invalid_evidence');sameRows(await rows(s.receipt),before);
        passed('actual D absence is read without a transport invocation or replacement');
        const first=await store(s,n,1);same(first.kind,'claimed');same(first.report.raw,s.envelope.raw());same(first.report.event,s.envelope.event());same(first.report.targets.length,2);same(first.report.selection.length,1);same(first.report.ordinals,[1]);same(await query(s.owner,n,0,1),first.report);
        passed('actual D commit retains exact signed outer, original work, full policy and selected target before invocation');
        s.owner.set_time(1850);const random=Crypto.prototype.getRandomValues;
        try{Crypto.prototype.getRandomValues=()=>{throw Error('retained D acquired new entropy');};const replay=await store(s,n,1);same(replay.kind,'retained');same(replay.report,first.report);}
        finally{Crypto.prototype.getRandomValues=random;}
        passed('exact retained D replay after original delivery expiry preserves identity and never authorizes redispatch');
        s.owner.set_time(1600);const second=await store(s,n,2,2,16);same(second.kind,'claimed');same(second.report.raw,first.report.raw);same(second.report.policy,first.report.policy);same(second.report.ordinals,[1]);same(second.report.at,1600);if(second.report.transaction===first.report.transaction)throw Error('new D reused original transaction');
        passed('new selected-endpoint retry retains the original exact outer and policy after signing expiry with an independent ordinal');
        const stable=await rows(s.receipt);await rejects(()=>store(s,n,2,1,16),'idempotency_conflict');await rejects(()=>store(s,n,3,1,5),'idempotency_conflict');sameRows(await rows(s.receipt),stable);
        passed('retained request conflicts and changed original publication criterion refuse without writes');
        const another=await s.owner.envelope(n,0,0),third=await store(s,n,3,1,0,another);same(third.kind,'claimed');same(third.report.ordinals,[2]);if(third.report.event===first.report.event)throw Error('independent wrap reused its original event');
        const snapshot=JSON.parse(await s.owner.snapshot(n));same(snapshot.deliveries,3);same(snapshot.observations,0);same(snapshot.publications.length,1);same(snapshot.message.text,'\u0001'.repeat(3000));same(Array.from(await s.owner.attempt(n,0)),originalClaim);await s.owner.close();
        passed('fresh wrapping of the original obtained seal advances only selected transport attempts while retaining one logical message');
        const peerN=212,peer=await setup(peerN,1);await store(peer,peerN,1);const self=await peer.owner.obtain(peerN,0,false);await peer.owner.capture(self);await peer.owner.install(peerN);const progress=JSON.parse(await peer.owner.snapshot(peerN));same(progress.publications.length,2);same(progress.deliveries,1);await peer.owner.close();
        passed('actual first self claim remains legal after peer delivery retention without replacing original peer work');
        return{tests,receipt:s.receipt,operation:n,index:0,id:1,golden:first.report};
      }
      if(profile==='delivery_limits') {
        const n=214,s=await setup(n);let first;
        for(let id=1;id<=12;id++){const value=await store(s,n,id,1,id===1?0:16);same(value.kind,'claimed');same(value.report.ordinals,[id]);first??=value.report;same(value.report.raw,first.raw);}
        const exhausted=await rows(s.receipt);await rejects(()=>store(s,n,13,1,16),'delivery_attempt_limit');sameRows(await rows(s.receipt),exhausted);
        passed('all twelve actual selected-endpoint invocations retain contiguous ordinals and the thirteenth is refused');
        const other=await store(s,n,14,2,16);same(other.report.ordinals,[1]);same(other.report.raw,first.raw);same(other.report.policy,first.policy);same((await store(s,n,1)).kind,'retained');await s.owner.close();
        passed('an exhausted endpoint does not consume another endpoint budget or change the original completion policy');
        const largeN=216,large=await setup(largeN,0,true),value=await store(large,largeN,1,32768,6);same(value.kind,'claimed');same(value.report.targets.length,16);same(value.report.selection.length,1);same(value.report.ordinals,[1]);
        const actual=await rows(large.receipt),parts=actual.keys.map(kb).filter(key=>key[16]===7&&key[20]>0);if(parts.length<2||parts.length>24)throw Error('actual valid long-target frame did not exercise bounded multipart storage');same(actual.keys.map(kb).filter(key=>key[16]===9).length,1);same(await query(large.owner,largeN,0,1),value.report);await large.owner.close();
        passed('actual sixteen-target policy and last-target selection span protected private parts with one matching request index');
        return{tests};
      }
      if(profile==='delivery_corruption') {
        const n=218,s=await setup(n);await store(s,n,1);await store(s,n,2,2,16);await s.owner.close();const stable=await rows(s.receipt);
        const at=(kind,ordinal,part)=>stable.keys.find(k=>{const bytes=kb(k);return bytes[16]===kind&&bytes[18]===0&&bytes[19]===ordinal&&bytes[20]===part;});
        for(const mode of ['manifest','part','index','tampered','foreign_index','t2_loss','all_predecessors']) {
          await change(s.receipt,store=>{
            if(mode==='manifest')store.delete(at(7,1,0));if(mode==='part')store.delete(at(7,1,1));if(mode==='index')store.delete(at(9,1,0));
            if(mode==='tampered'){const selected=at(7,1,1),i=stable.keys.indexOf(selected),raw=new Uint8Array(stable.values[i]);raw[raw.length-1]^=1;store.put(raw,selected);}
            if(mode==='foreign_index')store.put(stable.values[stable.keys.indexOf(at(9,2,0))],at(9,1,0));
            if(mode==='t2_loss'||mode==='all_predecessors')stable.keys.filter(k=>(mode==='all_predecessors'?[3,4,5,6]:[5,6]).includes(kb(k)[16])).forEach(k=>store.delete(k));
          });
          const damaged=await rows(s.receipt);let current=await open(s.receipt);await rejects(()=>query(current,n,0,1),'recovery_required');await current.close();
          current=await open(s.receipt);await rejects(()=>current.install(n),'recovery_required');await current.close();
          if(mode==='all_predecessors'){current=await open(s.receipt);await rejects(()=>current.claim(n,0),'recovery_required');await current.close();}
          sameRows(await rows(s.receipt),damaged);await restore(s.receipt,stable);
          passed('actual '+mode+' D damage refuses original query and predecessor replacement with exact damaged bytes preserved');
        }
        return{tests};
      }
      if(profile==='delivery_faults') {
        const n=220,s=await setup(n),initial=await rows(s.receipt);
        for(const mode of [1,2,3]){const invalid=await s.owner.envelope(n,0,mode);await rejects(()=>store(s,n,1,1,0,invalid),'invalid_evidence');}
        await rejects(()=>store(s,n,1,1,4),'unverified_approval');sameRows(await rows(s.receipt),initial);
        passed('actual valid outer signatures with wrong key, wrong contained seal, participant author or unapproved relay cannot create D facts');
        for(const [mode,code] of [[1,'workspace_unavailable'],[2,'grant_expired'],[3,'deadline_exceeded']]){await rejects(()=>store(s,n,1,1,mode),code);await s.owner.close();sameRows(await rows(s.receipt),initial);s.owner=await open(s.receipt);}
        s.owner.set_time(1000);
        passed('actual D callback rechecks clock, grant and original delivery deadline after crypto and before mutations');
        let written=false;const manifest=await key(n,7,0,1,0);
        try{IDBObjectStore.prototype.put=function(bytes,at){const request=put.call(this,bytes,at);if(this.transaction.db.name===s.receipt.namespace&&JSON.stringify(kb(at))===JSON.stringify(kb(manifest)))request.addEventListener('success',()=>{written=true;this.transaction.abort();},{once:true});return request;};await rejects(()=>store(s,n,1),'storage_failure');}
        finally{reset();}
        same(written,true);sameRows(await rows(s.receipt),initial);same((await store(s,n,1)).kind,'claimed');await s.owner.close();
        passed('actual frame-write success followed by abort retains no D fact and a known-rollback retry uses the caller original');
        const unknownN=222,unknown=await setup(unknownN),unchanged=await key(unknownN,1,0,0,1);let anchor;
        try{anchor=JSON.parse(await unknown.owner.store_envelope_unknown(unknownN,1,1,unknown.envelope,()=>inject(unknown.receipt.namespace,unchanged)));}
        finally{reset();}
        same(anchor.kind,'unknown');same(anchor.request,'01'.repeat(32));if(!anchor.transaction)throw Error('uncertain D lost its original transaction');await unknown.owner.close();let reopened=await open(unknown.receipt);const retained=await query(reopened,unknownN,0,1);same(retained.transaction,anchor.transaction);same(retained.raw,unknown.envelope.raw());await reopened.close();
        passed('actual unexpected completion preserves original D uncertainty and reopens the exact committed frame and transaction');
        const partialN=224,partial=await setup(partialN),failedIndex=await key(partialN,9,0,1),oldT0=await key(partialN,1,0,0,1);
        try{anchor=JSON.parse(await partial.owner.store_envelope_unknown(partialN,1,1,partial.envelope,()=>inject(partial.receipt.namespace,failedIndex,oldT0)));}
        finally{reset();}
        same(anchor.kind,'unknown');if(!anchor.transaction)throw Error('partial D lost original transaction');await partial.owner.close();const damaged=await rows(partial.receipt);reopened=await open(partial.receipt);await rejects(()=>query(reopened,partialN,0,1),'recovery_required');await reopened.close();sameRows(await rows(partial.receipt),damaged);
        passed('actual partial D completion with a missing request index is retained as unknown and cannot become absence or replacement');
        const cancelN=226,cancel=await setup(cancelN);await cancel.owner.abandon_envelope(cancelN,1,1,cancel.envelope);cancel.envelope.free();cancel.owner.abandon_close();await cancel.owner.close();reopened=await open(cancel.receipt);const completed=await query(reopened,cancelN,0,1);if(!completed)throw Error('lost observer discarded accepted original D input');await reopened.close();
        passed('accepted D owns its bounded candidate through observer loss, original disposal and cancelled close');
        return{tests};
      }
      throw Error('unknown delivery profile');
    } finally {reset();}
  },profile);
  if(result.receipt) {
    const message={kind:'delivery_query',...result.receipt,operation:result.operation,index:result.index,id:result.id};
    const worker=await page.evaluate(message=>new Promise((resolve,reject)=>{const worker=new Worker('./worker.js',{type:'module'});worker.onmessage=e=>{if(e.data.ready){worker.postMessage(message);return;}worker.terminate();resolve(e.data);};worker.onerror=error=>{worker.terminate();reject(Error(error.message));};}),message);
    assert.deepEqual(worker,{report:result.golden});result.tests.push('actual worker termination and fresh WASM preserve the exact original D report');
    await page.reload();
    const report=await page.evaluate(async r=>{const{default:init,WorkspaceFixture}=await import('./fixture.js');await init();const owner=await WorkspaceFixture.open(r.receipt.n,new Uint8Array(r.receipt.fingerprint),new Uint8Array(r.receipt.selection),new Uint8Array(r.receipt.proof));const report=JSON.parse(await owner.query_envelope(r.operation,r.index,r.id));await owner.close();return report;},result);
    assert.deepEqual(report,result.golden);result.tests.push('actual page and WASM restart preserve original D bytes, policy, selection and transaction');
    delete result.receipt;delete result.golden;
  }
  return result;
}
