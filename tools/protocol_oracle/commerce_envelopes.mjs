// Independent, deterministic synthetic interoperability data. These public
// fixture scalars/nonces/auxiliary bytes must never become runtime custody.
import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {getEventHash,getPublicKey,verifyEvent} from 'nostr-tools/pure';
import {v2} from 'nostr-tools/nip44';
import {schnorr} from '@noble/curves/secp256k1.js';

const root=new URL('../../',import.meta.url);
const read=path=>readFileSync(new URL(path,root),'utf8');
const sha=value=>createHash('sha256').update(value).digest('hex');
const secret=n=>Buffer.from(BigInt(n).toString(16).padStart(64,'0'),'hex');
const hex=value=>Buffer.from(value).toString('hex');
const nonce=name=>Buffer.from(sha('radroots synthetic fixture nonce/'+name),'hex');
const scalarFor=pub=>{for(const n of [1,2])if(getPublicKey(secret(n))===pub)return n;throw new Error('Unknown synthetic actor');};
const recordsText=read('crates/protocol/tests/fixtures/commerce_wire.v1.json');
const records=JSON.parse(recordsText);
const reference=JSON.parse(read('crates/nostr/tests/fixtures/nip44_provider.v1.json'));
assert.equal(v2.encrypt(reference.plaintext,v2.utils.getConversationKey(Buffer.from(reference.secret_a,'hex'),getPublicKey(Buffer.from(reference.secret_b,'hex'))),Buffer.from(reference.nonce,'hex')),reference.ciphertext);

