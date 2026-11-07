#!/usr/bin/env node
/**
 * tsx2rsx - local helper in the spirit of `dx translate --html`, extended for TSX/JSX.
 *
 * Converts HTML/JSX/TSX markup fragments to Dioxus 0.6 RSX.
 * Best-effort: handles tags, attributes (className/htmlFor/events/boolean),
 * self-closing tags, fragments, and preserves {expressions}. Complex TSX
 * (types, hooks, control flow, style objects, spread props) needs manual fixup.
 *
 * Usage:
 *   node tsx2rsx.mjs < input.tsx > output.rsx
 *   node tsx2rsx.mjs file.tsx
 *   node tsx2rsx.mjs --component Name file.tsx
 *   node tsx2rsx.mjs --rsx-only file.tsx   (no component wrapper implied; default prints body)
 *
 * Similar to: dx translate --html (Dioxus CLI), but accepts TSX/JSX syntax.
 */
import fs from "node:fs";

const args = process.argv.slice(2);
let wrap = null, file = null;
for (let i = 0; i < args.length; i++) {
  if (args[i] === "--component" && args[i + 1]) wrap = args[++i];
  else if (args[i] === "--help" || args[i] === "-h") {
    process.stdout.write(fs.readFileSync(new URL(import.meta.url), "utf8").match(/\/\*\*[\s\S]*?\*\//)[0] + "\n");
    process.exit(0);
  } else if (!args[i].startsWith("--")) file = args[i];
}

let src = file ? fs.readFileSync(file, "utf8") : fs.readFileSync(0, "utf8");

const EVENT = {
  onClick:"onclick", onChange:"onchange", onInput:"oninput", onSubmit:"onsubmit",
  onKeyDown:"onkeydown", onKeyUp:"onkeyup", onKeyPress:"onkeypress",
  onMouseEnter:"onmouseenter", onMouseLeave:"onmouseleave", onMouseDown:"onmousedown", onMouseUp:"onmouseup",
  onFocus:"onfocus", onBlur:"onblur", onDoubleClick:"ondoubleclick",
};
const RENAME = { className:"class", htmlFor:"r#for", tabIndex:"tabindex", srcSet:"srcset", autoComplete:"autocomplete", readOnly:"readonly", crossOrigin:"crossorigin", dateTime:"datetime" };

function rsxAttrName(name) {
  if (EVENT[name]) return EVENT[name];
  if (RENAME[name]) return RENAME[name];
  if (name.startsWith("aria-") || name.startsWith("data-")) return `"${name}"`;
  return name;
}

function splitAttrs(raw) {
  const a = (raw || "").trim();
  if (!a) return [];
  const attrs = [];
  let i = 0;
  while (i < a.length) {
    while (i < a.length && /\s/.test(a[i])) i++;
    if (i >= a.length) break;
    if (a[i] === "{" && a[i+1] === "." && a[i+2] === "." && a[i+3] === ".") {
      // spread: {...props}
      let depth=0, s="";
      while (i < a.length) { if(a[i]==="{") depth++; else if(a[i]==="}") depth--; s+=a[i++]; if(depth===0) break; }
      attrs.push({spread:s}); continue;
    }
    const nm = a.slice(i).match(/^([@:A-Za-z_][\w:.-]*)/);
    if (!nm) { i++; continue; }
    const name = nm[1]; i += name.length;
    while (i < a.length && /\s/.test(a[i])) i++;
    if (a[i] !== "=") { attrs.push({name, value:true}); continue; }
    i++; while (i < a.length && /\s/.test(a[i])) i++;
    if (a[i] === '"' || a[i] === "'") {
      const q=a[i++]; let v=""; while(i<a.length && a[i]!==q){ if(a[i]==="\\") v+=a[i++]; v+=a[i++]||""; } i++;
      attrs.push({name, value:`${q}${v}${q}`});
    } else if (a[i] === "{") {
      let depth=0, s=""; while(i<a.length){ if(a[i]==="{") depth++; else if(a[i]==="}") depth--; s+=a[i++]; if(depth===0) break; }
      let inner = s.slice(1,-1).trim();
      // If it's a quoted string expression, keep as string; else raw expr.
      if (/^["'`]/.test(inner) && /["'`]$/.test(inner)) attrs.push({name, value:inner});
      else attrs.push({name, expr:inner});
    } else {
      attrs.push({name, value:true});
    }
  }
  return attrs;
}

function fmtAttrs(raw) {
  const parts = splitAttrs(raw).map(a => {
    if (a.spread) return `..${a.spread.slice(1,-1).replace(/^\.\.\./,"")}`;
    const n = rsxAttrName(a.name);
    if (a.value === true) return `${n}: true`;
    if (a.expr != null) return `${n}: ${a.expr}`;
    return `${n}: ${a.value}`;
  });
  return parts.length ? parts.join(", ") + ", " : "";
}

// Minimal tokenizer for tags and text/{expr} children.
function convertBody(input) {
  let s = input;
  // light TS stripping
  s = s.replace(/^\s*import[\s\S]*?from\s+['"][^'"]+['"];?\s*/gm, "");
  s = s.replace(/^\s*export\s+(?:default\s+)?/gm, "");
  s = s.replace(/\{\s*\/\*[\s\S]*?\*\/\s*\}/g, "");
  // fragments (handle before generic tag parsing)
  s = s.replace(/<>/g, "<Fragment>").replace(/<\/>/g, "</Fragment>");

  let out = "";
  let i = 0;
  while (i < s.length) {
    if (s[i] === "<") {
      if (s.startsWith("</", i)) {
        const end = s.indexOf(">", i); if (end < 0) { out += s[i++]; continue; }
        out += " } "; i = end + 1; continue;
      }
      // tag
      let j = i + 1;
      const isComp = /[A-Z]/.test(s[j]);
      while (j < s.length && /[A-Za-z0-9_.:-]/.test(s[j])) j++;
      const tag = s.slice(i+1, j);
      if (!tag) { out += s[i++]; continue; }
      // attrs until > or />
      let k = j, depthBrace=0, q=null;
      while (k < s.length) {
        const c=s[k];
        if (q) { if(c==="\\" ) k++; else if(c===q) q=null; }
        else if (c==='"'||c==="'"||c==="`") q=c;
        else if (c==="{") depthBrace++;
        else if (c==="}") depthBrace=Math.max(0,depthBrace-1);
        else if (depthBrace===0 && c===">") break;
        else if (depthBrace===0 && c==="/" && s[k+1]===">") break;
        k++;
      }
      const attrs = s.slice(j, k);
      const self = s[k] === "/";
      out += `${tag} { ${fmtAttrs(attrs)}`;
      if (self) { out += "} "; i = k + 2; } else { i = k + 1; }
      continue;
    }
    if (s[i] === "{") {
      // expression child - keep, but may be control flow; pass through
      let depth=0, expr=""; const start=i;
      while(i<s.length){ if(s[i]==="{") depth++; else if(s[i]==="}") depth--; expr+=s[i++]; if(depth===0) break; }
      const inner = expr.slice(1,-1).trim();
      if (!inner) continue;
      // if it contains JSX-like returns, leave as-is-ish
      out += `${expr} `;
      continue;
    }
    // text
    let t=""; while(i<s.length && s[i]!=="<" && s[i]!=="{") t+=s[i++];
    const tt = t.replace(/\s+/g, " ").trim();
    if (tt) out += `${JSON.stringify(tt)} `;
  }
  return out.replace(/\s+\}/g, " }").replace(/\{\s+/g, "{ ").trim();
}

let body = convertBody(src);
if (wrap) {
  body = `use dioxus::prelude::*;\n\n#[component]\npub fn ${wrap}() -> Element {\n    rsx! {\n        ${body}\n    }\n}\n`;
}
process.stdout.write(body + (body.endsWith("\n") ? "" : "\n"));
