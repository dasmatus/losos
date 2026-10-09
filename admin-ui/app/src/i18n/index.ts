/* The message catalogue: one module per area of the UI, merged here.
 *
 * Keys are namespaced by their module ("home.…", "wizard.…") so two areas can
 * never claim the same key, and every entry carries en, sk and de side by side
 * — see define.ts. lib/i18n.ts is the only reader. */

import advanced from "./advanced";
import apps from "./apps";
import home from "./home";
import lab from "./lab";
import look from "./look";
import machines from "./machines";
import panes from "./panes";
import settings from "./settings";
import shell from "./shell";
import widgets from "./widgets";
import wizard from "./wizard";

export type { Entry, Plural, Text } from "./define";

export const MESSAGES = {
  ...shell,
  ...home,
  ...wizard,
  ...settings,
  ...panes,
  ...widgets,
  ...apps,
  ...look,
  ...machines,
  ...advanced,
  ...lab,
};

export type MessageKey = keyof typeof MESSAGES;
