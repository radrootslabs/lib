import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {cp,mkdir,readFile,readdir,writeFile} from 'node:fs/promises';
import {dirname,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const [generated,destination,tsc]=process.argv.slice(2);
assert(generated&&destination&&tsc);
const root=resolve(destination), source=resolve(generated), local=dirname(fileURLToPath(import.meta.url));
await mkdir(root,{recursive:false});
const pkg=resolve(root,'package'), consumer=resolve(root,'consumer');
await mkdir(pkg); await mkdir(consumer);
const files=['fixture.js','fixture.d.ts','fixture_bg.wasm','fixture_bg.wasm.d.ts'];
const hashes={};
for(const file of files){
  await cp(resolve(source,file),resolve(pkg,file));
  hashes[file]=createHash('sha256').update(await readFile(resolve(pkg,file))).digest('hex');
}
await writeFile(resolve(pkg,'package.json'),JSON.stringify({name:'@radroots/browser-feasibility',version:'0.0.0',private:true,
  description:'Synthetic authenticated host fixture for actual generated Radroots browser calls; not a general SDK distribution',
  type:'module',exports:{'.':{types:'./fixture.d.ts',import:'./fixture.js'}},files},null,2)+'\n');
function run(command,args,cwd){
  const result=spawnSync(command,args,{cwd,encoding:'utf8',maxBuffer:8*1024*1024});
  assert.equal(result.status,0,`${command} ${args.join(' ')}\n${result.stdout}\n${result.stderr}`);
  return result.stdout;
}
const archive=JSON.parse(run('npm',['pack','--json','--ignore-scripts'],pkg));
assert.equal(archive.length,1);
const tarball=resolve(pkg,archive[0].filename);
await writeFile(resolve(consumer,'package.json'),JSON.stringify({name:'neutral-radroots-browser-consumer',private:true,type:'module',
  dependencies:{'@radroots/browser-feasibility':`file:${tarball}`}},null,2)+'\n');
run('npm',['install','--offline','--ignore-scripts','--no-audit','--no-fund'],consumer);
const installed=resolve(consumer,'node_modules/@radroots/browser-feasibility');
for(const [file,hash] of Object.entries(hashes)) assert.equal(createHash('sha256').update(await readFile(resolve(installed,file))).digest('hex'),hash);
assert.deepEqual((await readdir(installed)).sort(),[...files,'package.json'].sort());
await cp(resolve(local,'consumer.ts'),resolve(consumer,'consumer.ts'));
await writeFile(resolve(consumer,'tsconfig.json'),JSON.stringify({compilerOptions:{target:'ES2022',module:'NodeNext',moduleResolution:'NodeNext',
  strict:true,skipLibCheck:false,noEmitOnError:true,lib:['ES2022','ESNext.Disposable','DOM'],outDir:'dist'},include:['consumer.ts']},null,2)+'\n');
const compiler=run(process.execPath,[resolve(tsc),'--version'],consumer).trim();
run(process.execPath,[resolve(tsc),'-p','.'],consumer);
const ssr=run(process.execPath,['--input-type=module','-e',`
for(const name of ['window','document','indexedDB','WebSocket','Worker']) Object.defineProperty(globalThis,name,{configurable:true,get(){throw Error('unexpected SSR host access: '+name);}});
const sdk=await import('@radroots/browser-feasibility');
if(typeof sdk.Client!=='function'||typeof sdk.CommandRequest!=='function'||typeof sdk.default!=='function') throw Error('missing actual exports');
console.log('inert SSR import passed');`],consumer).trim();
const report={scope:'packed neutral fixture, Rust-authenticated host injection; not a generic host factory or production SDK distribution',
  compiler,node:process.version,npm:run('npm',['--version'],consumer).trim(),archive:archive[0],hashes,ssr,
  consumer:resolve(consumer,'dist/consumer.js'),installed};
await writeFile(resolve(root,'pack.json'),JSON.stringify(report,null,2)+'\n');
console.log(JSON.stringify(report));
