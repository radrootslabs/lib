// Independent portable scope descriptors over original synthetic signed inputs.
// This producer never calls a Radroots coverage or record implementation.
import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
const root=new URL('../../',import.meta.url),sourcePath='crates/storage_sqlite/tests/fixtures/stock_history.v1.json';
const sourceBytes=readFileSync(new URL(sourcePath,root));const source=JSON.parse(sourceBytes);
const sha=value=>createHash('sha256').update(value).digest('hex');
const records=Object.fromEntries(source.sources.filter(row=>row.record_id!==null).map(row=>[row.name,{id:row.record_id,wire:JSON.parse(row.raw_rumor).content}]));
const chat=source.sources.find(row=>row.name==='chat').rumor_id;
const future=source.sources.find(row=>row.name==='future_commerce').rumor_id;
const unique=values=>[...new Set(values)].sort();
function checkpoint(names,external) {
  const known=unique(names.map(name=>records[name].id));
  const required=unique(names.flatMap(name=>JSON.parse(records[name].wire).parents));
  const used=unique(external.filter(id=>required.includes(id)&&!known.includes(id)));
  const hash=createHash('sha256').update(Buffer.from('radroots.commerce.coverage.v1\0'));
  for(const key of ['thread','order','buyer','seller'])hash.update(Buffer.from(source.scope[key],'hex'));
  for(const identities of [known,used,[]]) {
    const count=Buffer.alloc(8);count.writeBigUInt64BE(BigInt(identities.length));hash.update(count);
    for(const id of identities) {assert.match(id,/^[0-9a-f]{64}$/);hash.update(Buffer.from(id,'hex'));}
  }
  return {digest:hash.digest('hex'),sequence:known.length+used.length};
}
const external=[source.listing_id,chat];
const base=checkpoint(['quote','request'],external);
const vectors=[
  {name:'complete',records:['quote','request'],external,uninterpreted:[],readiness:'complete',checkpoint:base},
  {name:'duplicates_and_unrelated_external',records:['request','quote','request'],external:[...external,...external,'cd'.repeat(32)],uninterpreted:[],readiness:'complete',checkpoint:base},
  {name:'withdrawal_is_relevant_evidence',records:['quote','request','withdrawal'],external,uninterpreted:[],readiness:'complete',checkpoint:checkpoint(['quote','request','withdrawal'],external)},
  {name:'seller_order_head_is_relevant',records:['quote','request','decline'],external,uninterpreted:[],readiness:'complete',checkpoint:checkpoint(['quote','request','decline'],external)},
  {name:'missing_chat',records:['quote','request'],external:[source.listing_id],uninterpreted:[],readiness:'missing_history',checkpoint:null},
  {name:'missing_quote',records:['request'],external,uninterpreted:[],readiness:'missing_history',checkpoint:null},
  {name:'author_fork',records:['quote','request','withdrawal','buyer_fork'],external,uninterpreted:[],readiness:'conflicted',checkpoint:null},
  {name:'uninterpreted',records:['quote','request'],external,uninterpreted:[future],readiness:'uninterpreted',checkpoint:null},
  {name:'empty_is_data_only',records:[],external:[],uninterpreted:[],readiness:'complete',checkpoint:checkpoint([],[])},
];
const output={schema_version:1,transcript:'radroots.commerce.coverage.v1: NUL domain separator, scope thread/order/buyer/seller each32bytes, then known/required available external/uninterpreted sets eachu64BE count followed by sorted unique32byte identities',producer_sha256:sha(readFileSync(new URL(import.meta.url))),source_path:sourcePath,source_sha256:sha(sourceBytes),scope:source.scope,records,vectors};
const bytes=JSON.stringify(output,null,2)+'\n',target=new URL('crates/protocol/tests/fixtures/commerce_coverage.v1.json',root);
const args=process.argv.slice(2);assert(args.length===1&&['--write','--check'].includes(args[0]));
if(args[0]==='--write')writeFileSync(target,bytes);else assert.equal(readFileSync(target,'utf8'),bytes);
console.log(JSON.stringify({vectors:vectors.length,sha256:sha(bytes),bytes:Buffer.byteLength(bytes),check:args[0]==='--check'}));
