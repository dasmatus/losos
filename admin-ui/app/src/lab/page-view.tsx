/* A page Danube shows: HTML from the core (http() or error_page()).
 *
 * The core writes a few inline style="" attributes (a card's flex row, the
 * cloud login's gradient), and the Lab's policy is style-src 'self' with no
 * 'unsafe-inline': parsed from markup, each one would be refused and logged
 * as a violation. So the attribute is renamed before the markup is parsed,
 * and its declarations are put back through the CSSOM (element.style), which
 * the policy allows. The markup is the core's own, built from escaped names. */

import * as React from "react";

const STYLE_ATTR = /(<[a-zA-Z][^>]*?\s)style=("[^"]*"|'[^']*')/g;

export function setPageHtml(host: HTMLElement, html: string): void {
  let prev: string;
  let next = html;
  do {
    prev = next;
    next = prev.replace(STYLE_ATTR, "$1data-lab-style=$2");
  } while (next !== prev);
  // Parsed inert first (a <template> loads nothing and runs nothing), cleaned,
  // then moved into the page; the styles go on once the nodes are live.
  const tpl = document.createElement("template");
  tpl.innerHTML = next;
  for (const el of tpl.content.querySelectorAll("script, iframe, object, embed, style, link")) el.remove();
  for (const el of tpl.content.querySelectorAll("*")) {
    for (const attr of [...el.attributes]) if (attr.name.startsWith("on")) el.removeAttribute(attr.name);
  }
  host.replaceChildren(tpl.content);
  for (const el of host.querySelectorAll<HTMLElement>("[data-lab-style]")) {
    el.style.cssText = el.getAttribute("data-lab-style") ?? "";
    el.removeAttribute("data-lab-style");
  }
}

export function PageView({ html, className }: { html: string; className?: string }) {
  const ref = React.useRef<HTMLDivElement>(null);
  React.useLayoutEffect(() => {
    if (ref.current) setPageHtml(ref.current, html);
  }, [html]);
  return <div ref={ref} className={className} data-testid="danube-page" />;
}
