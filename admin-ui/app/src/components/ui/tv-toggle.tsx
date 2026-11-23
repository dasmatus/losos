import * as React from "react";
import {
  getTvLayout,
  getTvLayoutActive,
  setTvLayout,
  subscribeTvLayout,
  type TvLayout,
} from "@/lib/tv";

/** The React binding for the three-state TV layout store in lib/tv.ts. */
export function useTvLayout(): {
  mode: TvLayout;
  active: boolean;
  setTvLayout: (mode: TvLayout) => void;
} {
  const mode = React.useSyncExternalStore(subscribeTvLayout, getTvLayout, () => "auto" as TvLayout);
  const active = React.useSyncExternalStore(
    subscribeTvLayout,
    getTvLayoutActive,
    () => false,
  );
  return { mode, active, setTvLayout };
}
