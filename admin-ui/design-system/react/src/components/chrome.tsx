import type { ReactNode } from 'react';

export function Topbar({ brand = 'losos', href = '/', children }:
  { brand?: string; href?: string; children?: ReactNode }) {
  return (
    <header className="topbar">
      <a className="brand" href={href}>{brand}</a>
      <nav className="topnav" aria-label="Services">{children}</nav>
    </header>
  );
}

export function Indbar() {
  return <div className="indbar" role="progressbar"><span /></div>;
}

export function Container({ children }: { children: ReactNode }) {
  return <main className="container">{children}</main>;
}

export function Grid({ children }: { children: ReactNode }) {
  return <div className="grid" role="list">{children}</div>;
}

export function Layout({ children }: { children: ReactNode }) {
  return <div className="layout">{children}</div>;
}

export function Sidebar({ children }: { children: ReactNode }) {
  return <aside className="sidebar"><nav className="side-list">{children}</nav></aside>;
}

export function SideItem({ active, danger, href = '#', onClick, children }:
  { active?: boolean; danger?: boolean; href?: string; onClick?: () => void; children: ReactNode }) {
  const cls = ['side-item', danger && 'side-item--danger', active && 'is-active']
    .filter(Boolean).join(' ');
  return <a className={cls} href={href} onClick={onClick}>{children}</a>;
}

export function Content({ children }: { children: ReactNode }) {
  return <main className="content">{children}</main>;
}

export function ContentHeader({ title, children }:
  { title: string; children?: ReactNode }) {
  return <div className="content-header"><h1>{title}</h1>{children}</div>;
}

export function Section({ active, id, children }:
  { active?: boolean; id?: string; children: ReactNode }) {
  return <section id={id} className={active ? 'section is-active' : 'section'}>{children}</section>;
}
