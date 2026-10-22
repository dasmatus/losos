/* The diagram's operations, from wherever the Lab's actions live. */

import { actions } from "../actions";
import type { CanvasOps } from "./ops";

export function canvasOps(): CanvasOps {
  return actions().canvas;
}
