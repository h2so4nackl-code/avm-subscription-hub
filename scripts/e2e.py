#!/usr/bin/env python3
"""Real AVM SDK 0.3.0 subscription lifecycle; simulated ECC only.
Stand subprocess helper adapted from GOSH SDK's MIT bounty-escrow fixture.
"""
import argparse,json,subprocess,tempfile
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
DAPP="11"*32
BASE_TIME=1800000000
class Stand:
    def __init__(self, sdk, work):
        self.binary = str(sdk / "bin/avm-local-stand")
        self.profile = str(sdk / "profiles/wasmtime-release-profile.v1")
        self.state = work / "journey.state"
        self.log = []
        self.time = BASE_TIME
        self.selections = {}

    def command(self, operation, **options):
        cmd = [self.binary, operation, "--state", str(self.state), "--wasmtime-profile", self.profile]
        for key, value in options.items():
            cmd += ["--" + key.replace("_", "-")] + ([] if value is True else [str(value)])
        proc = subprocess.run(cmd, capture_output=True, text=True)
        try:
            result = json.loads(proc.stdout)
        except json.JSONDecodeError:
            raise RuntimeError(f"{operation}: {proc.stderr}\n{proc.stdout}") from None
        self.log.append({"operation": operation, "options": options, "exit_code": proc.returncode, "result": result})
        return result

    def timestamps(self):
        self.time += 1
        return dict(block_time=self.time, created_at=self.time, expire_at=self.time+3600, fuel_fee_shells=10000)

    def deploy(self, contract, salt, init=None):
        artifacts = ROOT / "contracts" / contract / "target/avm"
        options = dict(wasm=artifacts / "contract.wasm", acab=artifacts / "contract.acab",
                       descriptor=artifacts / "contract.descriptor", salt_hex=salt.encode().hex(),
                       init_json=json.dumps("0x"+(b"" if init is None else init).hex()), **self.timestamps())
        if contract in self.selections:
            for key in ["wasm", "acab", "descriptor"]:
                del options[key]
            options.update(self.selections[contract])
        # Convert paths to text for the evidence journal.
        options = {key: str(value) if isinstance(value, Path) else value for key,value in options.items()}
        precondition = self.command("deploy", quote_address=True, **options)
        assert precondition.get("quoted") is True, precondition
        address = precondition["address"]
        funding = self.command("fund-user", address=address, shells=10_000_000)
        assert funding["status"] == "ok", funding
        result = self.command("deploy", **options)
        assert result["status"] == "ok", result
        if contract not in self.selections:
            snapshot = self.command("inspect")
            existing = {v["selected_code_id"] for v in self.selections.values()}
            selection = next(item for item in snapshot["registry_entries"]
                             if item["key"].startswith("02") and item["value"][:64] not in existing)
            self.selections[contract] = {"selected_code_id": selection["value"][:64],
                                         "package_id": selection["value"][64:128]}
        return address

    def call(self, address, method, body=b"", identity="developer", value=0, extra="", expected_status=0):
        result = self.command("call", address=address, method=method,
                            args_json=json.dumps("0x"+body.hex()) if body else "null",
                            identity=identity, extra_currencies=(f"0:{value}," if value and extra else f"0:{value}" if value else "")+extra, **self.timestamps())
        assert result["status"] == "ok", result
        raw = bytes.fromhex(result["raw_executor_output"])
        actual_status = int.from_bytes(raw[4:8], "little")
        assert raw[:4] == b"ACMO" and actual_status == expected_status, {"method": method, "contract_status": actual_status, "expected": expected_status}
        return result

def num(v,n):return v.to_bytes(n,'little')
def addr(v):return bytes.fromhex(v.replace(':',''))
def config(admin,treasury):return bytes.fromhex(admin)+addr(treasury)+num(0,4)+num(7,4)+num(250,2)+num(10000,16)
def plan(asset,price,period):return bytes([asset])+num(price,16)+num(period,4)
def edit(index,version,price,period,active=True):return bytes([index])+num(version,4)+num(price,16)+num(period,4)+bytes([active])
def join(index,version,terms):return bytes([index])+num(version,4)+bytes([terms])
def withdraw(asset,amount):return bytes([asset])+num(amount,16)
def more(i,n):return num(i,8)+bytes([n])
def offer(i,award,until,digest):return num(i,8)+num(award,16)+num(until,4)+bytes([digest])*32
def accept(i,nonce,digest):return num(i,8)+num(nonce,4)+bytes([digest])*32
class R:
 def __init__(self,b):self.b=b;self.p=0
 def take(self,n):a=self.b[self.p:self.p+n];assert len(a)==n;self.p+=n;return a
 def u(self,n):return int.from_bytes(self.take(n),'little')
