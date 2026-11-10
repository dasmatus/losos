/* The catalogue search card (Panes.kt SearchCard) with its hit rows
 * (SearchHitRow): a term over GET /api/apps/search?q=…, every hit with an
 * Install button only when the hit carries a chart the box can read, and
 * the one words-per-source summary. The install itself is guarded on the
 * pane, which asks the box for chart details, confirms the release name
 * and asks before starting.
 */

import * as React from "react";
import { Text, TextInput, View } from "react-native";

import { Card } from "./Card";
import { PaneRow } from "./PaneRow";
import { LosButton } from "./primitives";
import { LOSOS_TYPE, LOSOS_RADIUS, type LososTokens } from "../theme/tokens";
import { encodeQuery, itemsOf, maybeObject, maybeString } from "../../api/wire";
import { failWord, type BoxApi, type JsonBody } from "../../api/api";
import type { Read } from "../panes/Read";

export interface SearchCardProps {
  tokens: LososTokens;
  api: BoxApi;
  busy: AppJob | null;
  onInstall: (hit: JsonBody) => void;
}

export interface AppJob {
  release: string;
  action: string;
}

export function SearchCard({ tokens, api, busy, onInstall }: SearchCardProps): React.JSX.Element {
  const [query, setQuery] = React.useState("");
  const [search, setSearch] = React.useState<Read | null>(null);

  const find = () => {
    const term = query.trim();
    if (term === "") return;
    const asked = encodeQuery(term);
    setSearch({ kind: "loading" });
    api
      .get(`/api/apps/search?q=${asked}`)
      .then((data) => setSearch({ kind: "done", data }))
      .catch((e: unknown) => setSearch({ kind: "failed", reason: failWord(e) }));
  };

  return (
    <Card tokens={tokens}>
      <Text style={{ ...LOSOS_TYPE.titleMedium, color: tokens.palette.onSurface }}>Find more</Text>
      <View style={{ height: 10 }} />
      <TextInput
        value={query}
        onChangeText={setQuery}
        onSubmitEditing={find}
        returnKeyType="search"
        autoCapitalize="none"
        autoCorrect={false}
        placeholder="Search the catalogue"
        placeholderTextColor={tokens.palette.onSurfaceVariant}
        style={{
          ...LOSOS_TYPE.bodyMedium,
          color: tokens.palette.onSurface,
          borderRadius: LOSOS_RADIUS.small,
          borderWidth: 1,
          borderColor: tokens.palette.outline,
          paddingHorizontal: 12,
          paddingVertical: 10,
        }}
      />
      <View style={{ height: 8 }} />
      <LosButton tokens={tokens} onPress={find} enabled={busy === null}>
        Find
      </LosButton>
      <View style={{ height: 6 }} />
      {search === null ? null : search.kind === "loading" ? (
        <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
          Looking…
        </Text>
      ) : search.kind === "failed" ? (
        <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.extra.crit }}>
          {`Could not search: ${search.reason}`}
        </Text>
      ) : (
        (() => {
          const hits = itemsOf(search.data, "results");
          const sources = searchSources(search.data);
          return (
            <View>
              {hits.length === 0 ? (
                <Text style={{ ...LOSOS_TYPE.bodyMedium, color: tokens.palette.onSurfaceVariant }}>
                  Nothing found.
                </Text>
              ) : null}
              {hits.map((hit, i) => (
                <SearchHitRow key={i} tokens={tokens} hit={hit} busy={busy} onInstall={onInstall} />
              ))}
              <Text style={{ ...LOSOS_TYPE.bodySmall, color: tokens.palette.onSurfaceVariant, marginTop: 4 }}>
                {`The box asked its ${sources.length} source${sources.length === 1 ? "" : "s"}; install uses the box's chart details and confirms before starting.`}
              </Text>
            </View>
          );
        })()
      )}
    </Card>
  );
}

/** Named sources when the box gave them; the hits' own source words
 *  otherwise, de-duplicated (Panes.kt searchSources). */
function searchSources(body: JsonBody): string[] {
  const raw = body.sources;
  if (Array.isArray(raw)) {
    const named = raw.filter((s): s is string => typeof s === "string" && s.trim() !== "");
    if (named.length > 0) return named;
  }
  const seen = new Set<string>();
  for (const hit of itemsOf(body, "results")) {
    const source = maybeString(hit, "source");
    if (source !== null && !seen.has(source)) seen.add(source);
  }
  return [...seen];
}

/** One hit (Panes.kt SearchHitRow): the name with its version, the
 *  summary and source under it, and the Install button guarded on a
 *  chart triple the box can act on. */
function SearchHitRow({
  tokens,
  hit,
  busy,
  onInstall,
}: {
  tokens: LososTokens;
  hit: JsonBody;
  busy: AppJob | null;
  onInstall: (hit: JsonBody) => void;
}) {
  const name = maybeString(hit, "name") ?? "-";
  const version = maybeString(hit, "version");
  const chart = maybeObject(hit, "chart");
  const canInstall =
    chart !== null &&
    (maybeString(chart, "repo") ?? "").length > 0 &&
    (maybeString(chart, "name") ?? "").length > 0 &&
    (maybeString(chart, "version") ?? "").length > 0;
  const detail = [
    maybeString(hit, "summary") ?? "",
    `source: ${maybeString(hit, "source") ?? "-"}`,
  ]
    .filter((s) => s !== "")
    .join("\n");
  return (
    <View style={{ width: "100%", paddingVertical: 4 }}>
      <PaneRow
        tokens={tokens}
        title={version !== null ? `${name}  ${version}` : name}
        detail={detail}
        tone={tokens.palette.onSurface}
      />
      {canInstall ? (
        <LosButton tokens={tokens} kind="outlined" onPress={() => onInstall(hit)} enabled={busy === null}>
          Install
        </LosButton>
      ) : null}
    </View>
  );
}
