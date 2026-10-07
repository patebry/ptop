#!/usr/bin/env node
// Compare original upgrade.js resolution/process state with the standalone restart adapter.
const fs=require('fs'),os=require('os'),path=require('path'),vm=require('vm');
const {EventEmitter}=require('events'),{spawnSync}=require('child_process');
const upstream=process.env.PTOP_UPSTREAM_VTOP;
if(!upstream)throw Error('PTOP_UPSTREAM_VTOP is required');
const directory=fs.mkdtempSync(path.join(os.tmpdir(),'ptop-restart-'));
const packageRoot=path.join(directory,'global','vtop');
fs.mkdirSync(path.join(packageRoot,'node_modules','fixture'),{recursive:true});
const report="module.exports={argv:process.argv.slice(1),theme:process[0]};";
fs.writeFileSync(path.join(packageRoot,'relative-vtop.js'),report);
fs.writeFileSync(path.join(packageRoot,'node_modules','fixture','vtop.js'),report);
const npm=path.join(directory,'npm');
fs.writeFileSync(npm,`#!/bin/sh\nprintf '%s\\n' '${path.join(directory,'global')}'\n`,{mode:0o755});
const originalArgv=process.argv;
const desiredArgv=['/original/monitor','--theme','nord','--update-interval','517'];
try {
 for(const target of [path.join(packageRoot,'relative-vtop.js'),'./relative-vtop.js','fixture/vtop.js']) {
  process.argv=[process.execPath,...desiredArgv];delete process[0];
  const child=new EventEmitter();child.stdout=new EventEmitter();child.stderr=new EventEmitter();
  let observed;
  const localRequire=require('module').createRequire(path.join(packageRoot,'upgrade.js'));
  const context={module:{exports:{}},process,console:{log(){}},setTimeout:fn=>fn(),require:name=>{
   if(name==='sudo')return ()=>child;
   observed=localRequire(name);return observed;
  }};
  vm.runInNewContext(fs.readFileSync(path.join(upstream,'upgrade.js'),'utf8'),context);
  context.module.exports.install('vtop',[{theme:'nord'}]);
  child.stdout.emit('data',Buffer.from('link -> '+target+'\n'));child.emit('close',0);
  // Modules deliberately export observations instead of starting any real application.
  const adapter=fs.readFileSync(path.join(__dirname,'../src/restart.js'),'utf8');
  const capture=adapter.replace('load(state.undefined ? undefined : state.module);','console.log(JSON.stringify(load(state.undefined ? undefined : state.module)));');
  const state={argv:desiredArgv,theme:'nord',module:target,undefined:false};
  const result=spawnSync(process.execPath,['-e',capture,JSON.stringify(state)],{encoding:'utf8',cwd:directory,env:{...process.env,PATH:directory+path.delimiter+process.env.PATH}});
  if(result.status!==0)throw Error(result.stderr);
  require('assert').deepStrictEqual(JSON.parse(result.stdout),JSON.parse(JSON.stringify(observed)));
  console.log('PASS: original updater argv/process property and module resolution:',path.isAbsolute(target)?'absolute':target);
 }
}finally{process.argv=originalArgv;delete process[0];fs.rmSync(directory,{recursive:true,force:true});}
