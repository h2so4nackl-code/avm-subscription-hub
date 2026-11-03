#!/usr/bin/env python3
"""Bind immutable tool identities, source hashes, vectors, artifacts and local receipts."""
from pathlib import Path
from datetime import datetime,timezone
import argparse,hashlib,json,re,subprocess,platform
ROOT=Path(__file__).resolve().parents[1]
def sha(p):return hashlib.sha256(p.read_bytes()).hexdigest()
def run(sdk):
 source={str(p.relative_to(ROOT)):sha(p) for p in sorted(ROOT.rglob('*')) if p.is_file() and not any(x in {'.git','.tooling','target','__pycache__','evidence','artifacts'} for x in p.relative_to(ROOT).parts)}
 native=(ROOT/'evidence/native-tests.txt').read_text();count=sum(map(int,re.findall(r'test result: ok\. (\d+) passed;',native)));assert count==107,count
 journeys={}
 for name in ['lifecycle','churn','capacity']:
  p=ROOT/'evidence'/f'avm-{name}.json';x=json.loads(p.read_text());assert x['status']=='PASS'
  journeys[name]={'status':'PASS','sha256':sha(p),'commands':len(x['journal']),'accounting':{k:x['accounting'][k] for k in ['deposited','withdrawn','liabilities','attempts','receipt_count','next_id']},'replay':x['replay']}
 artifacts={str(p.relative_to(ROOT)):{'sha256':sha(p),'bytes':p.stat().st_size} for p in sorted((ROOT/'artifacts').rglob('*')) if p.is_file()}
 q={'recorded_at_utc':datetime.now(timezone.utc).isoformat(),'platform':platform.platform(),'rustc':subprocess.check_output(['rustc','+1.93.0','--version'],text=True).strip(),
    'sdk_release':'avm-dev-2662a32e7c91-sdk-93e34e16e449','sdk_archive_sha256':'8735c632dfc6792fe3672eaa9e157fcf9a025a317ffaa1c4e59495531eb1383f',
    'tool_sha256':{name:sha(sdk/'bin'/name) for name in ['avm-contract-build','avm-local-stand']},'profile_sha256':sha(sdk/'profiles/wasmtime-release-profile.v1'),
    'source_sha256':source,'artifacts':artifacts,'native_tests':{'project':98,'inherited_sdk':9,'total':107,'log_sha256':sha(ROOT/'evidence/native-tests.txt')},
    'independent_python_bigint_vectors':4096,'vectors_sha256':sha(ROOT/'contracts/hub/tests/fixtures/proration.csv'),
    'optimization':json.loads((ROOT/'evidence/optimization.json').read_text()),'journeys':journeys,'scope':'Local Linux x86_64 SDK 0.3.0 and simulated ECC only. Not audited, not production-qualified, no mainnet or upstream approval claim.'}
 (ROOT/'evidence/qualification.json').write_text(json.dumps(q,indent=2)+'\n')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--sdk',type=Path,required=True);a=p.parse_args();run(a.sdk.resolve())
