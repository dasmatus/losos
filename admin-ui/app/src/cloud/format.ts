import {
  Archive01Icon,
  Doc01Icon,
  File01Icon,
  Folder01Icon,
  Image01Icon,
  MusicNote01Icon,
  Pdf01Icon,
  Ppt01Icon,
  Txt01Icon,
  Video01Icon,
  Xls01Icon,
} from "@hugeicons/core-free-icons";
import type { IconSvgElement } from "@hugeicons/react";
import { intlTag, t } from "@/lib/i18n";
export { formatBytes } from "@/lib/apps";

/* Figures and pictures for a row. Translated: call during a subscribed render. */

/** "Today 14:05", "Yesterday", "12 Mar", "12 Mar 2024". */
export function formatWhen(seconds: number, now: Date = new Date()): string {
  if (!Number.isFinite(seconds) || seconds <= 0) return "";
  const d = new Date(seconds * 1000);
  const tag = intlTag();
  const day = (x: Date) => new Date(x.getFullYear(), x.getMonth(), x.getDate()).getTime();
  const diff = Math.round((day(now) - day(d)) / 86_400_000);
  const time = d.toLocaleTimeString(tag, { hour: "2-digit", minute: "2-digit" });
  if (diff === 0) return t("cloud.when.today", { time });
  if (diff === 1) return t("cloud.when.yesterday", { time });
  if (d.getFullYear() === now.getFullYear()) return d.toLocaleDateString(tag, { day: "numeric", month: "short" });
  return d.toLocaleDateString(tag, { day: "numeric", month: "short", year: "numeric" });
}

/** The full date and time, for a tooltip or a version. */
export function formatFull(seconds: number): string {
  return new Date(seconds * 1000).toLocaleString(intlTag(), { dateStyle: "medium", timeStyle: "short" });
}

/** "March 2026", for the photo groups. */
export function formatMonth(seconds: number): string {
  return new Date(seconds * 1000).toLocaleDateString(intlTag(), { month: "long", year: "numeric" });
}

export function iconFor(entry: { dir: boolean; mime: string; name: string }): IconSvgElement {
  if (entry.dir) return Folder01Icon;
  const m = entry.mime;
  if (m.startsWith("image/")) return Image01Icon;
  if (m.startsWith("video/")) return Video01Icon;
  if (m.startsWith("audio/")) return MusicNote01Icon;
  if (m === "application/pdf") return Pdf01Icon;
  if (m.includes("zip") || m.includes("tar") || m.includes("compressed") || m.includes("7z")) return Archive01Icon;
  if (m.includes("spreadsheet") || m.includes("excel") || m === "text/csv") return Xls01Icon;
  if (m.includes("presentation") || m.includes("powerpoint")) return Ppt01Icon;
  if (m.includes("wordprocessing") || m.includes("msword") || m.includes("opendocument.text")) return Doc01Icon;
  if (m.startsWith("text/")) return Txt01Icon;
  return File01Icon;
}
