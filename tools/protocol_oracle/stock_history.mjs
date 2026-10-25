// Independent synthetic causal-store fixtures. Fixed public test scalars,
// nonces and auxiliary bytes provide no production custody or entropy claim.
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
const publicKey=n=>getPublicKey(secret(n));
const seller=publicKey(3),buyer=publicKey(2),oldSeller=publicKey(1);
assert.equal(Buffer.from(schnorr.sign(new Uint8Array(32),secret(3),new Uint8Array(32))).toString('hex').toUpperCase(),'E907831F80848D1069A5371B402410364BDF1C5F8307B0084C55F1CE2DCA821525F66A4A85EA8B71E482A74F382D2CE5EBEEE8FDB2172F477DF4900D310536C0');
const canonical=value=>value===null||typeof value!=='object'?JSON.stringify(value):Array.isArray(value)?'['+value.map(canonical).join(',')+']':'{'+Object.keys(value).sort().map(key=>JSON.stringify(key)+':'+canonical(value[key])).join(',')+'}';
const publicPath='crates/nostr/tests/fixtures/public_heads.v1.json';
const recordPath='crates/protocol/tests/fixtures/commerce_wire.v1.json';
const publicFixture=JSON.parse(read(publicPath));assert.equal(publicFixture.author,seller);
const listingRaw=publicFixture.vectors.find(row=>row.name==='listing_fixed').raw;
const listing=JSON.parse(listingRaw);assert(verifyEvent(JSON.parse(listingRaw)));
const profile=JSON.parse(listing.tags.find(tag=>tag[0]==='radroots:listing')[1]);
const templates=JSON.parse(read(recordPath)).vectors;
const template=name=>JSON.parse(templates.find(row=>row.name===name).wire);
const thread='71'.repeat(32),order='72'.repeat(32);
function sign(fields,scalar) {
  const event={pubkey:publicKey(scalar),...fields};const id=getEventHash(event);
  const signed={id,...event,sig:Buffer.from(schnorr.sign(Buffer.from(id,'hex'),secret(scalar),new Uint8Array(32))).toString('hex')};
  assert(verifyEvent(JSON.parse(JSON.stringify(signed))));return signed;
}
function encrypt(raw,from,to,label) {
  const nonce=Buffer.from(sha('radroots causal fixture/'+label),'hex');
  const encrypted=v2.encrypt(raw,v2.utils.getConversationKey(secret(from),publicKey(to)),nonce);
  assert.equal(v2.decrypt(encrypted,v2.utils.getConversationKey(secret(to),publicKey(from))),raw);return encrypted;
}
let sequence=4000;
function enclose(name,content,author,kind=8890,tags=[]) {
  const unsigned={pubkey:publicKey(author),created_at:typeof content==='string'?102:Number(content.created_at),kind,tags,content:typeof content==='string'?content:canonical(content)};
  const rumor={id:getEventHash(unsigned),...unsigned};const rawRumor=JSON.stringify(rumor);
  const seal=sign({created_at:1699999000-sequence,kind:13,tags:[],content:encrypt(rawRumor,author,3,name+'/rumor')},author);
  const ephemeral=sequence++;const rawSeal=JSON.stringify(seal);
  const wrap=sign({created_at:1699998000-sequence,kind:1059,tags:[['p',seller]],content:encrypt(rawSeal,ephemeral,3,name+'/seal')},ephemeral);
  const rawWrap=JSON.stringify(wrap);
  assert(Buffer.byteLength(unsigned.content)<=16384&&Buffer.byteLength(rawRumor)<=24576&&Buffer.byteLength(rawWrap)<=65536);
  return {name,recipient_scalar:3,raw_wrap:rawWrap,raw_seal:rawSeal,raw_rumor:rawRumor,rumor_id:rumor.id,record_id:typeof content==='string'?null:content.record_id??null};
}
function record(type,payload,author,sequence,previous,parents,orderId=order,time=101) {
  const content={profile:'radroots.commerce',version:1,type,thread_id:thread,order_id:orderId,author:publicKey(author),buyer,seller,created_at:String(time),author_sequence:String(sequence),author_previous:previous,parents:[...new Set(parents)].sort(),payload};
  assert(content.parents.length<=8);
  return {...content,record_id:sha(Buffer.concat([Buffer.from('radroots.record.v1\0'),Buffer.from(canonical(content))]))};
}
const chat=enclose('chat','Independent private question',2,14,[['p',seller]]);
const complete=JSON.parse(JSON.stringify(template('quote').payload.terms).replaceAll(oldSeller,seller));
for(const line of complete.lines) {
  line.source={coordinate:{kind:30402,author:seller,d:publicFixture.d},event_id:listing.id};
  line.product=structuredClone(profile.product);line.option=structuredClone(profile.options[0]);line.option_id=profile.options[0].id;
}
const quoted=record('quote',{terms:complete,expires_at:'250',in_reply_to:chat.rumor_id},3,1,null,[listing.id,chat.rumor_id],null,100);
const requested=record('request',{terms:complete,expires_at:'240',quote:quoted.record_id},2,1,null,[listing.id,quoted.record_id]);
const withdrawn=record('withdrawal_requested',{authorization:requested.record_id,reason:'Withdraw exact request'},2,2,requested.record_id,[requested.record_id]);
const fork=record('withdrawal_requested',{authorization:requested.record_id,reason:'Conflicting same-sequence statement'},2,2,requested.record_id,[requested.record_id]);
const declined=record('decline',{request:requested.record_id,reason:'Seller decline'},3,1,null,[requested.record_id]);
const missing='ab'.repeat(32);
const gap=record('withdrawal_requested',{authorization:requested.record_id,reason:'Missing predecessor'},2,3,missing,[missing,requested.record_id]);
const wrongPrevious=record('withdrawal_requested',{authorization:requested.record_id,reason:'Wrong predecessor author/scope'},2,2,quoted.record_id,[quoted.record_id,requested.record_id]);
const unrelated=record('issue',{code:'other',description:'Independent unrelated scope',lines:[],source_records:[]},2,1,null,[]);
unrelated.thread_id='73'.repeat(32);delete unrelated.record_id;unrelated.record_id=sha(Buffer.concat([Buffer.from('radroots.record.v1\0'),Buffer.from(canonical(unrelated))]));
const rows=[chat,enclose('quote',quoted,3),enclose('request',requested,2),enclose('request_rewrapped',requested,2),enclose('withdrawal',withdrawn,2),enclose('buyer_fork',fork,2),enclose('decline',declined,3),enclose('missing_predecessor',gap,2),enclose('wrong_predecessor',wrongPrevious,2),enclose('unrelated',unrelated,2),enclose('future_commerce',canonical({profile:'radroots.commerce',version:2,type:'future_record',private_future:'Unknown authenticated statement'}),2)];
const direct=record('request',{terms:complete,expires_at:'240',quote:null},2,1,null,[listing.id]);
const wrongSnapshot=structuredClone(complete);wrongSnapshot.lines[0].option.label='Changed copied option snapshot';
const wrongTerms=structuredClone(complete);wrongTerms.fulfillment[0].terms.instructions+=' Changed exact quote terms.';
const promise=structuredClone(complete);promise.supply={type:'future_promise',expected_window:{start:'300',end:'400',timezone:'Etc/UTC'},risk_disclosure:'Weather and future supply may change.'};
const variable=structuredClone(complete);variable.lines[0].price={type:'variable_weight',measurement_unit:'kg',minimum:'1',maximum:'2',rate:{currency:'CAD',exponent:2,minor:'150'},basis:'1',maximum_total:{currency:'CAD',exponent:2,minor:'300'}};
variable.lines[0].maximum_line_total={currency:'CAD',exponent:2,minor:'300'};variable.maximum_total={currency:'CAD',exponent:2,minor:'325'};
for(const [name,content] of [
  ['request_no_quote',direct],
  ['request_wrong_snapshot',record('request',{terms:wrongSnapshot,expires_at:'240',quote:null},2,1,null,[listing.id])],
  ['request_wrong_quote_terms',record('request',{terms:wrongTerms,expires_at:'240',quote:quoted.record_id},2,1,null,[listing.id,quoted.record_id])],
  ['request_early_expiry',record('request',{terms:complete,expires_at:'151',quote:quoted.record_id},2,1,null,[listing.id,quoted.record_id])],
  ['request_other_order',record('request',{terms:complete,expires_at:'240',quote:null},2,1,null,[listing.id],'74'.repeat(32))],
  ['request_future_promise',record('request',{terms:promise,expires_at:'240',quote:null},2,1,null,[listing.id])],
  ['request_variable_weight',record('request',{terms:variable,expires_at:'240',quote:null},2,1,null,[listing.id])],
])rows.push(enclose(name,content,2));
const output={schema_version:1,scope:{thread,order,buyer,seller},listing_raw:listingRaw,listing_id:listing.id,quote:quoted.record_id,request:requested.record_id,withdrawal:withdrawn.record_id,buyer_fork:fork.record_id,decline:declined.record_id,missing,producer:{library:'nostr-tools@2.23.5',curve:'@noble/curves@2.0.1',bip340_primary_vector_zero_matched:true,script_sha256:sha(readFileSync(new URL(import.meta.url))),package_lock_sha256:sha(read('tools/protocol_oracle/package-lock.json')),redistribution_provenance_sha256:sha(read('tools/protocol_oracle/provenance.v1.json')),record_template_path:recordPath,record_template_sha256:sha(read(recordPath)),public_source_path:publicPath,public_source_sha256:sha(read(publicPath)),synthetic_only:true,production_entropy_claimed:false},sources:rows};
const bytes=JSON.stringify(output,null,2)+'\n';
const target=new URL('crates/storage_sqlite/tests/fixtures/stock_history.v1.json',root);
const args=process.argv.slice(2);assert(args.length===1&&['--write','--check'].includes(args[0]));
if(args[0]==='--write')writeFileSync(target,bytes);else assert.equal(readFileSync(target,'utf8'),bytes);
console.log(JSON.stringify({sources:rows.length,sha256:sha(bytes),bytes:Buffer.byteLength(bytes),check:args[0]==='--check'}));
