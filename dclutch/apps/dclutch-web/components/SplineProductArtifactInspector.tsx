'use client';

import { useMemo, useState, type ChangeEvent } from 'react';
import { decodeResultDomainV2, type ResultDomainV2 } from '@/lib/coreFound';
import { formatTicksV1 } from '@/lib/founding/rangeProtection';

import { Alert, AlertDescription, AlertTitle } from '@/components/ui/alert';
import { Button } from '@/components/ui/button';
import { Card, CardContent, CardDescription, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Label } from '@/components/ui/label';
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/components/ui/table';
import {
  inspectSplineProductAuthoringArtifactsV1,
  type InspectedSplineProductArtifactsV1,
  type SplineProductArtifactFilesV1,
} from '@dclutch/sdk';

type ArtifactKeyV1 = keyof SplineProductArtifactFilesV1;
type LoadedArtifactV1 = Readonly<{ name: string; bytes: Uint8Array }>;
type LoadedArtifactsV1 = Partial<Record<ArtifactKeyV1, LoadedArtifactV1>>;
type BundleKeyV1 = 'report' | ArtifactKeyV1;

const MAX_REPORT_BYTES_V1 = 1_000_000;
const MAX_RECORD_BYTES_V1 = 1_000_000;

const ARTIFACTS_V1 = Object.freeze([
  Object.freeze({ key: 'product' as const, file: 'product.bin', purpose: 'Product record' }),
  Object.freeze({ key: 'resultDomain' as const, file: 'result-domain.bin', purpose: 'Result-domain record' }),
  Object.freeze({ key: 'portfolio' as const, file: 'portfolio.bin', purpose: 'Portfolio record' }),
  Object.freeze({ key: 'productBasis' as const, file: 'product-basis.bin', purpose: 'Spline basis record' }),
  Object.freeze({ key: 'priceGate' as const, file: 'price-gate.bin', purpose: 'Verified price-gate record' }),
] as const);

const BUNDLE_FILE_KEYS_V1: Readonly<Record<string, BundleKeyV1>> = Object.freeze({
  'report.json': 'report',
  ...Object.fromEntries(ARTIFACTS_V1.map(({ key, file }) => [file, key])),
});

/** Classify one compiler output bundle without reading or partially accepting it. */
export function classifySplineProductBundleFilesV1<T extends Readonly<{ name: string }>>(
  files: readonly T[],
): Readonly<Record<BundleKeyV1, T>> {
  const classified: Partial<Record<BundleKeyV1, T>> = {};
  for (const file of files) {
    const key = BUNDLE_FILE_KEYS_V1[file.name];
    if (key === undefined) throw new Error(`unexpected file ${file.name}; choose exactly the six compiler output files`);
    if (classified[key] !== undefined) throw new Error(`duplicate file ${file.name}`);
    classified[key] = file;
  }
  const missing = Object.entries(BUNDLE_FILE_KEYS_V1)
    .filter(([, key]) => classified[key] === undefined)
    .map(([file]) => file);
  if (missing.length > 0) throw new Error(`missing ${missing.join(', ')}`);
  return Object.freeze(classified as Record<BundleKeyV1, T>);
}

function refusal(error: unknown): string {
  return error instanceof Error ? error.message : 'the browser did not provide a usable refusal reason';
}

async function fileBytes(file: File, limit: number, noun: string): Promise<Uint8Array> {
  if (file.size === 0) throw new Error(`${noun} is empty`);
  if (file.size > limit) throw new Error(`${noun} exceeds the ${limit.toLocaleString()}-byte local inspection limit`);
  return new Uint8Array(await file.arrayBuffer());
}

function handoffJson(value: InspectedSplineProductArtifactsV1): string {
  return JSON.stringify(value.foundRecords, null, 2);
}

async function reportValue(file: File): Promise<unknown> {
  const source = await fileBytes(file, MAX_REPORT_BYTES_V1, 'compiler report');
  return JSON.parse(new TextDecoder('utf-8', { fatal: true }).decode(source));
}

