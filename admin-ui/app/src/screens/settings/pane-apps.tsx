import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { Alert01Icon, LinkSquare02Icon, Search01Icon } from "@hugeicons/core-free-icons";
import { Select } from "@/components/ui/input";
import { Spinner } from "@/components/ui/progress";
import { cn } from "@/lib/utils";
import { t as translate, type MessageKey } from "@/lib/i18n";
import { Rich, useT } from "@/lib/i18n-react";
import type { ServiceMode } from "@/lib/api";
import { useCatalogue, type CatalogueApp, type CatalogueState } from "./catalogue";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow } from "./rows";
import type { SettingsForm } from "./use-settings-form";

/* The apps this box runs, and where to look for more.
 *
 * Two rules hold over every string below. Nothing here names a container
 * runtime, an orchestrator or any of their nouns — the reader owns a box that
 * runs apps, and how those apps are packaged is this system's problem, not
 * theirs. And nothing here implies anyone checked the things the search
 * finds: the footer says so once, plainly, under the results rather than
 * buried in a tooltip.
 */

/** The wire spells these `native` and `container`; the reader never sees
 *  either word. The labels are what the choice actually costs them. */
const MODE_LABEL: Record<ServiceMode, MessageKey> = {
  native: "panes.apps.mode.native",
  container: "panes.apps.mode.container",
};

function isServiceMode(value: string): value is ServiceMode {
  return value === "native" || value === "container";
}

export function AppsPane({ form }: { form: SettingsForm }) {
  const t = useT();
  const filesId = React.useId();
  const codeId = React.useId();
  const searchId = React.useId();

  const draft = form.draft;
  const disabled = form.locked || !form.ready;
  const catalogue = useCatalogue(!form.locked);

  return (
    <>
      <PaneSection>
        <GroupTitle>{t("panes.apps.onThisBox")}</GroupTitle>
        <Group>
          <AppModeRow
            id={filesId}
            title={t("panes.apps.files.title")}
            detail={t("panes.apps.files.detail")}
            value={draft?.nextcloudMode ?? "container"}
            disabled={disabled}
            onChange={(mode) => form.set("nextcloudMode", mode)}
          />
          <AppModeRow
            id={codeId}
            title={t("panes.apps.code.title")}
            detail={t("panes.apps.code.detail")}
            value={draft?.forgejoMode ?? "container"}
            disabled={disabled}
            onChange={(mode) => form.set("forgejoMode", mode)}
            last
          />
        </Group>
        <GroupCaption>
          <Rich
            k="panes.apps.modeCaption"
            vars={{
              kept: (
                <strong className="font-medium text-ink">{t(MODE_LABEL.container)}</strong>
              ),
              native: <strong className="font-medium text-ink">{t(MODE_LABEL.native)}</strong>,
            }}
          />
        </GroupCaption>
      </PaneSection>

      <PaneSection>
        <GroupTitle>{t("panes.apps.findMore")}</GroupTitle>
        <Group>
          <StackRow last={catalogue.state.kind === "idle"}>
            <label htmlFor={searchId} className="sr-only">
              {t("panes.apps.searchLabel")}
            </label>
            <div className="relative">
              <HugeiconsIcon
                icon={Search01Icon}
                size={16}
                strokeWidth={1.5}
                color="currentColor"
                className="pointer-events-none absolute top-1/2 left-2.5 -translate-y-1/2 text-faint"
                aria-hidden="true"
              />
              <input
                id={searchId}
                type="search"
                value={catalogue.query}
                placeholder={t("panes.apps.searchPlaceholder")}
                autoComplete="off"
                spellCheck={false}
                disabled={disabled || catalogue.state.kind === "unsupported"}
                onChange={(event) => catalogue.setQuery(event.target.value)}
                className={cn(
                  "h-9 w-full rounded-control border border-line bg-surface pr-3 pl-8",
                  "text-sm text-ink placeholder:text-faint",
                  "transition-[border-color,box-shadow] duration-150",
                  "focus-visible:border-accent focus-visible:ring-2 focus-visible:ring-accent/35",
                  "focus-visible:outline-none",
                  "disabled:cursor-not-allowed disabled:bg-sunk disabled:text-faint",
                  "[&::-webkit-search-cancel-button]:hidden",
                )}
              />
            </div>
          </StackRow>

          <CatalogueBody state={catalogue.state} />
        </Group>

        <GroupCaption>
          {footerFor(catalogue.state)}
        </GroupCaption>
      </PaneSection>
    </>
  );
}

// ── Rows ──────────────────────────────────────────────────────────────────

interface AppModeRowProps {
  id: string;
  title: string;
  detail: string;
  value: ServiceMode;
  disabled: boolean;
  onChange: (mode: ServiceMode) => void;
  last?: boolean;
}

