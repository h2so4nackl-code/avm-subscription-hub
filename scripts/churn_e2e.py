#!/usr/bin/env python3
"""Prove reusable subscription slots and unique identifiers in the real AVM."""
import argparse,json,tempfile
from pathlib import Path
from e2e import ROOT,setup,plan,join,num,decode,withdraw

def run(sdk,output):
 output.parent.mkdir(parents=True,exist_ok=True)
 with tempfile.TemporaryDirectory(prefix='subhub-churn-') as temp:
  stand,keys,hub,receivers=setup(sdk,Path(temp))
  try:
   def c(method,body=b'',identity='user',expected=0,**kw):return stand.call(hub,'subscriptions.'+method,body,identity=identity,expected_status=expected,**kw)
   c('deposit',b'\0',value=1000000);c('plan.new',plan(0,100,100),identity='package-owner')
   next_id=1;gross_total=0;fee_total=0
   for batch in range(4):
    snapshots=[]
    for _ in range(16):
     d=decode(c('join',join(0,1,1)));sub=d['subs'][-1];assert sub['id']==next_id;snapshots.append(sub.copy());next_id+=1
    c('join',join(0,1,1),expected=4008)
    for sub in snapshots:
     # Python oracle is evaluated from the actual call's logical timestamp.
     call_time=stand.time+1;gross=min(100,100*(call_time-sub['start'])//100)
     gross_total+=gross;fee_total+=gross*250//10000
     d=decode(c('cancel',num(sub['id'],8)))
    assert not d['subs'];assert d['merchants'][0]['credit'][0]==gross_total-fee_total;assert d['fees'][0]==fee_total
   assert next_id==65 and d['next_id']==65
   c('cancel',num(1,8),expected=4011)
   c('withdraw.customer',withdraw(0,d['wallets'][0]['credit'][0]));c('withdraw.merchant',withdraw(0,gross_total-fee_total),identity='package-owner')
   if fee_total:c('withdraw.fees',withdraw(0,fee_total),identity='developer')
   final=decode(c('status'));assert final['liabilities']==[0,0] and final['withdrawn']==[1000000,0]
   inspection=stand.command('inspect');replay=stand.command('replay');assert replay['status']=='ok'
   for key in ['account_state_root','registry_root','cumulative_fee_shells','accepted_input_count']:assert replay[key]==inspection[key],key
   output.write_text(json.dumps({'status':'PASS','subscriptions_opened_and_closed':64,'maximum_concurrent_subscriptions':16,'oracle_gross':gross_total,'oracle_fee':fee_total,'accounting':final,'replay':replay,'journal':stand.log},indent=2)+'\n');print('Reusable-slot AVM journey: PASS (64 subscriptions, 16 concurrent, full exit, replay)')
  except Exception:
   output.write_text(json.dumps({'status':'FAIL','journal':stand.log},indent=2)+'\n');raise
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--sdk',type=Path,required=True);p.add_argument('--output',type=Path,default=ROOT/'evidence/avm-churn.json');a=p.parse_args();run(a.sdk.resolve(),a.output.resolve())
