import { createHash } from 'node:crypto';
const author = '79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798';
const vectors = [ ['public_event',0,null,'1700000120000'], ['business_seal',1,author,'1700000120000'], ['gift_wrap',2,author,'253402300799999'] ].map(([role,tag,recipient,deadline])=>{
 const operation='11'.repeat(32),command='22'.repeat(32),plan='33'.repeat(32),time=Buffer.alloc(8);time.writeBigUInt64BE(BigInt(deadline));
 const parts=[Buffer.from('radroots.exact_signing_intent.v1\0'),Buffer.from(operation,'hex'),Buffer.from(command,'hex'),Buffer.from(plan,'hex'),Buffer.from([tag]),Buffer.from([recipient===null?0:1])];
 if(recipient!==null)parts.push(Buffer.from(recipient,'hex'));parts.push(time);
 return {operation,command,plan,role,recipient,deadline,digest:createHash('sha256').update(Buffer.concat(parts)).digest('hex')};
});
process.stdout.write(JSON.stringify({schema:'radroots.exact_signing_intent.v1.independent_vectors',oracle:process.version,vectors},null,2)+'\n');