function AppModeRow({ id, title, detail, value, disabled, onChange, last }: AppModeRowProps) {
  const t = useT();
  return (
    <Row last={last}>
      <RowText htmlFor={id} title={title} detail={detail} />
      {/* No badge beside the picker. `local` and `mesh` are the app's only
          two badge variants and they mean "on this box" versus "on the mesh";
          both choices here run on this box, so borrowing that pair would
          teach the reader something untrue about the texture. */}
      <Select
        id={id}
        value={value}
        disabled={disabled}
        onChange={(event) => {
          const next = event.target.value;
          if (isServiceMode(next)) onChange(next);
        }}
      >
        <option value="container">{t(MODE_LABEL.container)}</option>
        <option value="native">{t(MODE_LABEL.native)}</option>
      </Select>
    </Row>
  );
}

// ── The catalogue ─────────────────────────────────────────────────────────

function CatalogueBody({ state }: { state: CatalogueState }) {
  const t = useT();
  switch (state.kind) {
    case "idle":
      return null;

    case "searching":
      return (
        <StackRow last>
          <div className="flex items-center gap-2.5 text-[13px] text-muted">
            <Spinner size={16} label={t("panes.apps.searching")} />
            {t("panes.apps.looking")}
          </div>
        </StackRow>
      );

    case "unsupported":
      return (
        <StackRow last>
          <Note>{t("panes.apps.unsupported")}</Note>
        </StackRow>
      );

    case "failed":
      return (
        <StackRow last>
          <Note tone="crit">{t("panes.apps.failed", { message: state.message })}</Note>
        </StackRow>
      );

    case "results": {
      const { apps } = state.results;
      if (apps.length === 0) {
        return (
          <StackRow last>
            <Note>{t("panes.apps.empty")}</Note>
          </StackRow>
        );
      }
      return (
        <>
          {apps.map((app, index) => (
            <ResultRow key={`${app.source}:${app.id}`} app={app} last={index === apps.length - 1} />
          ))}
        </>
      );
    }

    default: {
      const exhaustive: never = state;
      return exhaustive;
    }
  }
}

function ResultRow({ app, last }: { app: CatalogueApp; last: boolean }) {
  const t = useT();
  return (
    <Row last={last} className="animate-fade-in items-start">
      <div className="min-w-0 flex-1">
        <p className="flex flex-wrap items-baseline gap-x-2 text-sm leading-snug text-ink">
          {app.name}
          {app.version !== null && <span className="numeric text-[12px] text-faint">{app.version}</span>}
        </p>
        {app.summary !== null && (
          <p className="mt-0.5 text-[12.5px] leading-snug text-muted">{app.summary}</p>
        )}
        {/* The source is on every row and never abbreviated. It is the only
            thing here that says who wrote this and who you would be trusting. */}
        <p className="mt-1 text-[12px] text-faint">{t("panes.apps.from", { source: app.source })}</p>
      </div>
      {app.homepage !== null && (
        <a
          href={app.homepage}
          target="_blank"
          rel="noreferrer noopener external"
          className={cn(
            "mt-0.5 inline-flex shrink-0 items-center gap-1.5 rounded-control px-2 py-1",
            "text-[12.5px] text-accent transition-colors duration-150 hover:bg-accent-wash",
          )}
        >
          {t("panes.apps.lookAt")}
          <HugeiconsIcon
            icon={LinkSquare02Icon}
            size={13}
            strokeWidth={1.5}
            color="currentColor"
            aria-hidden="true"
          />
        </a>
      )}
    </Row>
  );
}

function Note({ children, tone }: { children: React.ReactNode; tone?: "crit" }) {
  return (
    <p
      className={cn(
        "flex items-start gap-2 text-[12.5px] leading-snug",
        tone === "crit" ? "text-crit" : "text-muted",
      )}
    >
      {tone === "crit" && (
        <HugeiconsIcon
          icon={Alert01Icon}
          size={15}
          strokeWidth={1.5}
          color="currentColor"
          className="mt-0.5 shrink-0"
          aria-hidden="true"
        />
      )}
      {children}
    </p>
  );
}

/* The disclaimer, and it has to be a disclaimer rather than a hedge.
 *
 * Anything found here was written by a stranger and published to a public
 * index. Nobody on the LosOS side looked at it, and this box has no shell to
 * undo an install from. Naming the indexes searched is part of the same
 * honesty: the reader can go and judge the source for themselves. */
function footerFor(state: CatalogueState): string {
  const base = translate("panes.apps.footer");
  if (state.kind !== "results" || state.results.sources.length === 0) return base;
  const { sources } = state.results;
  return translate("panes.apps.footerSearched", {
    count: sources.length,
    sources: sources.join(", "),
    footer: base,
  });
}
