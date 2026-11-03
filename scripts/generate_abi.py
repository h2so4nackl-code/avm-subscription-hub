#!/usr/bin/env python3
from pathlib import Path
import argparse,hashlib
ROOT=Path(__file__).resolve().parents[1]
METHODS={
 'STATUS':('subscriptions.status',None),'TOPUP':('subscriptions.topup',None),'PAUSE':('subscriptions.pause',1),
 'ADMIN_PROPOSE':('subscriptions.admin.propose',32),'ADMIN_ACCEPT':('subscriptions.admin.accept',None),
 'WALLET_OPEN':('subscriptions.wallet.open',64),'MERCHANT_ADD':('subscriptions.merchant.add',96),
 'PLAN_NEW':('subscriptions.plan.new',21),'PLAN_EDIT':('subscriptions.plan.edit',26),'DEPOSIT':('subscriptions.deposit',1),
 'JOIN':('subscriptions.join',6),'EXTEND':('subscriptions.extend',9),'COLLECT':('subscriptions.collect',9),
 'CANCEL':('subscriptions.cancel',8),'OFFER':('subscriptions.offer',60),'OFFER_ACCEPT':('subscriptions.offer.accept',44),'OFFER_REVOKE':('subscriptions.offer.revoke',8),
 'WITHDRAW_CUSTOMER':('subscriptions.withdraw.customer',17),'WITHDRAW_MERCHANT':('subscriptions.withdraw.merchant',17),'WITHDRAW_FEES':('subscriptions.withdraw.fees',17),
 'CALLBACK':('subscriptions.callback',80),'BIND':('subscriptions.receiver.bind',64),'REJECT':('subscriptions.receiver.reject',1),'RECEIVE':('subscriptions.receiver.receive',80)}
def schema(receiver):
 methods=[('subscriptions.receiver.deploy',8)] if receiver else [('subscriptions.deploy',122)]
 methods +=[(n,size) for n,size in METHODS.values() if ('.receiver.' in n)==receiver]
 sizes={n:i+2 for i,n in enumerate(sorted({size for _,size in methods if size is not None}))}
 state=192 if receiver else 12288
 lines=['[descriptor]','execution_mode = "message_isolated"',f'state_schema_id = "{hashlib.sha256(("subscription-v1-receiver" if receiver else "subscription-v1-hub").encode()).hexdigest()}"','','[abi]','state_root_type_id = 1','','[[abi.types]]','id = 1','kind = "fixed_bytes"','name = "state"',f'len = {state}']
 for n,i in sizes.items():lines+=['','[[abi.types]]',f'id = {i}','kind = "fixed_bytes"',f'name = "bytes_{n}"',f'len = {n}']
 for name,size in methods:
  lines+=['','[[abi.methods]]',f'name = "{name}"']
  if size is not None:lines+=[f'input_type_id = {sizes[size]}']
 if not receiver:
  u128=max(sizes.values())+1;u64=u128+1
  for n,t in [(u128,'u128'),(u64,'u64')]:lines+=['','[[abi.types]]',f'id = {n}',f'kind = "{t}"',f'name = "{t}_value"']
  lines+=['','[[abi.events]]','name = "subscriptions.changed"']
  for i,(name,t) in enumerate([('asset0_liability',u128),('asset1_liability',u128),('kind',u64),('subscription',u64)],1):lines+=['','[[abi.events.fields]]',f'id = {i}',f'type_id = {t}',f'name = "{name}"']
 names=['unauthorized','invalid_args','receiver_rejected'] if receiver else ['invalid_args','unauthorized','invalid_state','invalid_phase','unexpected_value','invalid_time','transfer_pending','invalid_callback','capacity_or_overflow','insufficient_balance','paused','stale_reference']
 for i,name in enumerate(names):lines+=['','[[abi.errors]]',f'status_code = {(4100 if receiver else 4000)+i}',f'name = "{name}"',f'message = "{name.replace("_"," ")}"']
 return '\n'.join(lines)+'\n'
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--check',action='store_true');a=p.parse_args()
 for name in ['hub','receiver']:
  path=ROOT/'contracts'/name/'avm-contract.toml';content=schema(name=='receiver')
  if a.check:assert path.read_text()==content,name
  else:path.write_text(content)
