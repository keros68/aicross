"""Real local subprocess regressions. No models, network, or credentials."""
import ctypes
import json
import os
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest import mock

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / 'references'))
import dispatch

class PiProcessTests(unittest.TestCase):
    def test_model_catalog_thinking_capabilities(self):
        with tempfile.TemporaryDirectory() as temp:
            catalog=Path(temp)/'.pi/agent/models-store.json'
            catalog.parent.mkdir(parents=True)
            catalog.write_text(json.dumps({'test':{'models':[{'id':'x','reasoning':True,'thinkingLevelMap':{'minimal':None,'low':None,'medium':None,'high':'high','max':'max'}}]}}),encoding='utf-8')
            with mock.patch.object(dispatch.Path,'home',return_value=Path(temp)):
                self.assertEqual(dispatch.pi_supported_thinking({'channel':'pi:test','model':'x'}),['off','high','max'])
                self.assertIsNone(dispatch.pi_supported_thinking({'channel':'pi:test','model':'missing'}))

    def run_child(self, code, text=None, timeout=3):
        return dispatch.run_process([sys.executable, '-c', code], None, timeout, text, pi_json=True)

    def test_cumulative_updates_filtered_before_retention(self):
        code="""import json
for i in range(1200):
 print(json.dumps({'type':'message_update','assistantMessage':{'content':'x'*(i*12)}}))
print(json.dumps({'type':'message_end','message':{'role':'assistant','model':'glm-5.3-flash','content':[{'type':'text','text':'done'}],'usage':{'input':1,'output':2}}}))
"""
        status, raw, err, failure=self.run_child(code)
        self.assertEqual(status,0)
        self.assertIsNone(failure)
        self.assertNotIn('message_update',raw)
        self.assertLess(len(raw),1000)
        self.assertIn('done',raw)
        self.assertIn('updates=1200',err)

    def test_large_unicode_stdin_is_complete_and_peer_is_set(self):
        text='第一行\n预算对照\n'*3000
        status,raw,err,failure=self.run_child("import sys,os,json; sys.stdin.reconfigure(encoding='utf-8'); s=sys.stdin.read(); print(json.dumps({'bytes':len(s.encode('utf-8')),'peer':os.environ.get('AI_CROSS_PEER')}))",text)
        self.assertIsNone(failure)
        self.assertEqual(status,0,err)
        self.assertEqual(json.loads(raw),{'bytes':len(text.encode('utf-8')),'peer':'1'})

    def test_timeout_is_bounded_and_keeps_complete_events(self):
        start=time.monotonic()
        status,raw,err,failure=self.run_child("import time; print('{\"type\":\"agent_start\"}',flush=True); time.sleep(60)",timeout=.4)
        self.assertIsNone(status)
        self.assertIn('超过',failure)
        self.assertIn('agent_start',raw)
        self.assertLess(time.monotonic()-start,4)

    @unittest.skipUnless(sys.platform=='win32','Windows process-tree regression')
    def test_windows_descendant_no_longer_holds_pipe(self):
        with tempfile.TemporaryDirectory() as temp:
            pidfile=Path(temp)/'grandchild.pid'
            code="import subprocess,sys,time; from pathlib import Path; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(60)']); Path(sys.argv[1]).write_text(str(p.pid)); time.sleep(60)"
            start=time.monotonic()
            status,raw,err,failure=dispatch.run_process([sys.executable,'-c',code,str(pidfile)],None,.8,pi_json=True)
            self.assertIsNotNone(failure)
            self.assertLess(time.monotonic()-start,4)
            pid=int(pidfile.read_text())
            kernel=ctypes.windll.kernel32
            kernel.OpenProcess.restype=ctypes.c_void_p
            handle=kernel.OpenProcess(0x100000,False,pid)
            if handle:
                try:self.assertNotEqual(kernel.WaitForSingleObject(ctypes.c_void_p(handle),0),258)
                finally:kernel.CloseHandle(ctypes.c_void_p(handle))

if __name__=='__main__':unittest.main()
