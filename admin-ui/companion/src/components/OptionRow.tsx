/* One option's row in the Advanced pane (SettingsPanes.kt OptionRow): the
 * `losos.<name>` line with its badges [careful, set by the installer, set,
 * fixed], the description and default under it, and the current value in
 * words - describe_value, which the terminal's Advanced pane carried.
 */

import type { LososTokens } from "../theme/tokens";
import { PaneRow } from "./PaneRow";
import { maybeBool, maybeObject, maybeString } from "../api/wire";
import type { JsonBody } from "../api/api";

export function OptionRow({ tokens, option }: { tokens: LososTokens; option: JsonBody }) {
  const name = maybeString(option, "name") ?? "-";
  const badges = optionBadges(option);
  const detail = [
    maybeString(option, "description") ?? "",
    maybeString(option, "defaultText") !== null ? `default: ${maybeString(option, "defaultText")}` : "",
  ]
    .filter((s) => s !== "")
    .join("\n");
  return (
    <PaneRow
      tokens={tokens}
      title={`losos.${name}${badges.length === 0 ? "" : `  [${badges.join(", ")}]`}`}
      detail={detail}
      value={optionWord(option)}
    />
  );
}

/** The badges, exactly the Compose row's order. */
function optionBadges(option: JsonBody): string[] {
  const badges: string[] = [];
  if (maybeBool(option, "danger")) badges.push("careful");
  const fixed = maybeString(option, "fixed");
  if (fixed === "installer") badges.push("set by the installer");
  else if (fixed !== null) badges.push("fixed");
  if (option.set !== undefined && option.set !== null) badges.push("set");
  return badges;
}

/* A value in words, the Advanced pane's describe_value over the editor
 * kind the option carries. Numbers by it: floats spell with a trailing
 * .0 the way the Compose port did. */
export function optionWord(option: JsonBody): string {
  return describeValue(maybeObject(option, "editor"), option.current, 0);
}

function describeValue(editor: JsonBody | null, value: unknown, depth: number): string {
  if (depth > 6) return "…";
  switch (typeof value) {
    case "undefined":
    case "object": {
      if (value === null) return "not set";
      if (Array.isArray(value)) {
        if (value.length === 0) return "not set";
        return value.map((item) => describeValue(editor, item, depth + 1)).join(", ");
      }
      const record = value as JsonBody;
      if (maybeString(record, "package") !== null) {
        return maybeString(record, "package") as string;
      }
      return maybeString(record, "nix") ?? "";
    }
    case "boolean":
      return value ? "on" : "off";
    case "number": {
      if (
        editor !== null &&
        maybeString(editor, "kind") === "float" &&
        Number.isInteger(value)
      ) {
        return `${value.toFixed(1)}`;
      }
      return `${value}`;
    }
    case "string":
      return value === "" ? `""` : value;
    default:
      return String(value);
  }
}
