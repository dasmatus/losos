import { FlowchartIcon } from "@hugeicons/core-free-icons";
import { registerOpener } from "./openers";

/* A JSON Canvas file (.canvas, the format Obsidian Canvas writes) opens in
 * LosOS Diagrams, which reads and saves it through this service's own API. */

registerOpener({
  id: "diagrams",
  label: "dg.cloud.opener",
  icon: FlowchartIcon,
  accepts: (entry) => /\.canvas$/i.test(entry.name),
  href: (entry) => `/diagrams/?file=${encodeURIComponent(entry.path)}`,
});
