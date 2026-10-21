/* What this box can do beyond the six steps every box has, as the box
 * itself reports it, so the tour never shows a step for something the
 * owner cannot open.
 *
 * Each feature is read from the answer its own code gives:
 *
 *   office   GET /api/options: losos.office.enable is on in the running
 *            generation (Office is off on a new box).
 *   lend     GET /api/options: the box declares losos.inference.lend.enable,
 *            so Market can lend its memory to AI models.
 *   byName   GET /api/domains: the edge gave this box names, which carry
 *            its wallet.
 *   chat     GET /api/me: `shared` lists chat. Chat, mail and calls run as
 *            the shared user, so lososd lists them only while the box
 *            shares its storage and has joined a mesh.
 *   builder  GET /api/options: losos.builder.enable is on in the running
 *            generation (Build with AI, under Look).
 *   profile  GET /api/me carries `picture`: this lososd keeps profiles.
 *
 * A step whose pane this person cannot open (paneOpen) is left out too:
 * the spare key has no Your account, and a person outside the market's
 * section has no Market. Until an answer comes, and if it never does, the
 * feature counts as missing. */

import * as React from "react";
import { getDomains, getOptions, useMe, type Me } from "@/lib/api";
import { paneOpen } from "@/screens/settings/panes";

export type Feature = "office" | "lend" | "byName" | "chat" | "builder" | "profile";

export type BoxFeatures = Readonly<Record<Feature, boolean>>;

export const NO_FEATURES: BoxFeatures = {
  office: false,
  lend: false,
  byName: false,
  chat: false,
  builder: false,
  profile: false,
};

/** The fields of /api/me that only newer lososd builds send. */
interface MeExtras {
  shared?: unknown;
  picture?: unknown;
}

/** What the two option answers say, by name without the `losos.` prefix. */
export interface OptionFacts {
  /** Names the box declares. */
  declared: ReadonlySet<string>;
  /** Names that are `true` in the running generation. */
  on: ReadonlySet<string>;
}

/** The features, from what /api/me, /api/options and /api/domains said. Pure, for the tests. */
export function featuresFrom(
  me: Me | null,
  options: OptionFacts | null,
  hasNames: boolean,
  open: (pane: "market" | "network" | "look" | "account") => boolean,
): BoxFeatures {
  const extras = (me ?? {}) as MeExtras;
  const shared = Array.isArray(extras.shared) ? extras.shared : [];
  return {
    office: options?.on.has("office.enable") === true,
    lend: options?.declared.has("inference.lend.enable") === true && open("market"),
    byName: hasNames && open("network"),
    chat: shared.includes("chat"),
    builder: options?.on.has("builder.enable") === true && open("look"),
    profile: me !== null && "picture" in extras && open("account"),
  };
}

/** For the tour: this box's features, re-read when the person signed in changes. */
export function useBoxFeatures(): BoxFeatures {
  const me = useMe();
  const [options, setOptions] = React.useState<OptionFacts | null>(null);
  const [hasNames, setHasNames] = React.useState(false);

  React.useEffect(() => {
    const controller = new AbortController();
    getOptions({ signal: controller.signal })
      .then((answer) => {
        if (!answer.available) return;
        setOptions({
          declared: new Set(answer.options.map((option) => option.name)),
          on: new Set(answer.options.filter((option) => option.current === true).map((option) => option.name)),
        });
      })
      .catch(() => {
        // No answer, or not this person's to read: the option-based steps stay out.
      });
    getDomains({ signal: controller.signal })
      .then((answer) => {
        const names = answer.available ? (answer as { names?: unknown }).names : undefined;
        setHasNames(Array.isArray(names) && names.length > 0);
      })
      .catch(() => {
        // No edge in reach: no names to be paid at.
      });
    return () => controller.abort();
  }, []);

  return React.useMemo(() => featuresFrom(me, options, hasNames, paneOpen), [me, options, hasNames]);
}
