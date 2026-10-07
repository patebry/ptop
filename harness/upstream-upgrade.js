#!/usr/bin/env node
// Execute the installed upgrade.js itself; every process, stream and timer is fake.
const fs = require('fs'), path = require('path'), vm = require('vm');
const {EventEmitter} = require('events');
const root = process.env.PTOP_UPSTREAM_VTOP;
if (!root) throw Error('PTOP_UPSTREAM_VTOP is required');
const cases = [
  {name:'success',out:['link -> /fixture/vtop.js\n'],err:[],status:0},
  {name:'failed-with-path',out:['link -> /fixture/vtop.js\n'],err:['npm failed\n'],status:1},
  {name:'missing-path',out:['up to date\n'],err:[],status:0},
  {name:'missing-token',out:['vtop.js\n'],err:[],status:1},
];
const results=[];
for (const scenario of cases) {
  let now=0; const logs=[],effects=[],timers=[],processStub={};
  const child=new EventEmitter();child.stdout=new EventEmitter();child.stderr=new EventEmitter();
  const context={module:{exports:{}},console:{log:value=>logs.push([now,String(value)+'\n'])},process:processStub,
    setTimeout:(fn,delay)=>timers.push([fn,delay]),require: name=>{
      if (name==='sudo') return (args,opts)=>{effects.push(['sudo',args,opts]);return child;};
      if (name==='./package.json') return {name:'vtop',version:'0.6.1'};
      effects.push(['require',now,name===undefined?'undefined':name]);return {};
    }};
  vm.runInNewContext(fs.readFileSync(path.join(root,'upgrade.js'),'utf8'),context,{filename:'upgrade.js'});
  context.module.exports.install('vtop',[{theme:'nord'}]);
  scenario.out.forEach(chunk=>child.stdout.emit('data',Buffer.from(chunk)));
  scenario.err.forEach(chunk=>child.stderr.emit('data',Buffer.from(chunk)));
  child.emit('close',scenario.status);
  for (const [fn,delay] of timers){now+=delay;fn();}
  results.push({name:scenario.name,logs,effects,process:processStub});
}
process.stdout.write(JSON.stringify(results,null,2)+'\n');
