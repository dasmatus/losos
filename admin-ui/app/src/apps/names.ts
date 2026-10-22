/* What an app's code can name without defining it: React's hooks, the LosOS
 * components and the data hooks the app frame puts in scope
 * (src/app-frame/runtime.tsx). The same lists are backend/src/jsx.rs's
 * HOOKS, COMPONENTS and DATA, which decide what the box accepts; the code
 * editor offers these as completions. */

import type { Completion } from "@codemirror/autocomplete";

export const APP_HOOKS = [
  "useState",
  "useEffect",
  "useMemo",
  "useRef",
  "useCallback",
  "useReducer",
  "useLayoutEffect",
  "Fragment",
] as const;

export const APP_COMPONENTS = [
  "Page",
  "Card",
  "Stack",
  "Row",
  "Text",
  "Heading",
  "Button",
  "Input",
  "TextArea",
  "Select",
  "Checkbox",
  "Badge",
  "Progress",
  "Stat",
  "Table",
  "Empty",
] as const;

export const APP_DATA = ["useCollection", "useStored", "losos"] as const;

export const APP_COMPLETIONS: Completion[] = [
  ...APP_COMPONENTS.map((label) => ({ label, type: "class", detail: "LosOS" })),
  ...APP_HOOKS.map((label) => ({ label, type: "function", detail: "React" })),
  ...APP_DATA.map((label) => ({ label, type: label === "losos" ? "variable" : "function", detail: "LosOS" })),
];
