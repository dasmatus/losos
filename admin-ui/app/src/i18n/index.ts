/* The message catalogue: one module per area of the UI, merged here.
 *
 * Keys are namespaced by their module ("home.…", "wizard.…") so two areas can
 * never claim the same key, and every entry carries en, sk and de side by side
 * — see define.ts. lib/i18n.ts is the only reader. */

import access from "./access";
import account from "./account";
import advanced from "./advanced";
import apps from "./apps";
import cloud from "./cloud";
import cloudApps from "./cloud-apps";
import credentials from "./credentials";
import collab from "./collab";
import games from "./games";
import home from "./home";
import inference from "./inference";
import install from "./install";
import lab from "./lab";
import look from "./look";
import mail from "./mail";
import machines from "./machines";
import manage from "./manage";
import menu from "./menu";
import notes from "./notes";
import office from "./office";
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
  ...inference,
  ...widgets,
  ...apps,
  ...cloudApps,
  ...cloud,
  ...collab,
  ...notes,
  ...install,
  ...look,
  ...machines,
  ...advanced,
  ...access,
  ...credentials,
  ...users,
  ...account,
  ...profile,
  ...lab,
  ...tour,
  ...palette,
  ...pins,
  ...manage,
  ...office,
  ...mail,
  ...menu,
  ...games,
};

export type MessageKey = keyof typeof MESSAGES;
