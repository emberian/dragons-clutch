'use client';

import { useState } from 'react';

import {
  LOCAL_DEPLOYMENT_V1,
  PROTOCOL_ROLES_V1,
  type ProtocolRoleV1,
} from '@/lib/deployments';
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

  function openEditor() {
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
          className="cluster-modal"
          role="dialog"
          aria-label="Custom deployment"
          onClick={(event) => event.stopPropagation()}
          onSubmit={(event) => { event.preventDefault(); save(); }}
        >
          <h2>Your own deployment</h2>
          <p>
            An endpoint and the seven role programs. Stored only in this browser; every surface
            reads them from here. The named clusters need none of this.
          </p>
          <label><span>JSON-RPC endpoint</span>
            <input value={draftEndpoint} onChange={(event) => setDraftEndpoint(event.target.value.trim())} spellCheck={false} placeholder="http://127.0.0.1:8899" />
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
          {problem === null ? null : <p className="cluster-modal-problem">{problem}</p>}
          <div className="cluster-modal-actions">
            <button type="button" className="secondary-action" onClick={() => setEditing(false)}>Cancel</button>
            <button type="submit">Use this deployment</button>
          </div>
        </form>
      </div>
    ) : null}
  </span>;
}