export default function SplineProductArtifactInspector() {
  const [report, setReport] = useState<Readonly<{ name: string; value: unknown }> | null>(null);
  const [artifacts, setArtifacts] = useState<LoadedArtifactsV1>({});
  const [result, setResult] = useState<InspectedSplineProductArtifactsV1 | null>(null);
  const [partition, setPartition] = useState<ResultDomainV2 | null>(null);
  const [status, setStatus] = useState('Load report.json and all five compiler files. Nothing is read from a chain.');
  const [copyStatus, setCopyStatus] = useState('Founding handoff not copied.');

  const missing = useMemo(() => ARTIFACTS_V1.filter(({ key }) => artifacts[key] === undefined), [artifacts]);
  const ready = report !== null && missing.length === 0;

  async function loadBundle(event: ChangeEvent<HTMLInputElement>) {
    const selected = Array.from(event.target.files ?? []);
    event.target.value = '';
    if (selected.length === 0) return;
    setResult(null);
    setCopyStatus('Founding handoff not copied.');
    try {
      const bundle = classifySplineProductBundleFilesV1(selected);
      const [value, ...recordBytes] = await Promise.all([
        reportValue(bundle.report),
        ...ARTIFACTS_V1.map(({ key, file }) => fileBytes(bundle[key], MAX_RECORD_BYTES_V1, file)),
      ]);
      const loaded = Object.fromEntries(ARTIFACTS_V1.map(({ key }, index) => [
        key,
        Object.freeze({ name: bundle[key].name, bytes: recordBytes[index]! }),
      ])) as Record<ArtifactKeyV1, LoadedArtifactV1>;
      setReport(Object.freeze({ name: bundle.report.name, value }));
      setArtifacts(Object.freeze(loaded));
      setStatus('Loaded one complete compiler bundle. The six files are not trusted until you verify the handoff.');
    } catch (error) {
      setStatus(`Bundle refused: ${refusal(error)}`);
    }
  }

  async function loadReport(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.item(0) ?? null;
    event.target.value = '';
    if (file === null) return;
    setResult(null);
    try {
      if (file.name !== 'report.json') throw new Error('expected the exact filename report.json');
      const value = await reportValue(file);
      setReport(Object.freeze({ name: file.name, value }));
      setStatus(`Loaded ${file.name}. Load every record file, then verify the handoff.`);
    } catch (error) {
      setStatus(`Report refused: ${refusal(error)}`);
    }
  }

  async function loadArtifact(key: ArtifactKeyV1, expectedFile: string, file: File) {
    setResult(null);
    try {
      if (file.name !== expectedFile) throw new Error(`expected the exact filename ${expectedFile}`);
      const loaded = Object.freeze({ name: file.name, bytes: await fileBytes(file, MAX_RECORD_BYTES_V1, file.name) });
      setArtifacts((current) => ({ ...current, [key]: loaded }));
      setStatus(`Loaded ${file.name}. Verification has not run yet.`);
    } catch (error) {
      setStatus(`Artifact refused: ${refusal(error)}`);
    }
  }

  async function inspect() {
    setResult(null);
    setCopyStatus('Founding handoff not copied.');
    if (report === null || !ready) {
      setStatus('Inspection refused: report.json and all five record files are required.');
      return;
    }
    try {
      const files: SplineProductArtifactFilesV1 = Object.freeze({
        product: artifacts.product!.bytes,
        resultDomain: artifacts.resultDomain!.bytes,
        portfolio: artifacts.portfolio!.bytes,
        productBasis: artifacts.productBasis!.bytes,
        priceGate: artifacts.priceGate!.bytes,
      });
      const inspected = await inspectSplineProductAuthoringArtifactsV1(report.value, files);
      setResult(inspected);
      // The one artifact that says what this market's OUTCOMES are. It was
      // already being decoded and discarded elsewhere; here it is read for
      // what it says rather than only for whether it is well formed.
      setPartition(decodeResultDomainV2(files.resultDomain));
      setStatus('Verified all five files against the Rust compiler report and generated Registry authorities. Nothing was signed or submitted.');
    } catch (error) {
      setPartition(null);
      setStatus(`Handoff refused: ${refusal(error)}`);
    }
  }

  async function copyHandoff() {
    if (result === null) return;
    if (navigator.clipboard === undefined) {
      setCopyStatus('Copy is unavailable in this browser. Select the JSON instead.');
      return;
    }
    try {
      await navigator.clipboard.writeText(handoffJson(result));
      setCopyStatus('Copied the exact five Found39 record coordinates. Nothing was executed.');
    } catch (error) {
      setCopyStatus(`Copy refused: ${refusal(error)}`);
    }
  }

  return <Card className="spline-artifact-inspector" aria-labelledby="spline-artifact-inspector-title">
    <CardHeader>
      <CardTitle id="spline-artifact-inspector-title">Inspect the compiler handoff</CardTitle>
      <CardDescription>Load the files from one <code>product spline</code> output directory. The SDK verifies bytes, generated schemas, SHA-256 identities, canonical Registry coordinates, and the report&rsquo;s cross-record summary. It does not reimplement the spline compiler or price-gate theorem.</CardDescription>
    </CardHeader>
    <CardContent className="spline-artifact-content">
      <div className="spline-artifact-bundle">
        <Label htmlFor="spline-bundle">Compiler output · choose all six files</Label>
        <Input id="spline-bundle" type="file" multiple accept="application/json,application/octet-stream,.json,.bin" onChange={(event) => { void loadBundle(event); }} />
        <p>Select <code>report.json</code> and the five <code>.bin</code> files together. The browser accepts only the exact compiler filenames and updates the bundle only after every file can be read.</p>
        <ul aria-label="Compiler bundle file status">
          <li><span>report.json</span><strong>{report === null ? 'not loaded' : 'loaded'}</strong></li>
          {ARTIFACTS_V1.map(({ key, file }) => <li key={key}><span>{file}</span><strong>{artifacts[key] === undefined ? 'not loaded' : `${artifacts[key]!.bytes.length.toLocaleString()} bytes`}</strong></li>)}
        </ul>
      </div>

      <details className="spline-artifact-replacements">
        <summary>Replace one file</summary>
        <p>Use these controls only when correcting one file in an already loaded bundle. Each replacement must keep its exact compiler filename.</p>
        <div className="spline-artifact-grid">
          <div className="spline-artifact-picker">
            <Label htmlFor="spline-report">Compiler report · report.json</Label>
            <Input id="spline-report" type="file" accept="application/json,.json" onChange={(event) => { void loadReport(event); }} />
            <small>{report === null ? 'Required · not loaded' : `${report.name} · loaded, not trusted until verification`}</small>
          </div>
          {ARTIFACTS_V1.map(({ key, file, purpose }) => <div className="spline-artifact-picker" key={key}>
            <Label htmlFor={`spline-${key}`}>{purpose} · {file}</Label>
            <Input id={`spline-${key}`} type="file" accept="application/octet-stream,.bin" onChange={(event) => {
              const selected = event.target.files?.item(0) ?? null;
              event.target.value = '';
              if (selected !== null) void loadArtifact(key, file, selected);
            }} />
            <small>{artifacts[key] === undefined ? 'Required · not loaded' : `${artifacts[key]!.name} · ${artifacts[key]!.bytes.length.toLocaleString()} bytes`}</small>
          </div>)}
        </div>
      </details>

      <div className="spline-artifact-action">
        <Button type="button" disabled={!ready} onClick={() => { void inspect(); }}>Verify compiler handoff</Button>
        <p>{ready ? 'Ready to verify locally.' : `Waiting for ${report === null ? 'report.json' : ''}${report === null && missing.length > 0 ? ' and ' : ''}${missing.length > 0 ? missing.map(({ file }) => file).join(', ') : ''}.`}</p>
      </div>
      <Alert variant={status.includes('refused') ? 'destructive' : 'default'}>
        <AlertTitle>{result === null ? 'Local verification' : 'Verified compiler handoff'}</AlertTitle>
        <AlertDescription><p aria-live="polite">{status}</p></AlertDescription>
      </Alert>

      {result !== null && <div className="spline-artifact-result">
        <div className="operator-route-contract">
          <article><span>Product</span><strong>{result.productOutcomeCount} outcomes</strong><p>Input SHA-256 <code>{result.inputSha256}</code></p></article>
          <article><span>Basis</span><strong>Degree {result.degree} · width {result.basisWidth}</strong><p>Interior multiplicity: {result.interiorMultiplicity ? 'yes' : 'no'} · scale {result.payoutScale}</p></article>
          <article><span>Price gate</span><strong>{result.verifiedPriceGate.atomCount} admitted atoms</strong><p>Mass {result.verifiedPriceGate.mass} · prices {result.verifiedPriceGate.prices.join(', ')}</p></article>
          <article><span>Rounding</span><strong>cumulative-floor-v3</strong><p>Semantic basis <code>{result.semanticBasisId}</code></p></article>
        </div>

        {partition !== null && <div className="spline-artifact-partition">
          <h4 className="detail-subhead">The outcome partition this market actually sells</h4>
          <p className="direct-status">{partition.regionCount} ordinary cells over {partition.cuts.length} cut{partition.cuts.length === 1 ? '' : 's'}, at {partition.denominator.toString()} ticks per whole unit — read out of the operator&rsquo;s own <code>result-domain.bin</code>, not derived here. These are where the outcome changes; the payoff knots in step 01 are where the payoff bends.</p>
          <div className="spline-artifact-table" tabIndex={0} role="region" aria-label="The result domain's cuts, in order">
            <Table>
              <TableHeader><TableRow><TableHead>Cell</TableHead><TableHead>From</TableHead><TableHead>To</TableHead></TableRow></TableHeader>
              <TableBody>{Array.from({ length: partition.regionCount }, (_, cell) => <TableRow key={cell}>
                <TableCell><strong>{cell}</strong></TableCell>
                <TableCell><code>{cell === 0 ? '−∞' : formatTicksV1(partition.cuts[cell - 1], partition.denominator)}</code></TableCell>
                <TableCell><code>{cell === partition.regionCount - 1 ? '+∞' : formatTicksV1(partition.cuts[cell], partition.denominator)}</code></TableCell>
              </TableRow>)}</TableBody>
            </Table>
          </div>
          <p className="direct-status">What share of the ex-ante outcome mass each cell holds is not shown, because this bundle does not carry it: <code>dclutch-product-compiler</code> computes a <code>PartitionQualityReportV1</code> with <code>cell_share_bps</code>, and the authoring report schema <code>dclutch/product-spline-authoring-report/v1</code> does not yet emit it. Until the producer does, this page can say where the cells are and not how much of the question each one takes.</p>
        </div>}

        <div className="spline-artifact-table" tabIndex={0} role="region" aria-label="Compiler files, bytes and Registry coordinates">
          <Table>
            <TableHeader><TableRow><TableHead>Compiler file</TableHead><TableHead>Bytes and SHA-256</TableHead><TableHead>Registry coordinates</TableHead></TableRow></TableHeader>
            <TableBody>{Object.values(result.records).map((record) => <TableRow key={record.file}>
              <TableCell><strong>{record.file}</strong><br /><small>schema {record.schemaId}</small></TableCell>
              <TableCell>{record.bytes.length.toLocaleString()} bytes<br /><code>{record.contentSha256}</code></TableCell>
              <TableCell><span>raw <code>{record.rawAccount}</code></span><br /><span>staging <code>{record.stagingAccount}</code></span></TableCell>
            </TableRow>)}</TableBody>
          </Table>
        </div>

        <div className="spline-founding-handoff">
          <div><span>Next exact consumer</span><h3>Found39 record coordinates</h3><p>These are the five Registry raw accounts accepted by <code>prepareCoreFoundV2</code>. Found still authenticates their live owner, schema, digest, raw/staging relationship, and price-gate certificate on chain. Inspection does not publish the records or found a Market.</p></div>
          <Button type="button" variant="outline" onClick={() => { void copyHandoff(); }}>Copy Found39 handoff</Button>
          <textarea readOnly value={handoffJson(result)} aria-label="Verified Found39 record coordinates" />
          <p aria-live="polite">{copyStatus}</p>
        </div>
      </div>}
    </CardContent>
  </Card>;
}
