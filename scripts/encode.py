#!/usr/bin/env python3
"""Public ABI argument encoder. No credentials, signing or network access."""
import argparse,json
from pathlib import Path

def u(value,size):
 n=int(value)
 if not 0<=n<1<<(size*8):raise ValueError('integer outside unsigned field range')
 return n.to_bytes(size,'little')
def hexbytes(value,n):
 b=bytes.fromhex(value.removeprefix('0x').replace(':',''))
 if len(b)!=n or not any(b):raise ValueError(f'expected nonzero {n}-byte hex value')
 return b

def main():
 p=argparse.ArgumentParser(description=__doc__);sub=p.add_subparsers(dest='cmd',required=True)
 c=sub.add_parser('config');c.add_argument('admin');c.add_argument('treasury');c.add_argument('--assets',nargs=2,type=int,default=[0,7]);c.add_argument('--fee-bps',type=int,default=250);c.add_argument('--fuel',type=int,default=10000)
 w=sub.add_parser('wallet');w.add_argument('destination')
 m=sub.add_parser('merchant');m.add_argument('owner');m.add_argument('destination')
 n=sub.add_parser('plan');n.add_argument('asset',type=int);n.add_argument('price');n.add_argument('period')
 j=sub.add_parser('join');j.add_argument('plan');j.add_argument('version');j.add_argument('periods')
 x=sub.add_parser('withdraw');x.add_argument('asset',type=int);x.add_argument('amount')
 i=sub.add_parser('id');i.add_argument('id')
 e=sub.add_parser('more');e.add_argument('id');e.add_argument('periods')
 o=sub.add_parser('offer');o.add_argument('id');o.add_argument('award');o.add_argument('until');o.add_argument('digest')
 a=sub.add_parser('accept');a.add_argument('id');a.add_argument('nonce');a.add_argument('digest')
 r=sub.add_parser('inspect');r.add_argument('receipt',type=Path)
 a=p.parse_args()
 if a.cmd=='config':
  if not a.assets[0]<a.assets[1] or 2 in a.assets or not 0<=a.fee_bps<=1000 or a.fuel<=0:raise ValueError('invalid asset/fee/fuel configuration')
  b=hexbytes(a.admin,32)+hexbytes(a.treasury,64)+b''.join(u(n,4) for n in a.assets)+u(a.fee_bps,2)+u(a.fuel,16)
 elif a.cmd=='wallet':b=hexbytes(a.destination,64)
 elif a.cmd=='merchant':b=hexbytes(a.owner,32)+hexbytes(a.destination,64)
 elif a.cmd=='plan':
  if a.asset not in [0,1] or int(a.price)<=0 or int(a.period)<=0:raise ValueError('invalid plan')
  b=u(a.asset,1)+u(a.price,16)+u(a.period,4)
 elif a.cmd=='join':
  if not 1<=int(a.periods)<=24:raise ValueError('period count outside 1..24')
  b=u(a.plan,1)+u(a.version,4)+u(a.periods,1)
 elif a.cmd=='withdraw':
  if a.asset not in [0,1] or int(a.amount)<=0:raise ValueError('invalid withdrawal')
  b=u(a.asset,1)+u(a.amount,16)
 elif a.cmd=='id':b=u(a.id,8)
 elif a.cmd=='more':b=u(a.id,8)+u(a.periods,1)
 elif a.cmd=='offer':b=u(a.id,8)+u(a.award,16)+u(a.until,4)+hexbytes(a.digest,32)
 elif a.cmd=='accept':b=u(a.id,8)+u(a.nonce,4)+hexbytes(a.digest,32)
 else:
  from e2e import decode
  d=decode(json.loads(a.receipt.read_text()));print(json.dumps({k:d[k] for k in ['deposited','withdrawn','liabilities','fees','next_id','attempts','receipt_count','pending','wallets','merchants','subs']},indent=2));return
 print(json.dumps('0x'+b.hex()))
if __name__=='__main__':main()
