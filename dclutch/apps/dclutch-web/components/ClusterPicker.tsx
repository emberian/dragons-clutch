'use client';

import { useEffect, useRef, useState, type KeyboardEvent } from 'react';

import {
  importDeploymentDocumentV1,
  LOCAL_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
  type ProtocolRoleV1,
} from '@dclutch/sdk/deployments';
import {
  chooseClusterV1,
  storeCustomDeploymentV1,
  storedCustomDeploymentV1,
  useDeploymentV1,
} from '@/lib/deploymentStore';

/**
 * The cluster picker — the ONE place "bring your own infrastructure" lives.
 *
 * Every product surface and every console reads the active deployment this
 * control selects. Devnet and Local are the baked manifest; Custom opens the
 * one modal where an endpoint and seven program addresses can be entered, and
 * the browser remembers them. Nothing else in the app ever asks for an
 * endpoint or a program address again.
 */
export default function ClusterPicker() {
  const deployment = useDeploymentV1();
  const [editing, setEditing] = useState(false);
  const [draftEndpoint, setDraftEndpoint] = useState('');
  const [draftPrograms, setDraftPrograms] = useState<Record<ProtocolRoleV1, string>>({ ...LOCAL_DEPLOYMENT_V1.programs });
  const [problem, setProblem] = useState<string | null>(null);
  const [importText, setImportText] = useState('');
  const [importNote, setImportNote] = useState<string | null>(null);
  const dialogRef = useRef<HTMLFormElement>(null);
  const firstFieldRef = useRef<HTMLTextAreaElement>(null);
  const returnFocusRef = useRef<HTMLElement | null>(null);

  useEffect(() => {
    if (!editing) return;
    firstFieldRef.current?.focus();
    return () => {
      const target = returnFocusRef.current;
      queueMicrotask(() => target?.focus());
    };
  }, [editing]);

  function openEditor() {
    returnFocusRef.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    const seed = storedCustomDeploymentV1();
    setDraftEndpoint(seed?.endpoint ?? deployment.endpoint);
    setDraftPrograms({ ...(seed?.programs ?? deployment.programs) });
    setProblem(null);
    setEditing(true);
  }

  function choose(value: string) {
    if (value === 'custom') {
      if (storedCustomDeploymentV1() !== null) chooseClusterV1('custom');
      else openEditor();
      return;
    }
    if (value === 'devnet' || value === 'local') chooseClusterV1(value);
  }

  function save() {
    try {
      storeCustomDeploymentV1({ endpoint: draftEndpoint, programs: draftPrograms });
      setEditing(false);
    } catch (error) {
      setProblem(error instanceof Error ? error.message : 'the deployment did not validate');
    }
  }

  function importDocument(text: string) {
    setImportText(text);
    if (text.trim() === '') { setImportNote(null); return; }
    try {
      const imported = importDeploymentDocumentV1(text);
      setDraftPrograms({ ...imported.programs });
      if (imported.endpoint !== null) setDraftEndpoint(imported.endpoint);
      setProblem(null);
      setImportNote(imported.endpoint !== null
        ? 'Seven programs and the endpoint filled from your run spec. Review below, then use it.'
        : 'Seven programs filled from your plan. It names no endpoint — set the RPC URL yourself.');
    } catch (error) {
      setImportNote(error instanceof Error ? error.message : 'the document did not import');
    }
  }

  function handleDialogKeyDown(event: KeyboardEvent<HTMLFormElement>) {
    if (event.key === 'Escape') {
      event.preventDefault();
      setEditing(false);
      return;
    }
    if (event.key !== 'Tab') return;
    const controls = Array.from(dialogRef.current?.querySelectorAll<HTMLElement>(
      'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
    ) ?? []);
    if (controls.length === 0) return;
    const first = controls[0];
    const last = controls[controls.length - 1];
    if (event.shiftKey && (document.activeElement === first || !dialogRef.current?.contains(document.activeElement))) {
      event.preventDefault();
      last.focus();
    } else if (!event.shiftKey && document.activeElement === last) {
      event.preventDefault();
      first.focus();
    }
  }

  return <span className="cluster-picker">
    <label>
      <span className="cluster-picker-label">cluster</span>
      <select value={deployment.cluster} onChange={(event) => choose(event.target.value)} aria-label="Active cluster">
        <option value="devnet">Devnet</option>
        <option value="local">Local</option>
        <option value="custom">Custom…</option>
      </select>
    </label>
    {deployment.cluster === 'custom'
      ? <button type="button" className="cluster-picker-edit" onClick={openEditor}>edit</button>
      : null}
    {editing ? (
      <div className="cluster-modal-backdrop" role="presentation" onClick={() => setEditing(false)}>
        <form
          ref={dialogRef}
          className="cluster-modal"
          role="dialog"
          aria-modal="true"
          aria-labelledby="cluster-dialog-title"
          aria-describedby="cluster-dialog-description"
          onClick={(event) => event.stopPropagation()}
          onKeyDown={handleDialogKeyDown}
          onSubmit={(event) => { event.preventDefault(); save(); }}
        >
          <h2 id="cluster-dialog-title">Your own deployment</h2>
          <p id="cluster-dialog-description">
            An endpoint and the seven role programs. Stored only in this browser; every surface
            reads them from here. The named clusters need none of this.
          </p>
          <label><span>Running the local successor bootstrap? Paste its run spec or plan and the form fills itself</span>
            <textarea
              ref={firstFieldRef}
              rows={3}
              value={importText}
              onChange={(event) => importDocument(event.target.value)}
              spellCheck={false}
              placeholder='{"schema": "dclutch-local-successor-run-spec-v2", …}'
            />
          </label>
          {importNote === null ? null : <p className="cluster-modal-note" aria-live="polite">{importNote}</p>}
          <label><span>JSON-RPC endpoint</span>
            <input value={draftEndpoint} onChange={(event) => setDraftEndpoint(event.target.value.trim())} spellCheck={false} placeholder="http://127.0.0.1:20890" />
          </label>
          <div className="cluster-modal-grid">
            {PROTOCOL_ROLES_V1.map((role) => (
              <label key={role}><span>{role} program</span>
                <input
                  value={draftPrograms[role]}
                  onChange={(event) => setDraftPrograms((current) => ({ ...current, [role]: event.target.value.trim() }))}
                  spellCheck={false}
                />
              </label>
            ))}
          </div>
          {problem === null ? null : <p className="cluster-modal-problem" role="alert">{problem}</p>}
          <div className="cluster-modal-actions">
            <button type="button" className="secondary-action" onClick={() => setEditing(false)}>Cancel</button>
            <button type="submit">Use this deployment</button>
          </div>
        </form>
      </div>
    ) : null}
  </span>;
}
