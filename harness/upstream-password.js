#!/usr/bin/env node
// Execute original node-sudo and read; only the child installer and pidof are fake.
const fs = require('fs'), vm = require('vm'), path = require('path');
const {createRequire} = require('module');
const upstream = process.env.PTOP_UPSTREAM_VTOP;
if (!upstream) throw Error('PTOP_UPSTREAM_VTOP is required');
const upstreamRequire = createRequire(path.resolve(upstream, 'app.js'));
const sudoPath = upstreamRequire.resolve('sudo');
const sudoRequire = createRequire(sudoPath);
const moduleObject = {exports:{}};
vm.runInNewContext(fs.readFileSync(sudoPath,'utf8'), {
  module:moduleObject, exports:moduleObject.exports, process, Buffer, console, setTimeout,
  require(name) {
    if(name==='inpath') return {sync:()=>process.env.PTOP_FAKE_SUDO};
    if(name==='pidof') return (_name,cb)=>cb(null,123);
    return sudoRequire(name);
  }
},{filename:'original-sudo.js'});
const child=moduleObject.exports(['npm','install','-g','vtop'],{cachePassword:false,prompt:'Password:',spawnOptions:{stdio:'inherit'}});
child.stdout.on('data',chunk=>console.log(chunk.toString()));
child.stderr.on('data',chunk=>console.log(chunk.toString()));
child.on('close',()=>console.log('DONE'));
