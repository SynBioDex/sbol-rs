"""Run timing cases and common-endpoint memory cases sequentially."""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent
subprocess.run([sys.executable,str(ROOT/"run.py")],check=True)
for trial in range(1,4):
    subprocess.run([sys.executable,str(ROOT/"run.py"),"--modes","ready","--warmup","0","--iters","1",
                    "--directory",f"memory/trial-{trial}"],check=True)
