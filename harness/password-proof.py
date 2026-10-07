#!/usr/bin/env python3
"""Compare original node-sudo/read with Rust using only synthetic passwords."""
import fcntl,json,os,pty,select,struct,subprocess,sys,tempfile,termios,time
from pathlib import Path
ROOT=Path(__file__).resolve().parent.parent
binary=str(Path(sys.argv[1] if len(sys.argv)>1 else ROOT/'target/release/ptop').resolve())
CASES={'plain':[b'fake-only\r'],'retry':[b'wrong\r',b'fake-only\r'],'cancel':[b'\x03'],'edit':[b'fake-bad\x7f\x7f\x7fonly\r'],'control':[b'bad\x15fake-only\r'],'arrows':[b'fake-onlX\x1b[D\x1b[3~y\r'],'unicode':['fake-é'.encode()+b'\x7fonly\r'],'word-delete':[b'fake/path\x17only\r'],'empty-eof':[b'\x04']}

def capture(original, answers):
 with tempfile.TemporaryDirectory(prefix='ptop-password-') as directory:
  root=Path(directory)
  scripts={
   'sudo':"import os,sys\nfrom pathlib import Path\nanswers=[]\nfor _ in range("+str(len(answers))+"):\n sys.stderr.write('#node-sudo-passwd#');sys.stderr.flush()\n answer=sys.stdin.readline()\n if not answer: sys.exit(0)\n answers.append(answer)\nPath(os.environ['FAKE_ANSWERS']).write_text(repr(answers))\nprint('link -> /fixture/vtop.js')",
   'npm':"print('{\"dist-tags\":{\"latest\":\"9.9.9\"}}')",
   'node':"pass",
   'ps':"print('  %CPU %MEM COMM\\n  9.0 1.0 fixture')"}
  for name,body in scripts.items():
   p=root/name;p.write_text('#!'+sys.executable+'\n'+body+'\n');p.chmod(0o755)
  master,slave=pty.openpty();fcntl.ioctl(slave,termios.TIOCSWINSZ,struct.pack('HHHH',24,160,0,0))
  env=dict(os.environ,PATH=str(root)+os.pathsep+os.environ['PATH'],TERM='xterm-256color',PTOP_FAKE_SUDO=str(root/'sudo'),FAKE_ANSWERS=str(root/'answers'))
  command=([subprocess.check_output(['which','node'],text=True).strip(),str(ROOT/'harness/upstream-password.js')] if original else [binary,'--vtop-parity','--update-interval','50','--quit-after','10'])
  child=subprocess.Popen(command,stdin=slave,stdout=slave,stderr=slave,env=env);output=bytearray();modes=[]
  def until(predicate):
   deadline=time.monotonic()+5
   while not predicate():
    assert time.monotonic()<deadline,bytes(output)
    if select.select([master],[],[],.1)[0]: output.extend(os.read(master,65536))
  try:
   if not original:
    # The centered load header can overwrite the notice prefix on a runner
    # with a long hostname. Its unique version token still signals that the
    # fake npm response was displayed; full notice layout has separate proofs.
    until(lambda:b'9.9.9' in output);os.write(master,b'u')
   for i,answer in enumerate(answers):
    until(lambda:output.count(b'#node-sudo-passwd#')>i)
    modes.append(termios.tcgetattr(slave)[:4]);os.write(master,answer)
   if answers==[b'\x04']:
    time.sleep(.2)
    while select.select([master],[],[],.05)[0]: output.extend(os.read(master,65536))
    assert child.poll() is None and not (root/'answers').exists(), 'EOF unexpectedly answered sudo'
    modes.append(termios.tcgetattr(slave)[:4])
    start=output.index(b'Password: ')-8
    return bytes(output[start:]), 'no answer; original leaves installer waiting',modes
   until(lambda:b'link -> /fixture/vtop.js' in output)
   child.wait(timeout=3)
   assert termios.tcgetattr(slave)[3]&termios.ICANON,'terminal not restored'
   start=output.index(b'Password: ');
   if output[max(0,start-8):start]==b'\x1b[1G\x1b[0J': start-=8
   end=output.index(b'link -> /fixture/vtop.js')
   return bytes(output[start:end]),(root/'answers').read_text(),modes
  finally:
   if child.poll() is None:child.kill();child.wait()
   os.close(master);os.close(slave)
for name,answers in CASES.items():
 expected=capture(True,answers);actual=capture(False,answers)
 assert actual==expected,(name,expected,actual)
 print('PASS:',name,'original node-sudo prompt bytes, fake answers, terminal modes and restoration')
