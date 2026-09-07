#!/usr/bin/env python3
"""Emit public first-admission bindings from checked founding reports."""
import argparse, base64, hashlib, json, os, tempfile
from pathlib import Path

def fail(msg): raise SystemExit(f"refused: {msg}")
def sha(data): return hashlib.sha256(data).hexdigest()
def obj(v, where):
 if not isinstance(v, dict): fail(f"{where} must be an object")
 return v
def write_atomic(path, data):
 path.parent.mkdir(parents=True, exist_ok=True)
 fd, temporary = tempfile.mkstemp(prefix=f".{path.name}.", dir=path.parent)
 try:
  with os.fdopen(fd, 'w', encoding='utf-8') as handle: handle.write(data)
  os.replace(temporary, path)
 except BaseException:
  try: os.unlink(temporary)
  except FileNotFoundError: pass
  raise
def main():
 p=argparse.ArgumentParser(); p.add_argument('--cohort', required=True); p.add_argument('--founding-report', required=True, action='append'); p.add_argument('--output', required=True); a=p.parse_args()
 cohort_bytes=Path(a.cohort).read_bytes(); cohort=obj(json.loads(cohort_bytes), 'cohort manifest')
 if cohort.get('schema') != 'dclutch-cohort-manifest-v1': fail('cohort manifest has another schema')
 markets = cohort.get('markets')
 if not isinstance(markets, list): fail('cohort manifest markets is absent')
 declared = {entry.get('address') for entry in markets if isinstance(entry, dict) and isinstance(entry.get('address'), str) and entry['address']}
 bindings = {}
 for report_path in a.founding_report:
  report_bytes=Path(report_path).read_bytes(); report=obj(json.loads(report_bytes), 'founding report')
  if report.get('schema') != 'dclutch-market-founding-report-v1': fail('founding report has another schema')
  market=report.get('market'); raw=obj(report.get('linked_liability_basis_record'), 'founding report linked_liability_basis_record')
  if not isinstance(market,str) or not market: fail('founding report market is absent')
  if market in bindings: fail(f'founding reports repeat market {market}')
  data=raw.get('data_base64')
  if not isinstance(data,str): fail('founding report linked basis data_base64 is absent')
  try: body=base64.b64decode(data, validate=True)
  except Exception: fail('founding report linked basis data_base64 is invalid')
  digest=sha(body)
  stated=raw.get('sha256')
  if stated != digest: fail('founding report linked basis sha256 does not match exact raw bytes')
  if market not in declared: fail('founding report market is not in this cohort manifest')
  bindings[market]={'linked_basis_record_digest':digest,'founding_report_sha256':sha(report_bytes)}
 out={'schema':'dclutch-public-market-bindings-v1','cohort':{'number':cohort.get('cohort'),'manifest_sha256':sha(cohort_bytes)},'markets':bindings}
 write_atomic(Path(a.output), json.dumps(out,sort_keys=True,separators=(',',':'))+'\n')
if __name__=='__main__': main()
