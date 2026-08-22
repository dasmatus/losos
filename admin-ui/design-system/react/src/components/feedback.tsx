import type { ReactNode } from 'react';
import { Indbar } from './chrome';

export type StateKind = 'building' | 'done' | 'failed';

export function Banner({ kind, title, message }:
  { kind: StateKind; title: string; message?: string }) {
  return (
    <div className="banner" data-kind={kind}>
      <div className="banner-row">
        <strong>{title}</strong>
        {message && <span className="banner-msg">{message}</span>}
      </div>
      <Indbar />
    </div>
  );
}

export function ErrorBanner({ onClose, children }:
  { onClose?: () => void; children: ReactNode }) {
  return (
    <div className="error-banner">
      <span>{children}</span>
      {onClose && (
        <button type="button" className="banner-close" aria-label="Dismiss" onClick={onClose}>
          ×
        </button>
      )}
    </div>
  );
}

export function Spinner() {
  return <span className="progress-icon spinner" />;
}

export function ProgressCard({ kind, title, log }:
  { kind: StateKind; title: string; log?: string }) {
  return (
    <div className="progress-card" data-kind={kind}>
      <div className="progress-head">
        <Spinner />
        <strong>{title}</strong>
      </div>
      {log !== undefined && <code className="progress-log">{log}</code>}
      <Indbar />
    </div>
  );
}
