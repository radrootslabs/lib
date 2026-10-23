// Independent synthetic public protocol fixtures. Scalars and auxiliary bytes
// are public test data, with no runtime custody or production entropy claim.
import {createHash} from 'node:crypto';
import {readFileSync,writeFileSync} from 'node:fs';
import assert from 'node:assert/strict';
import {getEventHash,getPublicKey,verifyEvent} from 'nostr-tools/pure';
import {schnorr} from '@noble/curves/secp256k1.js';
const secret=Buffer.from('00'.repeat(31)+'03','hex');
assert.equal(Buffer.from(schnorr.sign(new Uint8Array(32),secret,new Uint8Array(32))).toString('hex').toUpperCase(),'E907831F80848D1069A5371B402410364BDF1C5F8307B0084C55F1CE2DCA821525F66A4A85EA8B71E482A74F382D2CE5EBEEE8FDB2172F477DF4900D310536C0');
const author=getPublicKey(secret), d='01'.repeat(16), option='02'.repeat(32);
const canonical=v=>v===null||typeof v!=='object'?JSON.stringify(v):Array.isArray(v)?'['+v.map(canonical).join(',')+']':'{'+Object.keys(v).sort().map(k=>JSON.stringify(k)+':'+canonical(v[k])).join(',')+'}';
function sign(kind,tags,content,created_at=100){const e={pubkey:author,created_at,kind,tags,content};const id=getEventHash(e);const signed={id,...e,sig:Buffer.from(schnorr.sign(Buffer.from(id,'hex'),secret,new Uint8Array(32))).toString('hex')};assert(verifyEvent(JSON.parse(JSON.stringify(signed))));return signed;}
function profile(price='fixed',state='active') {
  const rate={currency:'CAD',minor:'325',exponent:2};
  return {profile:'radroots.listing',version:1,state,product:{name:'Carrots 🥕',description:'Independent public conformance fixture',producer:author,origin_summary:'Example region'},default_option:option,options:[{id:option,label:'One bunch',unit:{type:'package',label:'bunch',contents:{type:'unspecified'}},increment:'1',minimum:'1',price:price==='fixed'?{type:price,rate,basis:'1',discount_bps:0,tiers:[]}:price==='variable_weight'?{type:price,measurement_unit:'kg',minimum_per_sale_unit:'1',maximum_per_sale_unit:'2',rate,basis:'1',maximum_per_sale_unit_total:{...rate,minor:'650'}}:{type:'quote_required'},supply:{type:'unknown'},lead_seconds:'0'}],fulfillment:{modes:['pickup'],area:'Example region',lead_seconds:'0'},authority:null,media:[]};
}
function listing(p=profile(),created=100,mutate=tags=>tags){const tags=[['d',d],['title','Carrot bunch'],['summary',p.state==='active'?'One bunch; CAD 3.25 per bunch':'Unavailable: '+p.state],['published_at','99'],['location','Example region'],['radroots:price_unit','bunch'],['radroots:listing',canonical(p)]];if(p.state==='active')tags.push(['status','active']);if(p.options[0].price.type==='fixed')tags.push(['price','3.25','CAD']);return sign(30402,mutate(tags),'Synthetic carrots; availability: '+p.state,created);}
const rows=[];
const add=(name,e,expected)=>{rows.push({name,raw:JSON.stringify(e),expected});return e;};
const base=add('listing_fixed',listing(),'listing');
const large=profile();large.product.description='x'.repeat(4000);add('listing_large_profile',listing(large),'listing');
const exactBase=listing(large);const emptyExact=sign(30402,exactBase.tags,'');
const available=65536-Buffer.byteLength(JSON.stringify(emptyExact));
const exactContent='🥬'.repeat(Math.floor(available/4))+'x'.repeat(available%4);
assert([...exactContent].length<=16384);
const exactEvent=sign(30402,exactBase.tags,exactContent);assert.equal(Buffer.byteLength(JSON.stringify(exactEvent)),65536);
add('listing_exact_event_bound',exactEvent,'listing');
add('listing_shuffled',listing(profile(),100,t=>t.reverse()),'listing');
for(const [name,amount] of [['extra_zero','3.250'],['extra_zeros','3.2500']])add('listing_equivalent_price_'+name,listing(profile(),100,t=>t.map(v=>v[0]==='price'?['price',amount,'CAD']:v)),'listing');
add('listing_quote',listing(profile('quote_required')),'listing');
add('listing_variable',listing(profile('variable_weight')),'listing');
for(const state of ['paused','withdrawn'])add('listing_'+state,listing(profile('fixed',state),101),'listing');
const future=profile();future.version=2;add('listing_unsupported',listing(future,101),'unsupported');
const invalid=profile();invalid.extra=true;add('listing_unknown_field',listing(invalid,102),'invalid');
add('listing_duplicate_d',listing(profile(),103,t=>[...t,['d','ff'.repeat(16)]]),'invalid');
add('listing_ambiguous_d',listing(profile(),103,t=>t.map(v=>v[0]==='d'?[...v,'ambiguous']:v)),'invalid');
add('listing_contradictory_price',listing(profile(),104,t=>t.map(v=>v[0]==='price'?['price','0','CAD']:v)),'invalid');
add('listing_recurring_price',listing(profile(),104,t=>t.map(v=>v[0]==='price'?[...v,'month']:v)),'invalid');
add('listing_duplicate_profile',listing(profile(),104,t=>[...t,t.find(v=>v[0]==='radroots:listing')]),'invalid');
const badSig={...base,created_at:105,sig:'00'.repeat(64)};badSig.id=getEventHash(badSig);add('bad_signature',badSig,'unauthenticated');
add('bad_id',{...base,id:'00'.repeat(32)},'unauthenticated');
const cap={profile:'radroots.capabilities',version:1,supported_profiles:[{profile:'radroots.commerce',versions:[1,65535],features:['base-order-v1','bounded-variable-weight-v1']}],valid_until:'200'};
const capability=(body=cap,created=100,tags=[])=>sign(18890,tags,canonical(body),created);
add('capability',capability(),'capabilities');
add('capability_unsupported',capability({profile:'radroots.capabilities',version:2},101),'unsupported');
add('capability_unknown_feature',capability({...cap,supported_profiles:[{...cap.supported_profiles[0],features:['future-v2']}]},102),'unsupported');
add('capability_duplicate_versions',capability({...cap,supported_profiles:[{...cap.supported_profiles[0],versions:[1,1]}]},102),'invalid');
add('capability_duplicate_profiles',capability({...cap,supported_profiles:[...cap.supported_profiles,...cap.supported_profiles]},102),'capabilities');
add('capability_all_profiles',capability({...cap,supported_profiles:[...cap.supported_profiles,{profile:'radroots.listing',versions:[1,2,3,4,5,6,7,65535],features:['listing-options-v1']},{profile:'radroots.chat',versions:[1],features:['plaintext-inquiry-v1']}]}),'capabilities');
add('capability_tag',capability(cap,102,[['d','not-a-coordinate']]),'invalid');
add('capability_too_long',capability({...cap,valid_until:String(100+2592001)}),'invalid');
add('capability_exact_thirty_days',capability({...cap,valid_until:String(100+2592000)}),'capabilities');
add('capability_expired',capability({...cap,valid_until:'120'},110),'expired');
add('listing_future',listing(profile(),1000),'clock_ahead');
add('capability_future',capability({...cap,valid_until:'1200'},1000),'clock_ahead');
const output={schema:'radroots.public_heads.vectors.v1',producer:'Pinned nostr-tools2.23.5 and NobleCurves2.0.1; BIP340 primary vector0 matched first; independent synthetic test data only',author,d,observed_at:150,vectors:rows};
const path=new URL('../../crates/nostr/tests/fixtures/public_heads.v1.json',import.meta.url);
const bytes=JSON.stringify(output,null,2)+'\n';
if(process.argv.includes('--check'))assert.equal(readFileSync(path,'utf8'),bytes);else writeFileSync(path,bytes);
const projection={schema:'radroots.public_profiles.vectors.v1',producer:output.producer,source_fixture_sha256:createHash('sha256').update(bytes).digest('hex'),vectors:rows.filter(r=>!['bad_signature','bad_id'].includes(r.name)).map(r=>({...r,expected:r.expected==='clock_ahead'?(JSON.parse(r.raw).kind===30402?'listing':'capabilities'):r.expected==='expired'?'capabilities':r.expected}))};
const largeWire=rows.find(r=>r.name==='listing_exact_event_bound').raw;
for(const [relative,value] of [['../../crates/protocol/tests/fixtures/public_profiles.v1.json',projection],['../../crates/transport_nostr/tests/fixtures/public_large.v1.json',{schema:'radroots.transport_nostr.public_large.v1',producer:output.producer,source_fixture_sha256:projection.source_fixture_sha256,author,d,raw:largeWire}]]) {
  const target=new URL(relative,import.meta.url), projected=JSON.stringify(value,null,2)+'\n';
  if(process.argv.includes('--check'))assert.equal(readFileSync(target,'utf8'),projected);else writeFileSync(target,projected);
}
console.log(JSON.stringify({vectors:rows.length,sha256:createHash('sha256').update(bytes).digest('hex'),check:process.argv.includes('--check')}));
