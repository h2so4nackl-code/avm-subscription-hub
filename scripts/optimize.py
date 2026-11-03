#!/usr/bin/env python3
"""Pinned Binaryen pass, run twice; preserve both original compiler artifacts."""
from pathlib import Path
import argparse,hashlib,json,subprocess,tempfile
ROOT=Path(__file__).resolve().parents[1]
FLAGS=['-Oz','--enable-bulk-memory','--enable-sign-ext','--enable-nontrapping-float-to-int','--enable-mutable-globals','--enable-multivalue','--enable-reference-types','--strip-debug']
def run(binary):
 version=subprocess.check_output([str(binary),'--version'],text=True).strip();assert version.startswith('wasm-opt version 133'),version
 evidence={'binaryen':version,'flags':FLAGS,'contracts':{}}
 for contract in ['hub','receiver']:
  directory=ROOT/'contracts'/contract/'target/avm';wasm=directory/'contract.wasm';original=wasm.read_bytes();compiler=directory/'contract.compiler.wasm';compiler.write_bytes(original)
  with tempfile.TemporaryDirectory(prefix='subhub-opt-') as temp:
   paths=[Path(temp)/name for name in ['first.wasm','second.wasm']]
   for path in paths:subprocess.run([str(binary),str(compiler),*FLAGS,'-o',str(path)],check=True)
   optimized=paths[0].read_bytes();assert optimized==paths[1].read_bytes();wasm.write_bytes(optimized)
  evidence['contracts'][contract]={'compiler_bytes':len(original),'optimized_bytes':len(optimized),'compiler_sha256':hashlib.sha256(original).hexdigest(),'optimized_sha256':hashlib.sha256(optimized).hexdigest(),'two_independent_optimizer_outputs_match':True}
 (ROOT/'evidence/optimization.json').write_text(json.dumps(evidence,indent=2)+'\n');print('Two independent Binaryen outputs match: PASS')
if __name__=='__main__':
 p=argparse.ArgumentParser();p.add_argument('--wasm-opt',type=Path,required=True);a=p.parse_args();run(a.wasm_opt.resolve())
