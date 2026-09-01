'use client';

import { useState } from 'react';

import { Button } from '@/components/ui/button';

export type CommandWriterV1 = (command: string) => Promise<void>;

/** Copy the exact displayed bytes through a caller-owned clipboard boundary. */
export async function copyCommandV1(command: string, write: CommandWriterV1): Promise<string> {
  if (command.length === 0) throw new Error('the command block is empty');
  await write(command);
  return 'Copied the exact displayed commands. Nothing was executed.';
}

export default function CommandRunbook({
  label,
  command,
}: Readonly<{
  label: string;
  command: string;
}>) {
  const [status, setStatus] = useState('Not copied.');

  async function copy() {
    if (navigator.clipboard === undefined) {
      setStatus('Copy is unavailable in this browser. Select the command text instead.');
      return;
    }
    try {
      setStatus(await copyCommandV1(command, (text) => navigator.clipboard.writeText(text)));
    } catch (error) {
      setStatus(`Copy refused: ${error instanceof Error ? error.message : 'the browser did not provide a usable reason'}`);
    }
  }

  return <div className="operator-command-runbook">
    <div className="operator-command-heading">
      <strong>{label}</strong>
      <Button type="button" variant="outline" onClick={() => { void copy(); }}>Copy commands</Button>
    </div>
    <pre><code>{command}</code></pre>
    <p aria-live="polite">{status}</p>
  </div>;
}
