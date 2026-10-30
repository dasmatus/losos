/* Which page of the box this document is, and links that work from any of
 * them.
 *
 * The admin UI at / and LosOS Cloud at /cloud/ are two documents with one
 * shell: the same top bar, the same sidebar and the same command palette
 * (components/app-sidebar.tsx, src/palette/). Each runs its own router,
 * the admin page's at the root and LosOS Cloud's under the basename
 * /cloud, so a router <Link to="/storage"> drawn in LosOS Cloud would go to
 * /cloud/storage. Everything the shell draws names a page by its address
 * on the box instead, and these turn that into a router link where this
 * document holds the page and a plain link (a page load) where another
 * one does. */

import * as React from "react";
import { Link, useLocation, useNavigate } from "react-router-dom";

/** The router's basename: "" on the admin page, "/cloud" in LosOS Cloud. */
const SiteBase = React.createContext("");

export const SiteBaseProvider = SiteBase.Provider;

/* The documents of their own under the admin page's root: nginx serves
 * each from its own location (modules/containers.nix, cloud.nix, git.nix,
 * the wiki and knowledge bases), so the admin router cannot draw them. */
const ELSEWHERE = ["/cloud/", "/git/", "/wiki/", "/kb/", "/manage/"];

/** The router path for `href` in the document under `base`, or null when
 *  another document serves it. */
export function routeIn(base: string, href: string): string | null {
  if (base === "") {
    return ELSEWHERE.some((prefix) => href === prefix.slice(0, -1) || href.startsWith(prefix)) ? null : href;
  }
  if (href === base) return "/";
  if (href.startsWith(`${base}/`)) return href.slice(base.length);
  return null;
}

/** This page's address on the box: the router's path under its basename. */
export function useSitePath(): string {
  const base = React.useContext(SiteBase);
  const { pathname } = useLocation();
  return base === "" ? pathname : `${base}${pathname === "/" ? "/" : pathname}`;
}

/** A link element for `render` props (Base UI) and React.cloneElement:
 *  a router link where this document holds `href`, else an <a>. */
export function useSiteLink(): (href: string, props?: Record<string, unknown>) => React.ReactElement {
  const base = React.useContext(SiteBase);
  return React.useCallback(
    (href: string, props: Record<string, unknown> = {}) => {
      const route = routeIn(base, href);
      return route === null ? <a href={href} {...props} /> : <Link to={route} {...props} />;
    },
    [base],
  );
}

/** Go to `href` on the box: by the router when this document holds it,
 *  else by a page load. */
export function useSiteGo(): (href: string, state?: unknown) => void {
  const base = React.useContext(SiteBase);
  const navigate = useNavigate();
  return React.useCallback(
    (href: string, state?: unknown) => {
      const route = routeIn(base, href);
      if (route === null) window.location.assign(href);
      else navigate(route, state === undefined ? undefined : { state });
    },
    [base, navigate],
  );
}

/** The router's basename this document runs under. */
export function useSiteBase(): string {
  return React.useContext(SiteBase);
}
