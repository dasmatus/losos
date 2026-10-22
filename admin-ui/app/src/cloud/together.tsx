/* LosOS Cloud's files, handed to the other apps from the right-click menu:
 * send them by mail or in chat, or ask the box's AI about one.
 *
 * Each item is there only while the app it leads to is live for the
 * person: Mail and Chat exist on the shared side alone (GET /api/me's
 * `shared`), and the AI only when the box's model answers. A link into an
 * app the box does not run could only fail, so it is left out, not greyed.
 *
 * Words picked in a document or a note travel the same way, from the bar
 * over the selection (cloud/quick-text.tsx).
 *
 * The menu is built by plain functions (lib/context-menu.ts), so the AI's
 * dialog lives in <CloudTogether>, mounted once beside the app, and the
 * menu item asks it to open. */

import * as React from "react";
import { AiMagicIcon, BubbleChatIcon, Mail01Icon } from "@hugeicons/core-free-icons";
import { AskAiDialog, assistantReady } from "@/components/ask-ai";
import { loadMe, type AssistantSource } from "@/lib/api";
import { readCloudText, textual } from "@/lib/cloud-files";
import { addMenuItems } from "@/lib/context-menu";
import { addQuickActions } from "@/lib/quick-actions";
import { t } from "@/lib/i18n";
import { canHandTo, chatShareHref, composeMailHref } from "@/lib/intents";
import type { Entry } from "./api";

/** What the AI reads of one file, in characters. One more is sent, so the
 *  plugin, which keeps 9000, can say the rest was cut. */
const READ_CHARS = 9000;
/** Text files past this are not offered to the AI at all. */
const READ_BYTES = 4 * 1024 * 1024;

let aiReady = false;
let askAbout: ((entry: Entry) => void) | null = null;

addMenuItems("cloud.entry", (entries) => {
  const files = entries.filter((e) => !e.dir);
  // Folders do not travel by mail or chat; a mixed pick offers neither.
  const sendable = files.length > 0 && files.length === entries.length;
  const paths = files.map((e) => e.path);
  const one = entries.length === 1 ? files[0] : undefined;
  return [
    {
      id: "send-mail",
      label: t("together.send.mail"),
      icon: Mail01Icon,
      hidden: !sendable || !canHandTo("mail"),
      onSelect: () => window.location.assign(composeMailHref({ attach: paths })),
    },
    {
      id: "send-chat",
      label: t("together.send.chat"),
      icon: BubbleChatIcon,
      hidden: !sendable || !canHandTo("chat"),
      onSelect: () => window.location.assign(chatShareHref({ attach: paths })),
    },
    {
      id: "ask-ai",
      label: t("together.ai.askAbout"),
      icon: AiMagicIcon,
      hidden: one === undefined || !aiReady || askAbout === null || one.size > READ_BYTES || !textual(one.name, one.mime),
      onSelect: () => one !== undefined && askAbout?.(one),
    },
  ];
});

// Words picked in a document or a note go to Mail or Chat in a new tab, so
// the document stays open where it was.
addQuickActions("text.selection", ({ text }) => [
  {
    id: "send-mail",
    label: t("together.send.mail"),
    icon: Mail01Icon,
    hidden: !canHandTo("mail"),
    onSelect: () => window.open(composeMailHref({ body: text }), "_blank", "noopener"),
  },
  {
    id: "send-chat",
    label: t("together.send.chat"),
    icon: BubbleChatIcon,
    hidden: !canHandTo("chat"),
    onSelect: () => window.open(chatShareHref({ text }), "_blank", "noopener"),
  },
]);

/** Mounted once on Cloud's page: learns what this box runs for the person
 *  and draws the AI's dialog when a file's menu asks for it. */
export function CloudTogether() {
  const [entry, setEntry] = React.useState<Entry | null>(null);

  React.useEffect(() => {
    // `shared` decides the Mail and Chat items; nothing else here needs it.
    void loadMe().catch(() => null);
    void assistantReady().then((yes) => {
      aiReady = yes;
    });
    askAbout = setEntry;
    return () => {
      askAbout = null;
    };
  }, []);

  const sources = React.useCallback(async (): Promise<AssistantSource[]> => {
    if (entry === null) return [];
    return [{ title: entry.name, text: (await readCloudText(entry, READ_CHARS + 1)) ?? "" }];
  }, [entry]);

  if (entry === null) return null;
  return <AskAiDialog open onOpenChange={(open) => !open && setEntry(null)} sources={sources} context="cloud" />;
}
