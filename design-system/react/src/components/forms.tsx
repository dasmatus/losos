import type {
  ButtonHTMLAttributes, InputHTMLAttributes, ReactNode, SelectHTMLAttributes,
} from 'react';

type ButtonVariant = 'primary' | 'danger' | 'outline';

export function Button({ variant = 'primary', block, href, children, ...rest }:
  { variant?: ButtonVariant; block?: boolean; href?: string; children: ReactNode }
  & ButtonHTMLAttributes<HTMLButtonElement>) {
  const cls = [`btn-${variant}`, block && 'btn-block'].filter(Boolean).join(' ');
  if (href !== undefined) return <a className={cls} href={href}>{children}</a>;
  return <button type="button" className={cls} {...rest}>{children}</button>;
}

export function Group({ danger, children }:
  { danger?: boolean; children: ReactNode }) {
  return <div className={danger ? 'group group--danger' : 'group'}>{children}</div>;
}

export function Row({ children }: { children: ReactNode }) {
  return <div className="row">{children}</div>;
}

export function GroupHint({ children }: { children: ReactNode }) {
  return <p className="group-hint">{children}</p>;
}

export function Switch({ checked, onChange, label }:
  { checked: boolean; onChange?: (checked: boolean) => void; label?: string }) {
  const control = (
    <span className="switch">
      <input
        type="checkbox"
        checked={checked}
        onChange={e => onChange?.(e.currentTarget.checked)}
        readOnly={onChange === undefined}
      />
      <span className="switch-track" />
    </span>
  );
  if (label === undefined) return control;
  return (
    <label className="switch-row">
      <span className="row-label">{label}</span>
      {control}
    </label>
  );
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} />;
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} />;
}

export function AuthOverlay({ children }: { children: ReactNode }) {
  return <div className="auth-overlay">{children}</div>;
}

export function AuthCard({ title, hint, error, onSubmit, children }:
  { title: string; hint?: string; error?: string; onSubmit?: () => void; children: ReactNode }) {
  return (
    <form className="auth-card" onSubmit={e => { e.preventDefault(); onSubmit?.(); }}>
      <h2>{title}</h2>
      {hint && <p className="auth-hint">{hint}</p>}
      {children}
      {error && <p className="auth-error">{error}</p>}
    </form>
  );
}
