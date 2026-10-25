// Independent synthetic initial agreement/recipient intents, with no Radroots
// constructor. Public fixed test scalars/nonces are never runtime custody.
import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {getEventHash,getPublicKey,verifyEvent} from 'nostr-tools/pure';
import {v2} from 'nostr-tools/nip44';
import {schnorr} from '@noble/curves/secp256k1.js';
const root=new URL('../../',import.meta.url),read=path=>readFileSync(new URL(path,root),'utf8');
const sha=bytes=>createHash('sha256').update(bytes).digest('hex');
const canonical=value=>value===null||typeof value!=='object'?JSON.stringify(value):Array.isArray(value)?'['+value.map(canonical).join(',')+']':'{'+Object.keys(value).sort().map(key=>JSON.stringify(key)+':'+canonical(value[key])).join(',')+'}';
const secret=n=>Buffer.from(BigInt(n).toString(16).padStart(64,'0'),'hex');
const source='crates/protocol/tests/fixtures/commerce_wire.v1.json',data=JSON.parse(read(source));
const requestWire=data.vectors.find(row=>row.name==='request').wire,request=JSON.parse(requestWire);
const old=JSON.parse(data.vectors.find(row=>row.name==='agreement_committed').wire);
assert.equal(request.seller,getPublicKey(secret(1)));assert.equal(request.buyer,getPublicKey(secret(2)));
const terms=sha(Buffer.concat([Buffer.from('radroots.terms.v1\0'),Buffer.from(canonical(request.payload.terms))]));
const command={type:'orders.accept',context:{operation_id:'81'.repeat(32),actor:{participant:request.seller,role:'seller',organization:null,grant_id:'82'.repeat(32)},workspace_id:'83'.repeat(32),authority:old.payload.authority,route_digest:'84'.repeat(32),approval:null,expected:{order_id:request.order_id,agreement:null,seller_sequence:'0',buyer_head:request.record_id,checkpoint:{digest:'85'.repeat(32),sequence:'2',coverage:'scope_satisfied',missing:[]}},not_after:'600',correlation_id:null},input:{request:request.record_id,terms_digest:terms,allocation:{type:'automatic'}}};
const commandWire=canonical(command);
// The protocol omits correlation from its semantic transcript. Keep that
// independent transcript explicit rather than serializing an undefined key.
const semantic=structuredClone(command);delete semantic.context.correlation_id;
const digest=sha(Buffer.concat([Buffer.from('radroots.command.v1\0'),Buffer.from(canonical(semantic))]));
const record={profile:'radroots.commerce',version:1,type:'agreement_committed',thread_id:request.thread_id,order_id:request.order_id,author:request.seller,buyer:request.buyer,seller:request.seller,created_at:'451',author_sequence:'1',author_previous:null,parents:[request.record_id],payload:{base_agreement:null,authorization:request.record_id,terms_digest:terms,authority:old.payload.authority,seller_sequence:'1'}};
const recordId=sha(Buffer.concat([Buffer.from('radroots.record.v1\0'),Buffer.from(canonical(record))]));
const recordWire=canonical({...record,record_id:recordId});
const fields={pubkey:record.author,created_at:451,kind:8890,tags:[],content:recordWire};
const rumor={id:getEventHash(fields),...fields},rumorWire=JSON.stringify(rumor);
const seals=[];
for(const recipient of [request.seller,request.buyer].sort()) {
    const scalar=recipient===request.seller?1:2;
    const nonce=Buffer.from(sha('radroots initial commitment fixture/'+recipient),'hex');
    const ciphertext=v2.encrypt(rumorWire,v2.utils.getConversationKey(secret(1),recipient),nonce);
    assert.equal(v2.decrypt(ciphertext,v2.utils.getConversationKey(secret(scalar),request.seller)),rumorWire);
    const fields={pubkey:request.seller,created_at:450,kind:13,tags:[],content:ciphertext};
    const id=getEventHash(fields),event={id,...fields,sig:Buffer.from(schnorr.sign(Buffer.from(id,'hex'),secret(1),new Uint8Array(32))).toString('hex')};
    assert(verifyEvent(JSON.parse(JSON.stringify(event))));
    const raw=JSON.stringify(event);assert(Buffer.byteLength(raw)<=65536);
    const routeFields={pubkey:recipient,created_at:449,kind:10050,tags:[['relay','wss://relay.example']],content:''};
    const routeId=getEventHash(routeFields),route={id:routeId,...routeFields,sig:Buffer.from(schnorr.sign(Buffer.from(routeId,'hex'),secret(scalar),new Uint8Array(32))).toString('hex')};
    assert(verifyEvent(JSON.parse(JSON.stringify(route))));
    const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;},u64=n=>{const b=Buffer.alloc(8);b.writeBigUInt64BE(BigInt(n));return b;};
    const field=bytes=>Buffer.concat([u64(bytes.length),bytes]);
    const authored=sha(Buffer.concat([field(Buffer.from('radroots.authored_event_plan.v1')),u32(1),u32(7),field(Buffer.from('radroots.message.seal.v1')),field(Buffer.from(request.seller,'hex')),u64(450),u32(13),u32(0),field(Buffer.from(ciphertext)),field(Buffer.from(id,'hex'))]));
    const intent=sha(Buffer.concat([Buffer.from('radroots.exact_signing_intent.v1\0'),Buffer.from(command.context.operation_id,'hex'),Buffer.from(digest,'hex'),Buffer.from(authored,'hex'),Buffer.from([1,1]),Buffer.from(recipient,'hex'),u64(600000)]));
    seals.push({recipient,recipient_scalar:scalar,ciphertext,event_id:id,raw,route:{event_id:routeId,raw:JSON.stringify(route),selected:['wss://relay.example']},intent_digest:intent});
}
const u32=n=>{const b=Buffer.alloc(4);b.writeUInt32BE(n);return b;},u64=n=>{const b=Buffer.alloc(8);b.writeBigUInt64BE(BigInt(n));return b;};
const text=s=>Buffer.concat([u32(Buffer.byteLength(s)),Buffer.from(s)]);
const plan=Buffer.concat([Buffer.from('radroots.commerce.signing_plan.v1\0'),Buffer.from(command.context.operation_id,'hex'),Buffer.from(digest,'hex'),Buffer.from(recordId,'hex'),Buffer.from(rumor.id,'hex'),u64(451),Buffer.from([0,2]),...seals.flatMap(row=>[Buffer.from(row.recipient,'hex'),Buffer.from(request.seller,'hex'),u32(13),Buffer.from([0,1]),Buffer.from(row.event_id,'hex'),u64(450),text(row.ciphertext),Buffer.from(row.route.event_id,'hex'),text(row.route.raw),Buffer.from([1]),text(row.route.selected[0])])]);
const output={schema_version:1,producer:{script_sha256:sha(readFileSync(new URL(import.meta.url))),package_lock_sha256:sha(read('tools/protocol_oracle/package-lock.json')),redistribution_provenance_sha256:sha(read('tools/protocol_oracle/provenance.v1.json')),source_path:source,source_sha256:sha(read(source)),synthetic_only:true,production_entropy_claimed:false},request_wire:requestWire,command_wire:commandWire,command_digest:digest,created_at:'451',record_wire:recordWire,record_id:recordId,rumor_wire:rumorWire,rumor_id:rumor.id,seals,plan_hex:plan.toString('hex'),plan_digest:sha(plan)};
output.stock_routes=[2,3].map(scalar=>{
    const participant=getPublicKey(secret(scalar)),fields={pubkey:participant,created_at:100,kind:10050,tags:[['relay','wss://relay.example']],content:''};
    const id=getEventHash(fields),event={id,...fields,sig:Buffer.from(schnorr.sign(Buffer.from(id,'hex'),secret(scalar),new Uint8Array(32))).toString('hex')};
    assert(verifyEvent(JSON.parse(JSON.stringify(event))));
    return {recipient:participant,event_id:id,raw:JSON.stringify(event)};
});
const bytes=JSON.stringify(output,null,2)+'\n';
const args=process.argv.slice(2);assert(args.length===1&&['--write','--check'].includes(args[0]));
for(const path of ['crates/protocol/tests/fixtures/commerce_signing.v1.json','crates/nostr/tests/fixtures/commerce_signing.v1.json','crates/storage_sqlite/tests/fixtures/commerce_signing.v1.json']) {
    const target=new URL(path,root);
    if(args[0]==='--write')writeFileSync(target,bytes);else assert.equal(readFileSync(target,'utf8'),bytes);
}
console.log(JSON.stringify({sha256:sha(bytes),bytes:Buffer.byteLength(bytes),seals:seals.length,check:args[0]==='--check'}));
