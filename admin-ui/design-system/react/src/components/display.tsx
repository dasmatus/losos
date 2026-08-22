import type { ReactNode } from 'react';

export function Card({ title, status, children }:
  { title: string; status?: ReactNode; children?: ReactNode }) {
  return (
    <article className="card" role="listitem">
      <div className="card-head"><h2>{title}</h2>{status}</div>
      {children}
    </article>
  );
}

export function StatusDot({ state, label }:
  { state: 'up' | 'down' | 'checking'; label?: string }) {
  return (
    <span className="status">
      <span className="dot" data-state={state} />
      {label && <span className="status-text">{label}</span>}
    </span>
  );
}

export function Chip({ children }: { children: ReactNode }) {
  return <code className="chip">{children}</code>;
}

export function ChipRow({ children }: { children: ReactNode }) {
  return <p className="chip-row">{children}</p>;
}

export function Desc({ children }: { children: ReactNode }) {
  return <p className="desc">{children}</p>;
}

export function HintCard({ children }: { children: ReactNode }) {
  return <div className="hint-card"><p>{children}</p></div>;
}

export function SysList({ children }: { children: ReactNode }) {
  return <dl className="syslist">{children}</dl>;
}

export function SysRow({ label, children }:
  { label: string; children: ReactNode }) {
  return <div className="sysrow"><dt>{label}</dt><dd>{children}</dd></div>;
}