function canonical(value) {
  if(value===null||typeof value==='string'||typeof value==='boolean')return JSON.stringify(value);
  if(typeof value==='number'){assert(Number.isSafeInteger(value)&&value>=0);return JSON.stringify(value);}
  if(Array.isArray(value))return '['+value.map(canonical).join(',')+']';
  const keys=Object.keys(value).sort();assert(keys.every(key=>/^[\x00-\x7f]*$/.test(key)));
  return '{'+keys.map(key=>JSON.stringify(key)+':'+canonical(value[key])).join(',')+'}';
}
function recordWire(body) {
  const value=structuredClone(body);delete value.record_id;
  return canonical({...value,record_id:sha(Buffer.concat([Buffer.from('radroots.record.v1\0'),Buffer.from(canonical(value))]))});
}
function rumor(wire,overrides={}) {
  const body=JSON.parse(wire);
  const value={pubkey:body.author,created_at:Number(body.created_at),kind:8890,tags:[],content:wire,...overrides};
  return {id:getEventHash(value),...value};
}
function sign(value,n) {
  const unsigned={pubkey:getPublicKey(secret(n)),created_at:value.created_at,kind:value.kind,tags:value.tags,content:value.content};
  const id=getEventHash(unsigned);
  const event={id,...unsigned,sig:hex(schnorr.sign(Buffer.from(id,'hex'),secret(n),new Uint8Array(32)))};
  assert(verifyEvent(JSON.parse(JSON.stringify(event))));
  return event;
}
function encrypt(text,sender,recipient,name) {
  const key=v2.utils.getConversationKey(secret(sender),getPublicKey(secret(recipient)));
  const output=v2.encrypt(text,key,nonce(name));
  const reverse=v2.utils.getConversationKey(secret(recipient),getPublicKey(secret(sender)));
  assert.equal(v2.decrypt(output,reverse),text);
  return output;
}
let sequence=1000;
function enclose(name,rawRumor,sender,recipient,{sealMutate,outerMutate,sealAuthor=sender}={}) {
  let seal=sign({created_at:1_700_000_000-sequence,kind:13,tags:[],content:encrypt(rawRumor,sealAuthor,recipient,name+'/rumor')},sealAuthor);
  if(sealMutate)seal=sealMutate(seal,sealAuthor);
  const rawSeal=JSON.stringify(seal);
  const ephemeral=sequence++;
  let outer=sign({created_at:1_699_999_000-sequence,kind:1059,tags:[['p',getPublicKey(secret(recipient))]],content:encrypt(rawSeal,ephemeral,recipient,name+'/seal')},ephemeral);
  if(outerMutate)outer=outerMutate(outer,ephemeral);
  return {name,recipient_scalar:recipient,recipient:getPublicKey(secret(recipient)),rumor_id:JSON.parse(rawRumor).id,raw_rumor:rawRumor,raw_seal:rawSeal,raw_wrap:JSON.stringify(outer)};
}
const positives=[];
for(const row of records.vectors) {
  const body=JSON.parse(row.wire);const sender=scalarFor(body.author);
  const peer=scalarFor(body.author===body.seller?body.buyer:body.seller);
  const unsigned=rumor(row.wire);const raw=JSON.stringify(unsigned);
  assert(Buffer.byteLength(row.wire)<=16384&&Buffer.byteLength(raw)<=24576);
  for(const [copy,recipient] of [['peer',peer],['self',sender]]) {
    const item=enclose(row.name+'/'+copy,raw,sender,recipient);
    assert(Buffer.byteLength(item.raw_seal)<=65536&&Buffer.byteLength(item.raw_wrap)<=65536);
    positives.push({...item,record_fixture:row.name,record_id:row.record_id,expected:{outcome:'known'}});
  }
}
const base=records.vectors.find(r=>r.name==='decline');assert(base);
const baseBody=JSON.parse(base.wire);const baseRumor=rumor(base.wire);const rawBase=JSON.stringify(baseRumor);
const negatives=[];
function negative(name,raw=rawBase,options={},expected={outcome:'error',reason:'invalid_input'},recipient=2) {
  const item=enclose(name,raw,1,recipient,options);negatives.push({...item,expected});return item;
}
const error=reason=>({outcome:'error',reason});
const rejected=reason=>({outcome:'rejected',reason});
negative('outer_id',rawBase,{outerMutate:e=>({...e,id:'00'.repeat(32)})},error('unauthenticated'));
negative('outer_signature',rawBase,{outerMutate:e=>({...e,sig:'00'.repeat(64)})},error('unauthenticated'));
negative('outer_recipient',rawBase,{outerMutate:(e,n)=>sign({...e,tags:[['p',getPublicKey(secret(3))]]},n)},error('recipient_mismatch'));
negative('outer_duplicate_recipient',rawBase,{outerMutate:(e,n)=>sign({...e,tags:[...e.tags,...e.tags]},n)},error('recipient_mismatch'));
negative('outer_missing_recipient',rawBase,{outerMutate:(e,n)=>sign({...e,tags:[]},n)},error('recipient_mismatch'));
negative('outer_kind',rawBase,{outerMutate:(e,n)=>sign({...e,kind:13},n)},error('carrier_mismatch'));
negative('outer_wide_kind',rawBase,{outerMutate:(e,n)=>sign({...e,kind:65536},n)});
negative('outer_unknown_field',rawBase,{outerMutate:e=>({...e,extra:true})});
negative('outer_mac',rawBase,{outerMutate:(e,n)=>{const bytes=Buffer.from(e.content,'base64');bytes[bytes.length-1]^=1;return sign({...e,content:bytes.toString('base64')},n);}},error('provider_authentication_failed'));
negative('outer_encryption_version',rawBase,{outerMutate:(e,n)=>{const bytes=Buffer.from(e.content,'base64');bytes[0]=3;return sign({...e,content:bytes.toString('base64')},n);}},error('unsupported_version'));
negative('seal_id',rawBase,{sealMutate:e=>({...e,id:'00'.repeat(32)})},error('unauthenticated'));
negative('seal_signature',rawBase,{sealMutate:e=>({...e,sig:'00'.repeat(64)})},error('unauthenticated'));
negative('seal_kind',rawBase,{sealMutate:(e,n)=>sign({...e,kind:14},n)},error('carrier_mismatch'));
negative('seal_tags',rawBase,{sealMutate:(e,n)=>sign({...e,tags:[['p',getPublicKey(secret(2))]]},n)},error('carrier_mismatch'));
negative('seal_mac',rawBase,{sealMutate:(e,n)=>{const bytes=Buffer.from(e.content,'base64');bytes[bytes.length-1]^=1;return sign({...e,content:bytes.toString('base64')},n);}},error('provider_authentication_failed'));
negative('seal_encryption_version',rawBase,{sealMutate:(e,n)=>{const bytes=Buffer.from(e.content,'base64');bytes[0]=3;return sign({...e,content:bytes.toString('base64')},n);}},error('unsupported_version'));
negative('seal_author',rawBase,{sealAuthor:3},error('carrier_mismatch'));
negative('rumor_id',JSON.stringify({...baseRumor,id:'00'.repeat(32)}),{},error('unauthenticated'));
negative('rumor_wide_kind',JSON.stringify({...baseRumor,kind:65536}));
negative('rumor_duplicate_field','{"kind":8890,'+rawBase.slice(1));
negative('rumor_unknown_field',JSON.stringify({...baseRumor,extra:true}));
negative('rumor_uppercase_author',JSON.stringify({...baseRumor,pubkey:baseRumor.pubkey.toUpperCase()}));
negative('rumor_time_range',JSON.stringify({...baseRumor,created_at:253402300800}));
negative('known_rumor_tags',JSON.stringify(rumor(base.wire,{tags:[['p',getPublicKey(secret(2))]]})),{},rejected('carrier_mismatch'));
negative('record_identity',JSON.stringify(rumor(canonical({...baseBody,payload:{...baseBody.payload,reason:'Changed synthetic private reason'}}))),{},rejected('record_identity_mismatch'));
negative('record_missing_field',JSON.stringify(rumor(canonical(Object.fromEntries(Object.entries(baseBody).filter(([key])=>key!=='buyer'))),{pubkey:baseBody.author,created_at:Number(baseBody.created_at)})),{},rejected('invalid_profile'));
const changedTime=recordWire({...baseBody,created_at:'451'});
negative('record_time',JSON.stringify(rumor(changedTime,{created_at:450})),{},rejected('carrier_mismatch'));
const issue=JSON.parse(records.vectors.find(r=>r.name==='issue').wire);
assert.notEqual(issue.buyer,baseBody.author);
negative('record_author',JSON.stringify(rumor(recordWire({...issue,author:issue.buyer}),{pubkey:baseBody.author,created_at:450})),{},rejected('carrier_mismatch'));
negative('record_recipient',rawBase,{},rejected('recipient_mismatch'),3);
for(const [name,content] of [
  ['future_profile',{profile:'future.commerce',version:1,type:'future_record',private_future:{value:'Synthetic unknown data'}}],
  ['future_version',{profile:'radroots.commerce',version:2,type:'future_record',private_future:{value:'Synthetic unknown data'}}],
  ['future_type',{profile:'radroots.commerce',version:1,type:'future_record',private_future:{value:'Synthetic unknown data'}}],
])negative(name,JSON.stringify(rumor(JSON.stringify(content),{pubkey:baseBody.author,created_at:450})),{},{outcome:'unsupported',reason:'profile'});
negative('other_rumor_kind',JSON.stringify(rumor(base.wire,{kind:14,content:'Synthetic standard chat text'})),{},{outcome:'unsupported',reason:'carrier_kind'});
negative('record_limit_plus_one',JSON.stringify(rumor(records.invalid[0].wire)),{},error('limit_exceeded'));

