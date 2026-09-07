#!/usr/bin/env python3
"""Emit public first-admission bindings from one checksummed founding report."""
import argparse, base64, hashlib, json
from pathlib import Path

def fail(msg): raise SystemExit(f"refused: {msg}")
def sha(data): return hashlib.sha256(data).hexdigest()
def obj(v, where):
 if not isinstance(v, dict): fail(f"{where} must be an object")
 return v
def main():
 p=argparse.ArgumentParser(); p.add_argument('--cohort', required=True); p.add_argument('--founding-report', required=True); p.add_argument('--output', required=True); a=p.parse_args()
 cohort_bytes=Path(a.cohort).read_bytes(); cohort=obj(json.loads(cohort_bytes), 'cohort manifest')
 report_bytes=Path(a.founding_report).read_bytes(); report=obj(json.loads(report_bytes), 'founding report')
 if cohort.get('schema') != 'dclutch-cohort-manifest-v1': fail('cohort manifest has another schema')
 if report.get('schema') != 'dclutch-market-founding-report-v1': fail('founding report has another schema')
 market=report.get('market'); raw=obj(report.get('linked_liability_basis_record'), 'founding report linked_liability_basis_record')
 if not isinstance(market,str) or not market: fail('founding report market is absent')
 data=raw.get('data_base64')
 if not isinstance(data,str): fail('founding report linked basis data_base64 is absent')
 try: body=base64.b64decode(data, validate=True)
 except Exception: fail('founding report linked basis data_base64 is invalid')
 digest=sha(body)
 stated=raw.get('sha256')
 if stated != digest: fail('founding report linked basis sha256 does not match exact raw bytes')
 if not any(m.get('address') == market for m in cohort.get('markets',[]) if isinstance(m,dict)): fail('founding report market is not in this cohort manifest')
 out={'schema':'dclutch-public-market-bindings-v1','cohort':{'number':cohort.get('cohort'),'manifest_sha256':sha(cohort_bytes)},'markets':{market:{'linked_basis_record_digest':digest,'founding_report_sha256':sha(report_bytes)}}}
 Path(a.output).write_text(json.dumps(out,sort_keys=True,separators=(',',':'))+'\n')
if __name__=='__main__': main()
