/* The message catalogue: one module per area of the UI, merged here.
 *
 * Keys are namespaced by their module ("home.…", "wizard.…") so two areas can
 * never claim the same key, and every entry carries en, sk and de side by side
 * — see define.ts. lib/i18n.ts is the only reader. */

import access from "./access";
import account from "./account";
import advanced from "./advanced";
import apps from "./apps";
import basic from "./basic";
import calendar from "./calendar";
import calls from "./calls";
import chat from "./chat";
import ci from "./ci";
import cloud from "./cloud";
import cloudApps from "./cloud-apps";
import credentials from "./credentials";
import logs from "./logs";
import details from "./details";
import collab from "./collab";
import contacts from "./contacts";
import games from "./games";
import git from "./git";
import tracker from "./tracker";
import gitReview from "./git-review";
import home from "./home";
import inference from "./inference";
import install from "./install";
import diagrams from "./diagrams";
import lab from "./lab";
import look from "./look";
import mail from "./mail";
import mention from "./mention";
import machines from "./machines";
import manage from "./manage";
import menu from "./menu";
import moderation from "./moderation";
import notes from "./notes";
import office from "./office";
import palette from "./palette";
import panes from "./panes";
import pictures from "./pictures";
import pins from "./pins";
import profile from "./profile";
import quick from "./quick";
import report from "./report";
import settings from "./settings";
import statistics from "./statistics";
import shell from "./shell";
import together from "./together";
import tour from "./tour";
import userApps from "./user-apps";
import users from "./users";
import widgets from "./widgets";
import wiki from "./wiki";
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
  ...userApps,
  ...cloudApps,
  ...cloud,
  ...collab,
  ...notes,
  ...git,
  ...tracker,
  ...gitReview,
  ...install,
  ...look,
  ...pictures,
  ...details,
  ...report,
  ...machines,
  ...advanced,
  ...access,
  ...credentials,
  ...logs,
  ...users,
  ...moderation,
  ...account,
  ...profile,
  ...lab,
  ...diagrams,
  ...tour,
  ...palette,
  ...pins,
  ...calls,
  ...manage,
  ...chat,
  ...office,
  ...mail,
  ...contacts,
  ...calendar,
  ...menu,
  ...ci,
  ...mention,
  ...games,
  ...together,
  ...quick,
  ...wiki,
  ...statistics,
  ...basic,
};

export type MessageKey = keyof typeof MESSAGES;
