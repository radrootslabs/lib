import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdirSync,readFileSync,readdirSync,writeFileSync} from 'node:fs';
import {dirname,join,resolve} from 'node:path';
import {fileURLToPath} from 'node:url';

const options=new Map();for(let i=2;i<process.argv.length;i+=2)options.set(process.argv[i],resolve(process.argv[i+1]));
for(const name of ['--out','--library','--generator','--kotlinc','--jna','--java'])assert(options.has(name),name);
const out=options.get('--out'),fixture=dirname(fileURLToPath(import.meta.url));mkdirSync(out,{recursive:false});
const hash=path=>createHash('sha256').update(readFileSync(path)).digest('hex');
const files=(root,prefix='')=>readdirSync(root,{withFileTypes:true}).flatMap(entry=>entry.isDirectory()?files(join(root,entry.name),join(prefix,entry.name)):[join(prefix,entry.name)]).sort();
const library=options.get('--library'),generator=options.get('--generator');
const report={state:'running',scope:'actual retained native SDK memory/field/lifecycle bridge; no participant, generic factory or application qualification',library:{path:library,sha256:hash(library)},generator:{path:generator,sha256:hash(generator)},commands:[]};
function persist(){writeFileSync(join(out,'report.json'),JSON.stringify(report,null,2)+'\n');}
function run(command,args){
    const start=performance.now(),result=spawnSync(command,args,{encoding:'utf8',maxBuffer:8*1024*1024,timeout:240000});
    const log=join(out,`${report.commands.length}.log`);writeFileSync(log,(result.stdout||'')+(result.stderr||''));
    const row={command:[command,...args],exit_code:result.status,signal:result.signal,error:result.error?.message,seconds:(performance.now()-start)/1000,log,sha256:hash(log)};
    report.commands.push(row);persist();assert(!result.error&&result.status===0,readFileSync(log,'utf8'));console.log(JSON.stringify(row));
}
try{
    run(generator,['--version']);run('/usr/bin/swiftc',['--version']);run(options.get('--kotlinc'),['-version']);
    for(const language of ['swift','kotlin']){
        const first=join(out,language),second=first+'_repeat';
        for(const target of [first,second])run(generator,['generate','--library',library,'--language',language,'--no-format','--out-dir',target]);
        assert.deepEqual(files(first),files(second));for(const file of files(first))assert.equal(hash(join(first,file)),hash(join(second,file)));
        report[language]=Object.fromEntries(files(first).map(file=>[file,hash(join(first,file))]));
    }
    const swift=join(out,'swift'),executable=join(out,'swift_smoke');
    run('/usr/bin/swiftc',['-swift-version','6','-warnings-as-errors','-parse-as-library','-I',swift,'-Xcc',`-fmodule-map-file=${join(swift,'radroots_sdkFFI.modulemap')}`,join(swift,'radroots_sdk.swift'),join(fixture,'Smoke.swift'),library,'-o',executable]);
    run(executable,[]);
    const coroutine=resolve(options.get('--kotlinc'),'../../lib/kotlinx-coroutines-core-jvm.jar');
    const classpath=[options.get('--jna'),coroutine].join(':'),jar=join(out,'kotlin_smoke.jar');
    report.runtimeJars=Object.fromEntries([options.get('--jna'),coroutine].map(path=>[path,hash(path)]));
    run(options.get('--kotlinc'),[join(out,'kotlin/uniffi/radroots_sdk/radroots_sdk.kt'),join(fixture,'Smoke.kt'),'-Werror','-jvm-target','21','-classpath',classpath,'-include-runtime','-d',jar]);
    run(options.get('--java'),[`-Djna.library.path=${dirname(library)}`,'-cp',[jar,classpath].join(':'),'SmokeKt']);
    report.state='passed';
}finally{persist();}
