import { nixString, type OptionEditor, type OptionValue, type SettingsResponse } from "@/lib/api";
import type { SettingsPaneId } from "@/screens/settings/panes";

/* One Nix literal per option, and the rules for reading and writing it.
 *
 * The Advanced pane (screens/settings/pane-advanced.tsx) edits the lines of
 * modules/overrides.nix that the sixteen-key form does not own. Each of those
 * lines is `losos.<name> = <literal>;`, and the literal is what this module
 * deals in: the pane's draft holds the literal as a string, the editors turn
 * it into a switch, a number field or a list and back, and `fits` says
 * whether it is a value of the option's kind before Apply is armed.
 *
 * `fits` mirrors lososd's gate (backend/src/options.rs, `fits`): the same
 * shapes, the same bounds, so a value this file accepts is one the box
 * accepts, and a rejection is reported here, by the field, rather than as a
 * 400 after the button was pressed. The one rule stricter than a kind's own
 * shape is the ban on `${`: the file is evaluated by nixos-rebuild as root,
 * and `"${builtins.readFile "/var/secrets/…"}"` is a string by the parser's
 * lights. api.ts's nixString escapes it, and lososd refuses it escaped or
 * not, so the field refuses it first.
 */

/** Why a literal does not fit, as a message key and its variables. */
export interface Misfit {
  key:
    | "advanced.fit.bool"
    | "advanced.fit.int"
    | "advanced.fit.between"
    | "advanced.fit.atLeast"
    | "advanced.fit.atMost"
    | "advanced.fit.float"
    | "advanced.fit.string"
    | "advanced.fit.oneOf"
    | "advanced.fit.list"
    | "advanced.fit.interpolation"
    | "advanced.fit.opaque"
    | "advanced.fit.stray";
  vars?: Record<string, string | number>;
}

/** Whether `raw`, one Nix literal, is a value of the editor's kind. */
export function fits(editor: OptionEditor, raw: string): Misfit | null {
  if (raw.includes("${")) return { key: "advanced.fit.interpolation" };
  switch (editor.kind) {
    case "bool":
      return raw === "true" || raw === "false" ? null : { key: "advanced.fit.bool" };
    case "int": {
      if (!/^-?\d+$/.test(raw)) return { key: "advanced.fit.int" };
      const n = Number(raw);
      const { min, max } = editor;
      if ((min !== undefined && n < min) || (max !== undefined && n > max)) {
        if (min !== undefined && max !== undefined) {
          return { key: "advanced.fit.between", vars: { min, max } };
        }
        if (min !== undefined) return { key: "advanced.fit.atLeast", vars: { min } };
        return { key: "advanced.fit.atMost", vars: { max: max ?? 0 } };
      }
      return null;
    }
    case "float":
      // Nix's isFloat is false for `1`: the literal must carry a point.
      return /^-?\d+\.\d+$/.test(raw) ? null : { key: "advanced.fit.float" };
    case "str":
      return nixUnquote(raw) === null ? { key: "advanced.fit.string" } : null;
    case "enum": {
      const s = nixUnquote(raw);
      if (s === null) return { key: "advanced.fit.string" };
      return editor.values.includes(s)
        ? null
        : { key: "advanced.fit.oneOf", vars: { values: editor.values.join(", ") } };
    }
    case "list":
      return nixStringList(raw) === null ? { key: "advanced.fit.list" } : null;
    case "nullable":
      return raw === "null" ? null : fits(editor.inner, raw);
    case "opaque":
      return { key: "advanced.fit.opaque" };
    default: {
      const exhaustive: never = editor;
      return exhaustive;
    }
  }
}

/** The string inside one double-quoted Nix literal, or null when `raw` is
 *  not exactly one such literal. The inverse of api.ts's nixString. */
export function nixUnquote(raw: string): string | null {
  if (raw.length < 2 || !raw.startsWith('"') || !raw.endsWith('"')) return null;
  const inner = raw.slice(1, -1);
  let out = "";
  for (let i = 0; i < inner.length; i += 1) {
    const c = inner[i];
    if (c === "\\") {
      i += 1;
      const next = inner[i];
      if (next === undefined) return null;
      out += next === "n" ? "\n" : next === "t" ? "\t" : next === "r" ? "\r" : next;
    } else if (c === '"') {
      // An unescaped quote means `raw` was two strings, not one.
      return null;
    } else {
      out += c;
    }
  }
  return out;
}

/** The elements of one `[ "a" "b" ]` literal, or null for any other shape. */
export function nixStringList(raw: string): string[] | null {
  if (!raw.startsWith("[") || !raw.endsWith("]")) return null;
  const items: string[] = [];
  let rest = raw.slice(1, -1).trimStart();
  while (rest.length > 0) {
    if (!rest.startsWith('"')) return null;
    let i = 1;
    for (;;) {
      const c = rest[i];
      if (c === undefined) return null;
      if (c === "\\") i += 2;
      else if (c === '"') break;
      else i += 1;
    }
    const item = nixUnquote(rest.slice(0, i + 1));
    if (item === null) return null;
    items.push(item);
    const after = rest.slice(i + 1);
    if (after.length > 0 && !/^\s/.test(after)) return null;
    rest = after.trimStart();
  }
  return items;
}

