#!/usr/bin/env python3
"""Compare actual upstream and clone error/termination behavior with fake sensors."""
import os,sys,json,time,tempfile,pty,fcntl,struct,termios,subprocess,select,signal
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
UPSTREAM=Path(os.environ['PTOP_UPSTREAM_VTOP'])
BINARY=str(Path(sys.argv[1] if len(sys.argv)>1 else ROOT/'target/release/ptop').resolve())
COMMANDS=[['node',str(UPSTREAM/'bin/vtop.js')],[BINARY]]

def capture(command,failed=None,termination=None):
 with tempfile.TemporaryDirectory(prefix='ptop-failure-') as directory:
  root=Path(directory)
  body='''import os,sys
kind='process' if '-ewwwo' in sys.argv else 'memory'
if kind=='memory' and os.environ.get('FAKE_FAILURE')=='memory-empty':
 print('fixture memory failure',file=sys.stderr);sys.exit(7)
print('  %CPU %MEM COMM\\n  9.0 1.0 fixture-row' if kind=='process' else ('total used free\\nMem: 1000 100 900' if os.path.basename(sys.argv[0])=='free' else 'RSS COMM\\n1024 fixture'))
if os.environ.get('FAKE_FAILURE')==kind:
 print('fixture '+kind+' failure',file=sys.stderr);sys.exit(7)
'''
  for name in ['ps','free']:
   path=root/name;path.write_text('#!'+sys.executable+'\n'+body);path.chmod(0o755)
  npm=root/'npm';npm.write_text('#!'+sys.executable+'\nprint(\'{"dist-tags":{"latest":"0.6.1"}}\')');npm.chmod(0o755)
  master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,100,0,0));before=termios.tcgetattr(slave)
  env=dict(os.environ,PATH=str(root)+os.pathsep+os.environ['PATH'],TERM='xterm-256color',FAKE_FAILURE=failed or '')
  child=subprocess.Popen(command+['--update-interval','50','--no-mouse'],stdin=slave,stdout=slave,stderr=slave,env=env);output=bytearray()
  def drain(duration):
   end=time.monotonic()+duration
   while time.monotonic()<end:
    if select.select([master],[],[],min(.03,max(0,end-time.monotonic())))[0]:output.extend(os.read(master,65536))
  fatal=failed=='memory-empty' or (failed=='memory' and sys.platform=='darwin')
  try:
   deadline=time.monotonic()+4
   marker=(b'Error: Command failed:' if failed else b'fixture-row')
   while marker not in output and child.poll() is None and time.monotonic()<deadline:drain(.05)
   assert marker in output,bytes(output[-1500:])
   if fatal:
    child.wait(timeout=2);drain(.05);assert child.returncode==1,child.returncode
   else:
    assert child.poll() is None,'nonfatal sensor error exited'
    if failed:
     deadline=time.monotonic()+2
     while b'fixture-row' not in output and time.monotonic()<deadline:drain(.05)
     assert b'fixture-row' in output,'failed process stdout was not parsed'
     assert b'fixture '+failed.encode()+b' failure' in output
    if termination is None:os.write(master,b'q')
    elif isinstance(termination,list):
     for item in termination:
      try:os.kill(child.pid,item)
      except ProcessLookupError:break
    else:os.kill(child.pid,termination)
    child.wait(timeout=2);drain(.05);assert child.returncode==0,child.returncode
   assert b'\x1b[?1049l' in output,'alternate screen not restored'
   assert termios.tcgetattr(slave)==before,'terminal flags not restored'
   return child.returncode
  finally:
   if child.poll() is None:child.kill();child.wait()
   os.close(master);os.close(slave)
for failed in ['memory','memory-empty','process']:
 statuses=[capture(command,failed=failed) for command in COMMANDS]
 assert statuses[0]==statuses[1],statuses
 print('PASS:',failed,'exit7 error behavior, stdout handling and terminal restoration',statuses)
for termination in [signal.SIGTERM,signal.SIGINT,signal.SIGQUIT]*2:
 statuses=[capture(command,termination=termination) for command in COMMANDS]
 assert statuses==[0,0],statuses
 print('PASS:',signal.Signals(termination).name,'owned PTY termination exits0 and restores terminal')

for command in COMMANDS:
 assert capture(command)==0
 assert capture(command,termination=[signal.SIGTERM,signal.SIGINT,signal.SIGQUIT]*8)==0
print('PASS: normal q and repeated mixed termination burst restore terminal and exit0')
