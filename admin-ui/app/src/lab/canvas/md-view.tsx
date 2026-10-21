/* A card's Markdown as React elements (the tree is markdown.ts's). Links
 * open in a new tab; a [[wiki link]] is drawn as one but goes nowhere,
 * since a diagram has no vault round it. An @ tag is a chip: a person on
 * this box with their picture, a file with its icon, opening where the
 * file lives. A name nobody on the box has stays plain text. Classes are
 * in lab.css (.dg-md). */

import * as React from "react";
import { PersonAvatar } from "@/components/person";
import { usePeople } from "@/lib/mentions";
import { type Block, type Inline, parse } from "./markdown";

const People = React.createContext<ReadonlyMap<string, { name: string; displayName: string; picture: string | null }>>(new Map());

function Person({ name }: { name: string }) {
  const person = React.useContext(People).get(name);
  if (person === undefined) return <>@{name}</>;
  return (
    <span className="dg-at" data-testid="mention-chip" data-kind="person" title={person.displayName.trim() || person.name}>
      <PersonAvatar person={person} size="xs" className="dg-at-face" />@{person.name}
    </span>
  );
}

function Runs({ kids }: { kids: Inline[] }) {
  return (
    <>
      {kids.map((n, i) => {
        switch (n.k) {
          case "t":
            return <React.Fragment key={i}>{n.text}</React.Fragment>;
          case "code":
            return <code key={i}>{n.text}</code>;
          case "b":
            return (
              <strong key={i}>
                <Runs kids={n.kids} />
              </strong>
            );
          case "i":
            return (
              <em key={i}>
                <Runs kids={n.kids} />
              </em>
            );
          case "s":
            return (
              <s key={i}>
                <Runs kids={n.kids} />
              </s>
            );
          case "mark":
            return (
              <mark key={i}>
                <Runs kids={n.kids} />
              </mark>
            );
          case "a":
            return (
              <a key={i} href={n.href} target="_blank" rel="noreferrer noopener" data-md-link>
                <Runs kids={n.kids} />
              </a>
            );
          case "wiki":
            return (
              <span key={i} className="dg-wiki" title={n.target}>
                {n.label}
              </span>
            );
          case "at":
            return <Person key={i} name={n.name} />;
          case "file":
            return (
              <a
                key={i}
                href={n.href}
                target="_blank"
                rel="noreferrer noopener"
                data-md-link
                className="dg-at"
                data-testid="mention-chip"
                data-kind="file"
              >
                <svg viewBox="0 0 24 24" aria-hidden="true" className="dg-at-file">
                  <path d="M6 3h8l4 4v14H6zM14 3v4h4" />
                </svg>
                {n.label}
              </a>
            );
        }
      })}
    </>
  );
}

function Blocks({ blocks }: { blocks: Block[] }) {
  return (
    <>
      {blocks.map((b, i) => {
        switch (b.k) {
          case "h": {
            const H = `h${b.level}` as "h1";
            return (
              <H key={i}>
                <Runs kids={b.kids} />
              </H>
            );
          }
          case "p":
            return (
              <p key={i}>
                <Runs kids={b.kids} />
              </p>
            );
          case "hr":
            return <hr key={i} />;
          case "code":
            return (
              <pre key={i}>
                <code>{b.text}</code>
              </pre>
            );
          case "quote":
            return (
              <blockquote key={i} className={b.callout ? "dg-callout" : undefined} data-callout={b.callout ?? undefined}>
                {b.title && (
                  <p className="dg-callout-title">
                    <Runs kids={b.title} />
                  </p>
                )}
                <Blocks blocks={b.blocks} />
              </blockquote>
            );
          case "ul":
          case "ol": {
            const L = b.k;
            return (
              <L key={i} start={b.k === "ol" && b.start !== 1 ? b.start : undefined}>
                {b.items.map((it, j) => (
                  <li key={j} className={`dg-d${it.depth}${it.task !== null ? " dg-task" : ""}`} data-done={it.task === true || undefined}>
                    {it.task !== null && <input type="checkbox" checked={it.task} readOnly tabIndex={-1} aria-hidden="true" />}
                    <Runs kids={it.kids} />
                  </li>
                ))}
              </L>
            );
          }
        }
      })}
    </>
  );
}

export const Markdown = React.memo(function Markdown({ text }: { text: string }) {
  const blocks = React.useMemo(() => parse(text), [text]);
  const people = usePeople();
  return (
    <People.Provider value={people}>
      <div className="dg-md">
        <Blocks blocks={blocks} />
      </div>
    </People.Provider>
  );
});