/** One `[ "a" "b" ]` literal for a list of strings, on one line. */
export function nixList(items: readonly string[]): string {
  return items.length === 0 ? "[ ]" : `[ ${items.map(nixString).join(" ")} ]`;
}

/** The literal lososd would write for a value of the document's shape, or
 *  null when the value has no literal (a package, an attribute set). */
export function toLiteral(editor: OptionEditor, value: OptionValue): string | null {
  switch (editor.kind) {
    case "bool":
      return typeof value === "boolean" ? String(value) : null;
    case "int":
      return typeof value === "number" && Number.isInteger(value) ? String(value) : null;
    case "float":
      if (typeof value !== "number") return null;
      return Number.isInteger(value) ? value.toFixed(1) : String(value);
    case "str":
    case "enum":
      return typeof value === "string" ? nixString(value) : null;
    case "list":
      return Array.isArray(value) && value.every((v) => typeof v === "string")
        ? nixList(value as string[])
        : null;
    case "nullable":
      return value === null ? "null" : toLiteral(editor.inner, value);
    case "opaque":
      return null;
    default: {
      const exhaustive: never = editor;
      return exhaustive;
    }
  }
}

/* The sixteen options the settings form edits on its own panes, by their
 * Nix name. The Advanced pane shows each one with its value and a link to
 * the pane that owns it, rather than a second editor that could disagree
 * with the first — unless the owning pane is planned (Market), in which
 * case the Advanced row is the one place the option can be changed, and it
 * edits the form's own field so the sixteen lines carry it. */
export interface OwnedOption {
  key: keyof SettingsResponse;
  pane: SettingsPaneId;
}

export const OWNED: Readonly<Record<string, OwnedOption>> = {
  sharingMyStorage: { key: "sharingMyStorage", pane: "market" },
  "nextcloud.mode": { key: "nextcloudMode", pane: "apps" },
  "forgejo.mode": { key: "forgejoMode", pane: "apps" },
  hostName: { key: "hostName", pane: "network" },
  "nextcloud.https": { key: "https", pane: "network" },
  "gpu.enable": { key: "gpuEnable", pane: "hardware" },
  "nextcloud.apachePort": { key: "apachePort", pane: "network" },
  "proxy.enable": { key: "proxyEnable", pane: "network" },
  "cluster.enable": { key: "clusterEnable", pane: "mesh" },
  "cluster.shareCompute": { key: "shareCompute", pane: "mesh" },
  "cluster.computeWindow.start": { key: "computeWindowStart", pane: "mesh" },
  "cluster.computeWindow.end": { key: "computeWindowEnd", pane: "mesh" },
  "hardening.apparmor": { key: "hardeningApparmor", pane: "security" },
  "hardening.malloc": { key: "hardeningMalloc", pane: "security" },
  "hardening.nosmt": { key: "hardeningNosmt", pane: "security" },
  "hardening.usbguard": { key: "hardeningUsbguard", pane: "security" },
};

/* Options another pane draws an editor for, though they are lines of
 * overrides.nix like any the Advanced pane edits (form.extra, not one of the
 * sixteen fields). The Advanced pane shows each with its value and a link to
 * that pane, so there is one place to change it. */
export const ON_PANE: Readonly<Record<string, SettingsPaneId>> = {
  "nextcloud.site.defaultLanguage": "apps",
  "nextcloud.site.phoneRegion": "apps",
  "nextcloud.site.publicLinks": "apps",
  "nextcloud.site.linkPassword": "apps",
  "nextcloud.site.linkExpiryDays": "apps",
  "nextcloud.site.defaultQuotaGB": "apps",
  "nextcloud.site.trashDays": "apps",
  "nextcloud.site.versionDays": "apps",
  "forgejo.site.requireSignIn": "apps",
  "forgejo.site.landingPage": "apps",
  "forgejo.site.defaultPrivate": "apps",
  "forgejo.site.keepEmailPrivate": "apps",
  "forgejo.site.pushCreate": "apps",
};

/** The form field's value as the literal the Advanced row shows. */
export function ownedLiteral(editor: OptionEditor, value: SettingsResponse[keyof SettingsResponse]): string {
  if (typeof value === "number" && Number.isNaN(value)) return "";
  return toLiteral(editor, value) ?? String(value);
}

/** A literal as the form field's value, for an owned option edited from the
 *  Advanced pane. Null when the literal is not of the field's shape. */
export function ownedValue(
  editor: OptionEditor,
  raw: string,
): SettingsResponse[keyof SettingsResponse] | null {
  switch (editor.kind) {
    case "bool":
      return raw === "true" ? true : raw === "false" ? false : null;
    case "int":
      return /^-?\d+$/.test(raw) ? Number(raw) : null;
    case "str":
    case "enum":
      return nixUnquote(raw);
    default:
      return null;
  }
}