// Independently solve complete record/rumor bounds using escaped instructions.
const large=JSON.parse(records.vectors.find(r=>r.name==='exact_private_record_limit').wire);
function fillRecord(backslashes,time) {
  const value=structuredClone(large);value.created_at=String(time);
  value.payload.terms.fulfillment.forEach(g=>g.terms.instructions='');
  const fixed=Buffer.byteLength(recordWire(value));
  const ascii=16384-fixed-2*backslashes;
  if(ascii<0||ascii+backslashes>8*2048)return null;
  let text='\\'.repeat(backslashes)+'a'.repeat(ascii);
  for(const group of value.payload.terms.fulfillment){group.terms.instructions=text.slice(0,2048);text=text.slice(2048);}
  assert.equal(text,'');const wire=recordWire(value);assert.equal(Buffer.byteLength(wire),16384);return wire;
}
let exactRumor=null;
for(const time of [45,450]) {
  for(let k=0;k<=8192;k++) {
    const wire=fillRecord(k,time);if(wire===null)continue;
    const raw=JSON.stringify(rumor(wire));
    if(Buffer.byteLength(raw)===24576){exactRumor={wire,raw};break;}
    if(Buffer.byteLength(raw)>24576)break;
  }
  if(exactRumor)break;
}
assert(exactRumor,'No independently solved complete-rumor boundary');
const exact=enclose('exact_rumor_limit',exactRumor.raw,1,2);
positives.push({...exact,record_fixture:null,record_id:JSON.parse(exactRumor.wire).record_id,expected:{outcome:'known'}});
const overflowBody=JSON.parse(exactRumor.wire);
const extraGroup=overflowBody.payload.terms.fulfillment.find(g=>g.terms.instructions.length<2048);assert(extraGroup);
extraGroup.terms.instructions+='a';
negative('rumor_limit_plus_one',JSON.stringify(rumor(recordWire(overflowBody))),{},error('limit_exceeded'));
assert.equal(Buffer.byteLength(negatives.at(-1).raw_rumor),24577);

