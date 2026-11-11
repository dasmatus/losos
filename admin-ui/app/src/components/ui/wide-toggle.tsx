import * as React from "react";
import { getWide, setWide, subscribeWide } from "@/lib/wide";

/** The React binding for the two-state wide store in lib/wide.ts. */
export function useWide(): {
  wide: boolean;
  setWide: (wide: boolean) => void;
} {
  const wide = React.useSyncExternalStore(subscribeWide, getWide, () => false);
  return { wide, setWide };
}
