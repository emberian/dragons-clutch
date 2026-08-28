'use client';

import { useEffect, useMemo, useState, type DragEvent } from 'react';

/**
 * One local build artifact, entering the browser.
 *
 * The README's provenance table is the contract this component renders: every
 * artifact a console asks for has exactly one producer, and the input itself
 * must say so — "if a console asks you to paste something and you don't know
 * where it comes from, that's a bug in the console." So the label names the
 * artifact, the provenance line names the producer and the file, and the
 * natural gesture is dropping that file here. Pasting its base64 stays as the
 * fallback for a machine where the file cannot be picked (an air-gapped copy,
 * a remote shell), labeled as exactly that.
 *
 * Reading a file changes nothing about trust: the bytes are checked downstream
 * exactly as pasted bytes are. What the reader gets on intake is confirmation
 * they picked up the file they meant to: its size and SHA-256, computed here,
 * to compare against the producer's own summary.
 */
export default function ArtifactInput({
  label,
  provenance,
  value,
  onChange,
  required = false,
  expectedBytes,
}: Readonly<{
  /** What this artifact is, e.g. "core · complete checked release". */
  label: string;
  /** Who produces it and where the file lives, in one concrete sentence. */
  provenance: string;
  /** The artifact as base64 — the state shape the consoles already verify. */
  value: string;
  onChange: (base64: string) => void;
  required?: boolean;
  /** Exact size the consumer will demand, when the format is fixed-width. */
  expectedBytes?: number;
}>) {
  const [dragging, setDragging] = useState(false);
  const [digest, setDigest] = useState<Readonly<{ source: string; hex: string }> | null>(null);

  /** The pasted or dropped bytes, or null when the base64 does not decode. */
  const decoded = useMemo(() => {
    if (value === '') return null;
    try {
      const binary = atob(value);
      return Uint8Array.from(binary, (character) => character.charCodeAt(0));
    } catch {
      return null;
    }
  }, [value]);

  useEffect(() => {
    if (decoded === null) return;
    let cancelled = false;
    void crypto.subtle.digest('SHA-256', decoded.slice().buffer).then((raw) => {
      if (cancelled) return;
      const hex = Array.from(new Uint8Array(raw), (byte) => byte.toString(16).padStart(2, '0')).join('');
      setDigest({ source: value, hex });
    }).catch(() => undefined);
    return () => { cancelled = true; };
  }, [decoded, value]);

  const fact = value === ''
    ? 'nothing loaded yet'
    : decoded === null
      ? 'not decodable as base64 yet'
      : [
        expectedBytes !== undefined && decoded.length !== expectedBytes
          ? `${decoded.length.toLocaleString()} bytes — the consumer expects exactly ${expectedBytes.toLocaleString()}`
          : `${decoded.length.toLocaleString()} bytes`,
        digest !== null && digest.source === value ? `SHA-256 ${digest.hex}` : null,
      ].filter((part) => part !== null).join(' · ');

  async function readFile(file: File) {
    const bytes = new Uint8Array(await file.arrayBuffer());
    let binary = '';
    for (let offset = 0; offset < bytes.length; offset += 16_384) {
      binary += String.fromCharCode(...bytes.slice(offset, offset + 16_384));
    }
    onChange(btoa(binary));
  }

  function onDrop(event: DragEvent<HTMLElement>) {
    event.preventDefault();
    setDragging(false);
    const file = event.dataTransfer.files.item(0);
    if (file !== null) void readFile(file);
  }

  return <div className={`artifact-input${dragging ? ' dragging' : ''}`}
    onDragOver={(event) => { event.preventDefault(); setDragging(true); }}
    onDragLeave={() => setDragging(false)}
    onDrop={onDrop}
  >
    <div className="artifact-heading"><span>{label}</span><small>{provenance}</small></div>
    <label className="artifact-file">
      <input
        type="file"
        aria-label={`Choose ${label}`}
        onChange={(event) => { const file = event.target.files?.item(0) ?? null; if (file !== null) void readFile(file); event.target.value = ''; }}
      />
      <span>Drop the file here, or click to choose it</span>
    </label>
    <label className="artifact-paste">
      <span>Offline fallback · paste the same file as base64</span>
      <textarea
        required={required}
        spellCheck={false}
        value={value}
        onChange={(event) => onChange(event.target.value.trim())}
      />
    </label>
    <p className="artifact-fact" aria-live="polite">{fact}</p>
  </div>;
}
