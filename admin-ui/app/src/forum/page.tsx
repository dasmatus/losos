import * as React from "react";
import {
  Alert,
  AlertDescription,
  AlertTitle,
} from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import {
  Empty,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { HugeiconsIcon } from "@hugeicons/react";
import { BubbleChatIcon } from "@hugeicons/core-free-icons";
import { SCAFFOLD_FORUM_STATUS, type ForumStatus } from "./contracts";

/**
 * Minimal routed Forum shell/placeholder (Phase 0 contracts/UI shell).
 *
 * This page is intentionally non-breaking: it does not require unimplemented
 * backend routes. It documents contract/status shape and coordinates the
 * React admin UI target with the Dioxus rewrite (PR #522) via shared contracts.
 */
export default function ForumPage() {
  const status: ForumStatus = SCAFFOLD_FORUM_STATUS;

  return (
    <div className="mx-auto flex w-full max-w-4xl flex-col gap-4 p-4 md:p-6">
      <div className="flex flex-wrap items-center gap-2">
        <h1 className="text-xl font-semibold text-ink">LosOS Forum</h1>
        <Badge variant={status.scaffold ? "neutral" : "default"}>
          {status.phase}
        </Badge>
        <Badge variant={status.enabled ? "ok" : "warn"}>
          {status.enabled ? "enabled" : "scaffold / gated"}
        </Badge>
      </div>

      <Alert>
        <AlertTitle>Phase 0 scaffold and contracts</AlertTitle>
        <AlertDescription>
          Discourse-style topics + Slack-style channels, <code>@LosOS</code>{" "}
          mentions, ActivityPub and Matrix DM bridging (DMs appear in LosOS
          Chat) via the edge proxy network. Full service/UI is not implemented
          yet. Contracts:{" "}
          <code>docs/superpowers/specs/2026-11-07-losos-forum-ui-api-contracts.md</code>.
        </AlertDescription>
      </Alert>

      <div className="grid gap-4 md:grid-cols-2">
        <Card>
          <CardHeader>
            <CardTitle>Features</CardTitle>
            <p className="text-sm text-muted">Planned Phase 1–4 surfaces.</p>
          </CardHeader>
          <CardContent className="space-y-2 text-sm">
            <FeatureRow
              label="Topics / channels"
              detail="Phase 1 core hybrid model"
              state="planned"
            />
            <FeatureRow
              label="@LosOS mentions"
              detail={`assistant ${status.features.assistant.enabled ? "enabled" : "disabled"} / ${status.features.assistant.implemented ? "implemented" : "pending"}`}
              state={status.features.assistant.implemented ? "ready" : "planned"}
            />
            <FeatureRow
              label="ActivityPub"
              detail={`${status.features.activityPub.enabled ? "enabled" : "disabled"} / ${status.features.activityPub.implemented ? "implemented" : "pending"}`}
              state={status.features.activityPub.implemented ? "ready" : "planned"}
            />
            <FeatureRow
              label="Matrix DM bridge"
              detail={`${status.features.matrixDmBridge.enabled ? "enabled" : "disabled"} / chat visible: ${status.features.matrixDmBridge.chatVisible ? "yes" : "no"}`}
              state={status.features.matrixDmBridge.implemented ? "ready" : "planned"}
            />
          </CardContent>
        </Card>

        <Card>
          <CardHeader>
            <CardTitle>Gates and routing</CardTitle>
            <p className="text-sm text-muted">Edge-local/proxy dependencies.</p>
          </CardHeader>
          <CardContent className="space-y-2 text-sm">
            <FeatureRow
              label="Edge-local routing"
              detail={status.edgeLocalRouting ? "design default on" : "off"}
              state={status.edgeLocalRouting ? "ready" : "planned"}
            />
            <FeatureRow
              label="Proxy required"
              detail={status.gates.proxyRequired ? "yes" : "no"}
              state={status.gates.proxyRequired ? "ready" : "planned"}
            />
            <FeatureRow
              label="Mesh required"
              detail={
                status.gates.meshRequired == null
                  ? "open design decision"
                  : status.gates.meshRequired
                    ? "yes"
                    : "no"
              }
              state={status.gates.meshRequired == null ? "planned" : "ready"}
            />
            <FeatureRow
              label="Edge reachable"
              detail={status.edge.reachable ? "yes" : "unknown / not checked in placeholder"}
              state={status.edge.reachable ? "ready" : "planned"}
            />
          </CardContent>
        </Card>
      </div>

      <Card>
        <CardHeader>
          <CardTitle>UI surfaces (contracted)</CardTitle>
          <p className="text-sm text-muted">Phase 1–2 request/response shapes are defined for these surfaces.</p>
        </CardHeader>
        <CardContent>
          <Empty className="border-dashed">
            <EmptyHeader>
              <EmptyMedia variant="icon">
                <HugeiconsIcon icon={BubbleChatIcon} />
              </EmptyMedia>
              <EmptyTitle>Forum UI implementation pending</EmptyTitle>
              <EmptyDescription>
                Spaces, topics, channels, DMs, reactions, mentions, and{" "}
                <code>@LosOS</code> provenance rendering will be implemented
                against <code>/api/forum/*</code> contracts. React is the
                current delivery target; PR #522 Dioxus rewrite should consume
                the same contracts with minimal duplication.
              </EmptyDescription>
            </EmptyHeader>
          </Empty>
        </CardContent>
      </Card>
    </div>
  );
}

function FeatureRow({
  label,
  detail,
  state,
}: {
  label: string;
  detail: string;
  state: "ready" | "planned";
}) {
  return (
    <div className="flex items-start justify-between gap-3 rounded-md border border-line p-2">
      <div>
        <div className="font-medium text-ink">{label}</div>
        <div className="text-muted">{detail}</div>
      </div>
      <Badge variant={state === "ready" ? "ok" : "neutral"}>{state}</Badge>
    </div>
  );
}
