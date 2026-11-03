#!/usr/bin/env python3
"""Real AVM maximum plans, merchants, subscriptions and settled callback history."""
import argparse,json,tempfile
from pathlib import Path
from e2e import ROOT,setup,plan,join,num,addr,decode,withdraw

def run(sdk,output):
 output.parent.mkdir(parents=True,exist_ok=True)
 with tempfile.TemporaryDirectory(prefix='subhub-capacity-') as temp:
  stand,keys,hub,receivers=setup(sdk,Path(temp))
  try:
   def c(method,body=b'',identity='user',expected=0,**kw):return stand.call(hub,'subscriptions.'+method,body,identity=identity,expected_status=expected,**kw)
   # Three stand signing identities are available; native tests cover eight wallets.
   for identity in ['developer','package-owner']:c('wallet.open',addr(receivers[0]),identity=identity)
   for n in [0x71,0x72,0x73]:c('merchant.add',bytes([n])*32+addr(receivers[1]),identity='developer')
   c('merchant.add',bytes([0x74])*32+addr(receivers[1]),identity='developer',expected=4008)
   for _ in range(8):c('plan.new',plan(0,1,1000000),identity='package-owner')
   c('plan.new',plan(0,1,1000000),identity='package-owner',expected=4008)
   c('deposit',b'\0',value=10000)
   for _ in range(16):c('join',join(0,1,1))
   c('join',join(0,1,1),expected=4008)
   stand.call(receivers[0],'subscriptions.receiver.reject',b'\1')
   for n in range(1,128):
    d=decode(c('withdraw.customer',withdraw(0,1)))
    assert d['attempts']==n and d['receipt_count']==n and not d['pending'] and d['withdrawn']==[0,0]
    assert len(d['subs'])==16 and len(d['plans'])==8 and len(d['merchants'])==4
   # Release all escrow without charge (zero integer proration) and exit before the cap.
   for n in range(1,17):c('cancel',num(n,8))
   stand.call(receivers[0],'subscriptions.receiver.reject',b'\0')
   final=decode(c('withdraw.customer',withdraw(0,10000)))
   assert final['attempts']==128 and final['receipt_count']==128 and final['liabilities']==[0,0]
   c('deposit',b'\0',value=1,expected=4008)
   inspection=stand.command('inspect');replay=stand.command('replay');assert replay['status']=='ok'
   for key in ['account_state_root','registry_root','cumulative_fee_shells','accepted_input_count']:assert replay[key]==inspection[key],key
   output.write_text(json.dumps({'status':'PASS','maximum_receipts':128,'peak_subscriptions':16,'peak_plans':8,'peak_merchants':4,'stand_wallets':3,'accounting':final,'replay':replay,'journal':stand.log},indent=2)+'\n');print('Capacity AVM journey: PASS (128 receipts, 16 active subscriptions, full exit and replay)')
  except Exception:
   output.write_text(json.dumps({'status':'FAIL','journal':stand.log},indent=2)+'\n');raise
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--sdk',type=Path,required=True);p.add_argument('--output',type=Path,default=ROOT/'evidence/avm-capacity.json');a=p.parse_args();run(a.sdk.resolve(),a.output.resolve())
