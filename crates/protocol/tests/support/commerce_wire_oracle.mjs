// Independently authored synthetic profile bytes. No Rust output is read here.
import { createHash } from 'node:crypto';
import { readFileSync, writeFileSync } from 'node:fs';
import assert from 'node:assert/strict';

const termsSuite=JSON.parse(readFileSync(new URL('../fixtures/terms_wire.v1.json',import.meta.url),'utf8'));
const terms=index=>JSON.parse(termsSuite.vectors[index].wire);
const id=byte=>byte.toString(16).padStart(2,'0').repeat(32);
const hash=value=>createHash('sha256').update(value).digest('hex');
const seller=terms(0).seller,buyer=terms(0).buyer;
const authority={executor_id:id(30),generation:'1'};
const lines=[{line_id:id(5),amount:'1'}];
const reason='Private reason \0 🥬 Å A\u030a';
const window={start:'300',end:'400',timezone:'Etc/UTC'};
const resolution={issues:[id(8)],accepted_receipts:[id(9)],release_residual:lines,summary:reason};
const parents=[3,6,7,8,9,10,11,12].map(id);
function canonical(value) {
  if(value===null||typeof value==='string'||typeof value==='boolean')return JSON.stringify(value);
  if(typeof value==='number'){assert(Number.isSafeInteger(value)&&value>=0);return JSON.stringify(value);}
  if(Array.isArray(value))return '['+value.map(canonical).join(',')+']';
  const keys=Object.keys(value).sort();assert(keys.every(key=>/^[\x00-\x7f]*$/.test(key)));
  return '{'+keys.map(key=>JSON.stringify(key)+':'+canonical(value[key])).join(',')+'}';
}
function header(type,payload,author) {
  return {profile:'radroots.commerce',version:1,type,thread_id:id(20),order_id:type==='quote'?null:id(21),
    author,buyer,seller,created_at:'450',author_sequence:'1',author_previous:null,parents,payload};
}
function encode(body,name=body.type) {
  const without=canonical(body);
  const record_id=hash(Buffer.concat([Buffer.from('radroots.record.v1\0'),Buffer.from(without)]));
  const wire=canonical({...body,record_id});
  return {name,type:body.type,required_parents:requiredParents(body),canonical_without_id:without,record_id,wire,utf8_bytes:Buffer.byteLength(wire),
    raw_preimage_sha256:hash(without),terms_domain_digest:hash(Buffer.concat([Buffer.from('radroots.terms.v1\0'),Buffer.from(without)]))};
}
function requiredParents(body) {
  const fields={quote:['in_reply_to'],request:['quote'],decline:['request'],terms_proposed:['base_agreement'],
    terms_authorized:['proposal'],terms_rejected:['proposal'],agreement_committed:['base_agreement','authorization'],
    withdrawal_requested:['authorization'],withdrawal_acknowledged:['withdrawal'],cancellation_proposed:['agreement'],
    cancellation_authorized:['proposal'],residual_released:['proposal','authorization','agreement'],ready:['agreement'],
    handover:['agreement'],receipt:['handover','supersedes'],issue:[],resolution_proposed:['agreement'],
    resolution_authorized:['proposal'],resolution_committed:['proposal','authorization']};
  const payload=body.payload;
  const refs=fields[body.type].map(field=>payload[field]);
  if(payload.terms)refs.push(...payload.terms.lines.map(line=>line.source.event_id));
  if(body.type==='issue')refs.push(...payload.source_records);
  if(body.type==='resolution_proposed')refs.push(...payload.resolution.issues,...payload.resolution.accepted_receipts);
  refs.push(body.author_previous);
  return [...new Set(refs.filter(value=>value!==null))].sort();
}
const payloads=[
  ['quote',{terms:terms(0),expires_at:'600',in_reply_to:id(6)},seller],
  ['request',{terms:terms(1),expires_at:'600',quote:id(6)},buyer],
  ['decline',{request:id(6),reason},seller],
  ['terms_proposed',{base_agreement:id(7),terms:terms(2),expires_at:'600',reason},buyer],
  ['terms_authorized',{proposal:id(6),terms_digest:termsSuite.vectors[2].terms_digest,expires_at:'600'},seller],
  ['terms_rejected',{proposal:id(6),reason},seller],
  ['agreement_committed',{base_agreement:id(7),authorization:id(6),terms_digest:termsSuite.vectors[2].terms_digest,authority,seller_sequence:'1'},seller],
  ['withdrawal_requested',{authorization:id(6),reason},buyer],
  ['withdrawal_acknowledged',{withdrawal:id(6),seller_sequence:'1'},seller],
  ['cancellation_proposed',{agreement:id(7),residual:lines,reason,expires_at:'600'},buyer],
  ['cancellation_authorized',{proposal:id(6),expires_at:'600'},seller],
  ['residual_released',{proposal:id(6),authorization:id(8),agreement:id(7),residual:lines,authority,seller_sequence:'1'},seller],
  ['ready',{agreement:id(7),fulfillment_group:id(4),lines,window},seller],
  ['handover',{agreement:id(7),fulfillment_group:id(4),lines,measurements:[{line_id:id(5),quantity:{amount:'500',unit:'g'},line_total:{currency:'CAD',minor:'439',exponent:2}}],occurred_at:'440',authority,seller_sequence:'1'},seller],
  ['receipt',{handover:id(6),lines:[{line_id:id(5),amount:'0'}],supersedes:id(9),occurred_at:'440',note:reason},buyer],
  ['issue',{code:'quality',description:reason,lines,source_records:[id(8)]},buyer],
  ['resolution_proposed',{agreement:id(7),resolution,expires_at:'600'},buyer],
  ['resolution_authorized',{proposal:id(6),expires_at:'600'},seller],
  ['resolution_committed',{proposal:id(6),authorization:id(8),authority,seller_sequence:'1'},seller],
];
assert.equal(payloads.length,19);
const vectors=payloads.map(([type,payload,author])=>encode(header(type,payload,author)));
for(const [type,field] of [['quote','in_reply_to'],['request','quote'],['terms_proposed','base_agreement'],['agreement_committed','base_agreement'],['receipt','supersedes']]) {
  const [,payload,author]=payloads.find(row=>row[0]===type);
  const body=header(type,structuredClone(payload),author);body.payload[field]=null;
  vectors.push(encode(body,type+'_explicit_null'));
}
const previous=header('decline',{request:id(6),reason},seller);previous.author_sequence='2';previous.author_previous=id(12);
vectors.push(encode(previous,'decline_previous_author_record'));
const observed=header('receipt',{handover:id(6),lines:[{line_id:id(5),amount:'1'},{line_id:id(6),amount:'0'}],supersedes:null,occurred_at:'440',note:reason},buyer);
vectors.push(encode(observed,'receipt_two_lines'));
const reversed=structuredClone(observed);reversed.payload.lines.reverse();vectors.push(encode(reversed,'receipt_two_lines_reversed'));
const [,handoverPayload,handoverAuthor]=payloads.find(row=>row[0]==='handover');
const measured=header('handover',structuredClone(handoverPayload),handoverAuthor);
measured.payload.lines.push({line_id:id(6),amount:'1'});
measured.payload.measurements.push({line_id:id(6),quantity:{amount:'1',unit:'kg'},line_total:{currency:'CAD',minor:'878',exponent:2}});
vectors.push(encode(measured,'handover_two_measurements'));
const measuredReversed=structuredClone(measured);measuredReversed.payload.measurements.reverse();
vectors.push(encode(measuredReversed,'handover_two_measurements_reversed'));
for(const unit of ['kg','mL','L','each']) {
  const [,payload,author]=payloads.find(row=>row[0]==='handover');const body=header('handover',structuredClone(payload),author);
  body.payload.measurements[0].quantity.unit=unit;vectors.push(encode(body,'handover_unit_'+unit));
}
for(const code of ['shortage','substitution','collection','excess','other']) {
  const [,payload,author]=payloads.find(row=>row[0]==='issue');const body=header('issue',structuredClone(payload),author);
  body.payload.code=code;vectors.push(encode(body,'issue_code_'+code));
}
const limitTerms=termsSuite.vectors.find(v=>v.name==='exact_content_limit');assert(limitTerms);
const large=header('quote',{terms:JSON.parse(limitTerms.wire),expires_at:'600',in_reply_to:id(6)},seller);
let excess=encode(large).utf8_bytes-16384;
assert(excess>0);
for(const group of large.payload.terms.fulfillment){const remove=Math.min(excess,group.terms.instructions.length);group.terms.instructions=group.terms.instructions.slice(remove);excess-=remove;}
assert.equal(excess,0);
const exact=encode(large,'exact_private_record_limit');assert.equal(exact.utf8_bytes,16384);vectors.push(exact);
const overflow=structuredClone(large);
overflow.payload.terms.fulfillment.find(group=>group.terms.instructions.length<2048).terms.instructions+='a';
const invalid=encode(overflow,'private_record_limit_plus_one');assert.equal(invalid.utf8_bytes,16385);
const output=JSON.stringify({schema:'radroots.commerce.record.vectors.v1',producer:'independent ECMAScript canonicalization and Node.js SHA-256; synthetic data only',scope:'Closed complete record bytes/hash claims only; no authenticated parent history, consent, stock, physical truth, nested crypto or issuance qualification.',vectors,invalid:[{...invalid,expected:'limit_exceeded'}]},null,2)+'\n';
const target=new URL('../fixtures/commerce_wire.v1.json',import.meta.url);
if(process.argv.includes('--write'))writeFileSync(target,output);
else if(process.argv.includes('--check'))assert.equal(readFileSync(target,'utf8'),output,'Commerce fixture is stale');
else throw new Error('Use --write or --check');
