#!/usr/bin/env python3
"""Independent unbounded Python-integer oracle; deterministic full-width Rust inputs."""
from pathlib import Path
import argparse,random
ROOT=Path(__file__).resolve().parents[1]
def render():
 r=random.Random(0x5AB5C012);rows=['price,elapsed,period,gross,fee_bps,fee']
 for index in range(4096):
  period=r.randrange(1,2**32//24);terms=r.randrange(1,25);price=r.randrange(1,(2**128-1)//terms+1)
  elapsed=r.randrange(period*terms+1);bps=r.randrange(1001);gross=price*elapsed//period
  rows.append(f'{price},{elapsed},{period},{gross},{bps},{gross*bps//10000}')
 return '\n'.join(rows)+'\n'
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');a=p.parse_args();path=ROOT/'contracts/hub/tests/fixtures/proration.csv'
 if a.check:assert path.read_text()==render()
 else:path.write_text(render())