def decode(receipt):
 r=R(bytes.fromhex(receipt['raw_state']));assert r.take(4)==b'ASH1';r.take(122)
 d={name:[r.u(16),r.u(16)] for name in ['deposited','withdrawn','fees']}
 d.update(next_id=r.u(8),attempts=r.u(8),paused=bool(r.u(1)));r.take(32);counts=list(r.take(5));d['pending']=bool(r.u(1));r.take(26)
 for name,n in [('wallets',counts[0]),('merchants',counts[1])]:
  d[name]=[]
  for _ in range(n):d[name].append({'owner':r.take(32).hex(),'destination':r.take(64).hex(),'credit':[r.u(16),r.u(16)]})
 d['plans']=[]
 for _ in range(counts[2]):d['plans'].append(dict(merchant=r.u(1),asset=r.u(1),version=r.u(4),price=r.u(16),period=r.u(4),active=bool(r.u(1))))
 d['subs']=[]
 for _ in range(counts[3]):
  s=dict(id=r.u(8),wallet=r.u(1),plan=r.u(1),merchant=r.u(1),asset=r.u(1),price=r.u(16),period=r.u(4),version=r.u(4),start=r.u(4),terms=r.u(4),billed=r.u(16),last_time=r.u(4),offer_nonce=r.u(4),offer=bool(r.u(1)),offer_amount=r.u(16),offer_until=r.u(4),offer_digest=r.take(32).hex());d['subs'].append(s)
 d['receipt_count']=counts[4];r.take(counts[4]*32);assert not any(r.take(len(r.b)-r.p))
 liabilities=d['fees'].copy()
 for w in d['wallets']+d['merchants']:
  for a in range(2):liabilities[a]+=w['credit'][a]
 for s in d['subs']:liabilities[s['asset']]+=s['price']*s['terms']-s['billed']
 assert liabilities==[d['deposited'][a]-d['withdrawn'][a] for a in range(2)],d
 d['liabilities']=liabilities;return d

def setup(sdk,work):
 stand=Stand(sdk,work);start=stand.command('init',dapp_id=DAPP);assert start['status']=='ok';keys=start['identities'];assets=num(0,4)+num(7,4)
 receivers=[stand.deploy('receiver',name,assets) for name in ['customer','merchant','platform']]
 hub=stand.deploy('hub','subscription-hub',config(keys['developer'],receivers[2]))
 for a in [hub,*receivers]:assert stand.command('fund-user',address=a,shells=10**12)['status']=='ok'
 for a in receivers:stand.call(a,'subscriptions.receiver.bind',addr(hub))
 stand.call(hub,'subscriptions.wallet.open',addr(receivers[0]),identity='user')
 stand.call(hub,'subscriptions.merchant.add',bytes.fromhex(keys['package_owner'])+addr(receivers[1]))
 return stand,keys,hub,receivers

