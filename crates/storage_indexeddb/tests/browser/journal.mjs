import assert from 'node:assert/strict';

export async function runJournal(page,profile) {
  const result=await page.evaluate(async profile=>{
    const {default:init,WorkspaceFixture}=await import('./fixture.js');await init();
    const tests=[],passed=name=>{tests.push(name);console.log('PASS '+name);};
    const same=(a,b)=>{if(JSON.stringify(a)!==JSON.stringify(b))throw Error('assertion mismatch: '+JSON.stringify(a)+' / '+JSON.stringify(b));};
    const rejects=async(fn,code)=>{try{await fn();}catch(error){if(String(error)!==code)throw Error('expected '+code+', received '+String(error));return;}throw Error('expected refusal: '+code);};
    const kb=key=>Array.from(new Uint8Array(key));
    const receipt=(owner,n)=>({n,fingerprint:Array.from(owner.fingerprint()),selection:Array.from(owner.selection()),proof:Array.from(owner.proof()),namespace:owner.data_namespace()});
    const open=(r,capture=false)=>(capture?WorkspaceFixture.open_capture:WorkspaceFixture.open)(r.n,new Uint8Array(r.fingerprint),new Uint8Array(r.selection),new Uint8Array(r.proof));
    const rawOpen=r=>new Promise((resolve,reject)=>{const req=indexedDB.open(r.namespace,1);req.onupgradeneeded=()=>req.transaction.abort();req.onsuccess=()=>resolve(req.result);req.onerror=reject;});
    const rows=async r=>{const db=await rawOpen(r);try{return await new Promise((resolve,reject)=>{const tx=db.transaction('records','readonly'),store=tx.objectStore('records');let keys,values;store.getAllKeys().onsuccess=e=>keys=e.target.result;store.getAll().onsuccess=e=>values=e.target.result;tx.oncomplete=()=>resolve({keys,values});tx.onabort=reject;});}finally{db.close();}};
    const sameRows=(a,b)=>{same(a.keys.map(kb),b.keys.map(kb));same(a.values.map(kb),b.values.map(kb));};
    const plan=async(owner,n)=>{await owner.admit(n,false);await owner.bind_plan(n,await owner.new_delivery_plan(n));return owner.obtain(n,0,false);};
    const setup=async n=>{const owner=await WorkspaceFixture.create(n+1),seal=await plan(owner,n);return{owner,seal,receipt:receipt(owner,n+1)};};
    const nativePut=IDBObjectStore.prototype.put,nativeAdd=IDBObjectStore.prototype.add,nativeAbort=IDBTransaction.prototype.abort,nativeClose=IDBDatabase.prototype.close,nativeOpen=IDBFactory.prototype.open;
    const reset=()=>{IDBObjectStore.prototype.put=nativePut;IDBTransaction.prototype.abort=nativeAbort;IDBDatabase.prototype.close=nativeClose;IDBFactory.prototype.open=nativeOpen;};
    const inject=(namespace,key,redirect)=>{
      IDBObjectStore.prototype.put=function(bytes,at){if(this.transaction.db.name===namespace&&JSON.stringify(kb(at))===JSON.stringify(kb(key))){const req=nativeAdd.call(this,bytes,redirect??at);req.addEventListener('error',event=>event.preventDefault());return req;}return nativePut.call(this,bytes,at);};
      IDBTransaction.prototype.abort=function(){if(this.db.name===namespace)return;return nativeAbort.call(this);};
    };
    const unknownSeal=async s=>{const prefix=new Uint8Array(await crypto.subtle.digest('SHA-256',new Uint8Array([...new TextEncoder().encode('radroots.browser.local_outbox.operation.prefix.v2\0'),...new Uint8Array(32).fill(s.receipt.n-1)]))).slice(0,16),all=await rows(s.receipt),at=all.keys.find(k=>kb(k)[16]===1&&kb(k)[20]===1&&kb(k).slice(0,16).every((byte,index)=>byte===prefix[index]));if(!at)throw Error('missing original operation fault target');try{inject(s.receipt.namespace,at);return Array.from(await s.owner.capture_unknown(s.seal));}finally{reset();}};
    const setupDelivery=async n=>{const s=await setup(n);await s.owner.capture(s.seal);await s.owner.install(n);s.envelope=await s.owner.envelope(n,0,0);await s.owner.store_envelope(n,1,3,0,s.envelope);return s;};
    const faultTarget=async s=>(await rows(s.receipt)).keys.find(k=>kb(k)[16]===1&&kb(k)[20]===1);
    const unknownObservation=async(s,value)=>{const at=await faultTarget(s);try{return JSON.parse(await s.owner.retain_observation_unknown(value,()=>inject(s.receipt.namespace,at)));}finally{reset();}};
    try {
      if(profile==='journal') {
        const n=200,s=await setup(n),anchor=await unknownSeal(s);await rejects(()=>s.owner.authenticate(),'closed');const stable=await rows(s.receipt);let closes=0,opens=0;
        try{IDBDatabase.prototype.close=function(){if(this.name===s.receipt.namespace)closes++;return nativeClose.call(this);};IDBFactory.prototype.open=function(...args){if(args[0]===s.receipt.namespace)opens++;return nativeOpen.apply(this,args);};await rejects(()=>s.owner.reconcile(s.seal,true),'unauthorized');same(closes,0);same(opens,0);}finally{reset();}
        sameRows(await rows(s.receipt),stable);await rejects(()=>open(s.receipt,true),'writer_active');
        passed('actual uncertain shared journal refuses ordinary admission and wrong-proof recovery without disposing its original connection or writer');
        let contender,opened=false,original;
        try{IDBFactory.prototype.open=function(...args){if(args[0]===s.receipt.namespace&&!opened){opened=true;contender=open(s.receipt,true).then(()=> 'unexpected_writer',error=>String(error));}return nativeOpen.apply(this,args);};original=Array.from(await s.owner.reconcile(s.seal,false));same(original.slice(0,32),anchor.slice(64,96));same(Array.from(await s.owner.captured_original(s.seal)),original);same(opened,true);same(await contender,'writer_active');same(await s.owner.authenticate(),1);}finally{reset();}
        sameRows(await rows(s.receipt),stable);await s.owner.close();
        passed('actual shared-trait reconciliation closes and reopens under the same retained writer and confirms the original capture transaction and bytes');
        const otherN=202,other=await setup(otherN),older=await plan(other.owner,otherN+2);await other.owner.capture(older);await unknownSeal(other);await other.owner.reconcile(older,false);await rejects(()=>other.owner.authenticate(),'closed');await other.owner.reconcile(other.seal,false);same(await other.owner.authenticate(),1);await other.owner.close();
        passed('confirming an unrelated older seal cannot clear the original uncertain capture identity');
        const lateN=206,late=await setup(lateN);await late.owner.revoke();late.owner.set_time(0);const lateAnchor=await unknownSeal(late),confirmed=Array.from(await late.owner.reconcile(late.seal,false));same(confirmed.slice(0,32),lateAnchor.slice(64,96));await late.owner.close();
        passed('actual original-proof reconciliation retains revoked and unavailable-clock obtained material without fresh authority');
        const cancelN=208,cancel=await setup(cancelN);await unknownSeal(cancel);cancel.owner.abandon_reconcile(cancel.seal);cancel.seal.free();cancel.owner.abandon_close();await cancel.owner.close();const reopened=await open(cancel.receipt);if((await reopened.captured(cancelN,0)).length===0)throw Error('cancelled recovery discarded original obtained bytes');await reopened.close();
        passed('accepted same-owner recovery survives lost observation and original disposal while final close drains before releasing the writer');
        return{tests,receipt:s.receipt,operation:n,golden:original};
      }
      if(profile==='journal_observations') {
        const n=210,s=await setupDelivery(n),original=await s.owner.observation(n,0,1,0,1,0),later=await s.owner.observation(n,0,1,0,1,1000100),other=await s.owner.observation(n,0,1,0,3,1000200),expected=JSON.parse(original.original());same((await unknownObservation(s,original)).report,expected);await rejects(()=>s.owner.authenticate(),'closed');await rejects(()=>s.owner.retain_observation(original,true),'unauthorized');
        same(JSON.parse(await s.owner.retain_observation(other,false)).kind,'committed');await rejects(()=>s.owner.authenticate(),'closed');same(JSON.parse(await s.owner.retain_observation(later,false)).report,expected);same(await s.owner.authenticate(),1);const actual=JSON.parse(await s.owner.query_observations(n,0,1));same(actual.length,2);same(actual.find(value=>value.class===1),expected);await s.owner.close();
        passed('actual same-owner target recovery preserves first details and unknown time while another outcome class cannot clear the original uncertainty');
        const missingN=212,missing=await setupDelivery(missingN),value=await missing.owner.observation(missingN,0,1,0,5,0),before=await rows(missing.receipt),old=await faultTarget(missing),target=new Uint8Array(before.keys.find(k=>kb(k)[16]===7&&kb(k)[20]===0));target[16]=8;target[21]=0;target[22]=5;let result;
        try{result=JSON.parse(await missing.owner.retain_observation_unknown(value,()=>inject(missing.receipt.namespace,target,old)));}finally{reset();}same(result.kind,'unknown');same((await rows(missing.receipt)).keys.filter(k=>kb(k)[16]===8).length,0);same(JSON.parse(await missing.owner.retain_observation(value,false)).report,JSON.parse(value.original()));same(await missing.owner.authenticate(),1);same(JSON.parse(await missing.owner.query_observations(missingN,0,1)),[result.report]);await missing.owner.close();
        passed('actual same-owner reconciliation recaptures the exact obtained observation after unexpected completion omitted its row');
        const lateN=214,late=await setupDelivery(lateN),lateValue=await late.owner.observation(lateN,0,1,0,4,0);await late.owner.revoke();late.owner.set_time(0);await unknownObservation(late,lateValue);same(JSON.parse(await late.owner.retain_observation(lateValue,false)).report,JSON.parse(lateValue.original()));await late.owner.close();
        passed('actual same-owner target recovery uses retained proof despite revocation and an unavailable clock');
        const generalN=216,general=await setupDelivery(generalN),oldValue=await general.owner.observation(generalN,0,1,0,1,0),unchanged=await faultTarget(general);
        try{result=JSON.parse(await general.owner.store_envelope_unknown(generalN,2,1,general.envelope,()=>inject(general.receipt.namespace,unchanged)));}finally{reset();}same(result.kind,'unknown');same(JSON.parse(await general.owner.retain_observation(oldValue,false)).kind,'committed');await rejects(()=>general.owner.authenticate(),'closed');await general.owner.close();let reopened=await open(general.receipt);same(JSON.parse(await reopened.query_observations(generalN,0,1)),[JSON.parse(oldValue.original())]);await reopened.close();
        passed('retaining an older obtained fact cannot clear general delivery uncertainty or authorize fresh work');
        const multipleN=218,multiple=await setupDelivery(multipleN),a=await multiple.owner.observation(multipleN,0,1,0,1,0),b=await multiple.owner.observation(multipleN,0,1,1,2,0);same((await unknownObservation(multiple,a)).kind,'unknown');same((await unknownObservation(multiple,b)).kind,'unknown');await multiple.owner.retain_observation(a,false);await rejects(()=>multiple.owner.authenticate(),'closed');await multiple.owner.retain_observation(b,false);await rejects(()=>multiple.owner.authenticate(),'closed');await multiple.owner.close();reopened=await open(multiple.receipt);same(JSON.parse(await reopened.query_observations(multipleN,0,1)).length,2);await reopened.close();
        passed('multiple unrelated uncertain target captures require explicit reopening even when each original fact is subsequently confirmed');
        return{tests};
      }
      if(profile==='journal_reopen') {
        const n=180,s=await setup(n),anchor=await unknownSeal(s),stable=await rows(s.receipt);let upgraded=false;
        try{IDBFactory.prototype.open=function(...args){if(args[0]===s.receipt.namespace&&!upgraded){upgraded=true;const upgrade=nativeOpen.call(this,args[0],2);upgrade.onupgradeneeded=()=>upgrade.result.createObjectStore('foreign_fixture');upgrade.onsuccess=()=>upgrade.result.close();}return nativeOpen.apply(this,args);};await rejects(()=>s.owner.reconcile(s.seal,false),'recovery_required');}finally{reset();}
        same(upgraded,true);
        const actual=await new Promise((resolve,reject)=>{const request=indexedDB.open(s.receipt.namespace);request.onerror=reject;request.onsuccess=async()=>{const db=request.result;try{const state=await new Promise((resolve,reject)=>{const tx=db.transaction('records','readonly'),store=tx.objectStore('records');let keys,values;store.getAllKeys().onsuccess=e=>keys=e.target.result;store.getAll().onsuccess=e=>values=e.target.result;tx.oncomplete=()=>resolve({keys,values});tx.onabort=reject;});resolve({version:db.version,stores:Array.from(db.objectStoreNames),...state});}catch(error){reject(error);}finally{db.close();}};});same(actual.version,2);same(actual.stores,['foreign_fixture','records']);sameRows(actual,stable);await rejects(()=>open(s.receipt,true),'writer_active');let opened=0;
        try{IDBFactory.prototype.open=function(...args){if(args[0]===s.receipt.namespace)opened++;return nativeOpen.apply(this,args);};await rejects(()=>s.owner.reconcile(s.seal,true),'unauthorized');same(opened,0);}finally{reset();}
        passed('actual incompatible existing schema refuses recovery without repair, preserves protected rows and retains writer and original-proof authorization');
        // This restores only an isolated test database after verifying refusal.
        // The SDK never creates, repairs, downgrades or restores the namespace.
        await new Promise((resolve,reject)=>{const request=indexedDB.deleteDatabase(s.receipt.namespace);request.onsuccess=resolve;request.onerror=reject;request.onblocked=()=>reject(Error('fixture namespace deletion blocked'));});
        await new Promise((resolve,reject)=>{const request=indexedDB.open(s.receipt.namespace,1);request.onupgradeneeded=()=>request.result.createObjectStore('records');request.onerror=reject;request.onsuccess=()=>{const db=request.result,tx=db.transaction('records','readwrite'),store=tx.objectStore('records');stable.keys.forEach((key,i)=>store.put(stable.values[i],key));tx.oncomplete=()=>{db.close();resolve();};tx.onabort=()=>{db.close();reject(Error('fixture original restoration aborted'));};};});
        const restored=Array.from(await s.owner.reconcile(s.seal,false));same(restored.slice(0,32),anchor.slice(64,96));same(await s.owner.authenticate(),1);sameRows(await rows(s.receipt),stable);await s.owner.close();
        passed('cached original grant authorization permits same-owner retry after failed reopen only when the isolated original schema and bytes are restored');
        const damagedN=182,damaged=await setup(damagedN);await unknownSeal(damaged);const intact=await rows(damaged.receipt),claim=intact.keys.find(k=>kb(k)[16]===3);let db=await rawOpen(damaged.receipt);await new Promise((resolve,reject)=>{const tx=db.transaction('records','readwrite');tx.objectStore('records').delete(claim);tx.oncomplete=resolve;tx.onabort=reject;});db.close();const broken=await rows(damaged.receipt);await rejects(()=>damaged.owner.reconcile(damaged.seal,false),'recovery_required');sameRows(await rows(damaged.receipt),broken);await rejects(()=>damaged.owner.authenticate(),'closed');await rejects(()=>open(damaged.receipt,true),'writer_active');
        passed('actual missing original claim cannot become capture authority or replacement during same-owner recovery');
        db=await rawOpen(damaged.receipt);await new Promise((resolve,reject)=>{const tx=db.transaction('records','readwrite'),store=tx.objectStore('records');intact.keys.forEach((key,i)=>store.put(intact.values[i],key));tx.oncomplete=resolve;tx.onabort=reject;});db.close();await damaged.owner.reconcile(damaged.seal,false);same(await damaged.owner.authenticate(),1);sameRows(await rows(damaged.receipt),intact);await damaged.owner.close();
        passed('failed original-claim validation retains uncertainty and writer until the actual original capture can be confirmed');
        return{tests};
      }
      throw Error('journal profile is not implemented yet');
    } finally {reset();}
  },profile);
  if(result.receipt) {
    const message={kind:'recovered_query',...result.receipt,operation:result.operation};
    const worker=await page.evaluate(message=>new Promise((resolve,reject)=>{const worker=new Worker('./worker.js',{type:'module'});worker.onmessage=e=>{if(e.data.ready){worker.postMessage(message);return;}worker.terminate();resolve(e.data);};worker.onerror=error=>{worker.terminate();reject(Error(error.message));};}),message);
    assert.deepEqual(worker,{captured:result.golden});result.tests.push('actual worker termination and fresh WASM preserve the exact reconciled original capture');
    await page.reload();const captured=await page.evaluate(async r=>{const{default:init,WorkspaceFixture}=await import('./fixture.js');await init();const owner=await WorkspaceFixture.open(r.receipt.n,new Uint8Array(r.receipt.fingerprint),new Uint8Array(r.receipt.selection),new Uint8Array(r.receipt.proof));const captured=Array.from(await owner.captured(r.operation,0));await owner.close();return captured;},result);
    assert.deepEqual(captured,result.golden);result.tests.push('actual page and WASM restart preserve original reconciled transaction, representation digest and unknown observation time');delete result.receipt;delete result.golden;
  }
  assert(result.tests.length>0);return result;
}