const outerBound=enclose('exact_outer_limit',rawBase,1,2);
let outer=JSON.parse(outerBound.raw_wrap);const outerScalar=sequence-1;
const emptyTags=Array.from({length:16},()=>['alt','']);outer=sign({...outer,tags:[...outer.tags,...emptyTags]},outerScalar);
let remaining=65536-Buffer.byteLength(JSON.stringify(outer));assert(remaining>0);
for(let i=1;i<outer.tags.length;i++){const size=Math.min(remaining,4096);outer.tags[i][1]='a'.repeat(size);remaining-=size;}
assert.equal(remaining,0);outer=sign(outer,outerScalar);assert.equal(Buffer.byteLength(JSON.stringify(outer)),65536);
positives.push({...outerBound,raw_wrap:JSON.stringify(outer),record_fixture:'decline',record_id:base.record_id,expected:{outcome:'known'}});
outer.tags.find(tag=>tag[0]==='alt'&&tag[1].length<4096)[1]+='a';outer=sign(outer,outerScalar);
negatives.push({...outerBound,name:'outer_limit_plus_one',raw_wrap:JSON.stringify(outer),expected:error('limit_exceeded')});
assert.equal(Buffer.byteLength(negatives.at(-1).raw_wrap),65537);
const output=JSON.stringify({schema:'radroots.commerce.envelopes.v1',producer:{library:'nostr-tools',version:'2.23.5',git:'ed47ef8ef73d78c338fbaf530e48be0fd4282194',curve:'@noble/curves@2.0.1',primary_nip44_reference_equal:true,source_records_sha256:sha(recordsText),synthetic_only:true,production_entropy_claimed:false},positives,negatives},null,2)+'\n';
const target=new URL('crates/nostr/tests/fixtures/commerce_envelopes.v1.json',root);
const args=process.argv.slice(2);assert(args.length===1&&['--write','--check'].includes(args[0]));
if(args[0]==='--write')writeFileSync(target,output);else assert.equal(sha(readFileSync(target)),sha(output),'Stale independently generated envelope fixture');
console.log(JSON.stringify({positive:positives.length,negative:negatives.length,bytes:Buffer.byteLength(output),sha256:sha(output),primary_nip44_reference_equal:true}));
