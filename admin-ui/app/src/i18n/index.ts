/* The message catalogue: one module per area of the UI, merged here.
 *
 * Keys are namespaced by their module ("home.…", "wizard.…") so two areas can
 * never claim the same key, and every entry carries en, sk and de side by side
 * — see define.ts. lib/i18n.ts is the only reader. */

import access from "./access";
import account from "./account";
import advanced from "./advanced";
import apps from "./apps";
import cloudApps from "./cloud-apps";
import home from "./home";
import install from "./install";
import lab from "./lab";
import look from "./look";
import machines from "./machines";
import manage from "./manage";
import palette from "./palette";
import panes from "./panes";
import pins from "./pins";
import profile from "./profile";
import settings from "./settings";
import shell from "./shell";
import tour from "./tour";
import users from "./users";
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
  ...cloudApps,
  ...install,
  ...look,
  ...machines,
  ...advanced,
  ...access,
  ...users,
  ...account,
  ...profile,
  ...lab,
  ...tour,
  ...palette,
  ...pins,
  ...manage,
};

export type MessageKey = keyof typeof MESSAGES;