def run(sdk,output):
 output.parent.mkdir(parents=True,exist_ok=True)
 with tempfile.TemporaryDirectory(prefix='avm-subscriptions-') as temp:
  stand,keys,hub,receivers=setup(sdk,Path(temp))
  try:
   def c(method,body=b'',identity='user',expected=0,**kw):return stand.call(hub,'subscriptions.'+method,body,identity=identity,expected_status=expected,**kw)
   c('deposit',b'\0',value=10000);c('deposit',b'\1',extra='7:20000')
   c('plan.new',plan(0,1000,100),identity='package-owner');c('plan.new',plan(1,2000,100),identity='package-owner')
   first=c('join',join(0,1,3));t0=decode(first)['subs'][0]['start']
   second=c('join',join(1,1,2));t1=decode(second)['subs'][1]['start']
   c('plan.edit',edit(0,1,9000,200),identity='package-owner')
   c('join',join(0,1,1),expected=4011)
   c('withdraw.merchant',withdraw(0,1),identity='package-owner',expected=4009)
   stand.time=t0+99;d=decode(c('collect',more(1,1),identity='developer'));assert d['merchants'][0]['credit']==[975,0] and d['fees']==[25,0]
   # Plan repricing did not alter the running subscription's snapshot.
   assert (d['subs'][0]['price'],d['subs'][0]['period'],d['subs'][0]['version'])==(1000,100,1)
   stand.time=t1+149;d=decode(c('cancel',num(2,8)));assert d['merchants'][0]['credit'][1]==2925 and d['fees'][1]==75
   assert d['wallets'][0]['credit'][1]==17000
   # Negotiate a fixed additional award after accruing to the offer timestamp.
   now=stand.time+1;c('offer',offer(1,200,now+100,0x61));d=decode(c('status'));gross_before=d['subs'][0]['billed']
   c('offer.accept',accept(1,2,0x61),identity='package-owner',expected=4011)
   d=decode(c('offer.accept',accept(1,1,0x61),identity='package-owner'));assert not d['subs'];assert d['wallets'][0]['credit'][0]==10000-gross_before-200
   assert d['merchants'][0]['credit'][0]+d['fees'][0]==gross_before+200
   # A rejected recipient really fails, returns the exact asset and settles failure.
   stand.call(receivers[1],'subscriptions.receiver.reject',b'\1')
   before=d['merchants'][0]['credit'][0]
   d=decode(c('withdraw.merchant',withdraw(0,before),identity='package-owner'));assert d['withdrawn']==[0,0] and d['attempts']==1 and not d['pending']
   stand.call(receivers[1],'subscriptions.receiver.reject',b'\0')
   d=decode(c('withdraw.merchant',withdraw(0,before),identity='package-owner'));assert d['withdrawn'][0]==before and d['merchants'][0]['credit'][0]==0
   c('withdraw.merchant',withdraw(1,2925),identity='package-owner')
   c('withdraw.fees',withdraw(0,d['fees'][0]),identity='developer');c('withdraw.fees',withdraw(1,75),identity='developer')
   # Emergency pause inhibits new sales, but not customer refunds.
   c('pause',b'\1',identity='developer');c('join',join(1,1,1),expected=4010)
   d=decode(c('status'))
   c('withdraw.customer',withdraw(0,d['wallets'][0]['credit'][0]));d=decode(c('withdraw.customer',withdraw(1,17000)))
   assert d['liabilities']==[0,0] and d['withdrawn']==[10000,20000] and d['attempts']==7 and d['receipt_count']==7
   # Two-step operator handover; neither operator can access another owner's credit.
   c('admin.propose',bytes.fromhex(keys['package_owner']),identity='developer');c('admin.accept',identity='package-owner')
   c('pause',b'\0',identity='developer',expected=4001);c('pause',b'\0',identity='package-owner')
   inspection=stand.command('inspect');replay=stand.command('replay');assert replay['status']=='ok'
   for key in ['account_state_root','registry_root','cumulative_fee_shells','accepted_input_count']:assert replay[key]==inspection[key],key
   assert inspection['queue']['message_count']==0
   received=[]
   for receiver in receivers:
    b=bytes.fromhex(stand.command('inspect',account=receiver)['raw_state']);received.append([int.from_bytes(b[104:120],'little'),int.from_bytes(b[120:136],'little')])
   assert [sum(x[a] for x in received) for a in range(2)]==[10000,20000]
   output.write_text(json.dumps({'status':'PASS','accounting':d,'receivers':received,'replay':replay,'journal':stand.log},indent=2)+'\n');print('Subscriptions real AVM lifecycle: PASS (two assets, full exit and replay)')
  except Exception:
   output.write_text(json.dumps({'status':'FAIL','journal':stand.log},indent=2)+'\n');raise
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--sdk',type=Path,required=True);p.add_argument('--output',type=Path,default=ROOT/'evidence/avm-lifecycle.json');a=p.parse_args();run(a.sdk.resolve(),a.output.resolve())
