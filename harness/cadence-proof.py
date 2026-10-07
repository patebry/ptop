#!/usr/bin/env python3
"""Real upstream/app PTYs with slow fake ps: timer cadence, overlap and sort wake."""
import os,sys,json,time,tempfile,pty,fcntl,struct,termios,subprocess,select
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
UPSTREAM=Path(os.environ['PTOP_UPSTREAM_VTOP'])
BINARY=str(Path(sys.argv[1] if len(sys.argv)>1 else ROOT/'target/release/ptop').resolve())

def capture(command):
 with tempfile.TemporaryDirectory(prefix='ptop-cadence-') as directory:
  root=Path(directory);log=root/'polls.jsonl'
  body='''import os,sys,time,json
kind='process' if '-ewwwo' in sys.argv else 'memory'
def record(event):
 fd=os.open(os.environ['FAKE_POLL_LOG'],os.O_WRONLY|os.O_APPEND|os.O_CREAT,0o600)
 os.write(fd,(json.dumps([kind,event,time.monotonic(),os.getpid()])+'\\n').encode());os.close(fd)
record('start');time.sleep(2.4 if kind=='process' else .35);record('end')
print('  %CPU %MEM COMM\\n  9.0 1.0 fixture' if kind=='process' else ('total used free\\nMem: 1000 100 900' if os.path.basename(sys.argv[0])=='free' else 'RSS COMM\\n1024 fixture'))
'''
  for name in ('ps','free'):
   fake=root/name;fake.write_text('#!'+sys.executable+'\n'+body);fake.chmod(0o755)
  npm=root/'npm';npm.write_text('#!'+sys.executable+'\nprint(\'{"dist-tags":{"latest":"0.6.1"}}\')\n');npm.chmod(0o755)
  master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,100,0,0))
  env=dict(os.environ,PATH=str(root)+os.pathsep+os.environ['PATH'],TERM='xterm-256color',FAKE_POLL_LOG=str(log))
  child=subprocess.Popen(command,stdin=slave,stdout=slave,stderr=slave,env=env)
  output=bytearray()
  def drain(duration):
   deadline=time.monotonic()+duration
   while time.monotonic()<deadline:
    if select.select([master],[],[],min(.05,max(0,deadline-time.monotonic())))[0]:output.extend(os.read(master,65536))
  try:
   deadline=time.monotonic()+5;origin=None
   while origin is None:
    drain(.03);assert time.monotonic()<deadline,'initial poll missing'
    rows=[json.loads(line) for line in log.read_text().splitlines()] if log.exists() else []
    origin=next((r[2] for r in rows if r[:2]==['process','start']),None)
   drain(max(0,origin+3.1-time.monotonic()));os.write(master,b'm')
   drain(max(0,origin+6.3-time.monotonic()));os.write(master,b'q');drain(.15)
   assert child.wait(timeout=2)==0,bytes(output[-500:])
   rows=[json.loads(line) for line in log.read_text().splitlines()]
   for kind,interval in [('memory',.2),('process',2.)]:
    starts=[r[2] for r in rows if r[:2]==[kind,'start']]
    ends=[r[2] for r in rows if r[:2]==[kind,'end']]
    assert starts[1]<ends[0],(kind,'slow polls did not overlap',starts,ends)
    relative=[t-starts[0] for t in starts]
    for expected in ([.2,.4,.6,1.,2.,4.,6.] if kind=='memory' else [3.1,relative[1]+2,relative[1]+4]):
     assert any(abs(t-expected)<.16 for t in relative),(kind,expected,relative)
    if kind=='memory':
     # These timestamps are recorded after launching a Python subprocess, not
     # when the scheduler dispatches it. Loaded CI can delay one start and then
     # run the next promptly. Check cumulative phase instead of treating a
     # single delayed launch as a slower polling interval. The two-interval
     # allowance is fixed for the entire run, not accumulated per sample: a
     # work-then-sleep loop (350ms work +200ms sleep) fails within three polls.
     phase_errors=[t-index*interval for index,t in enumerate(relative)]
     assert len(starts)>=28,('too few memory polls',relative)
     assert max(abs(error) for error in phase_errors)<.4,('memory cadence drift',phase_errors)
     print('memory cadence:',round(relative[-1]/(len(starts)-1),3),'seconds mean,',
           round(max(abs(error) for error in phase_errors),3),'seconds maximum cumulative phase error')
    print('PASS:',Path(command[0]).name,kind,'fixed cadence/overlap',len(starts),'polls')
  finally:
   if child.poll() is None:child.kill();child.wait()
   os.close(master);os.close(slave)
   # Let only our short-lived fake sensor children finish before removing their log.
   time.sleep(2.5)

capture(['node',str(UPSTREAM/'bin/vtop.js'),'--update-interval','300','--no-mouse'])
capture([BINARY,'--update-interval','300','--no-mouse'])
