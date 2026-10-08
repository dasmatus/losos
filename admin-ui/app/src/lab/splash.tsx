/* The splash: a Packet Tracer look-alike above, LosOS below, while the core
 * and (on the hosted copy) qemu-wasm load. With LOSOS_LAB_MEME at build time
 * the two pictures stand left of the two halves. Ported from shell.html. */

import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import { plateUrl } from "./icons";
import { MEME } from "./shape";

export type SplashStep = "canvas" | "qemu" | "placing" | "failed";

export function Splash({ step, gone, error }: { step: SplashStep; gone: boolean; error?: string }) {
  const t = useT();
  const msg =
    step === "qemu" ? t("lab.splash.qemu") : step === "placing" ? t("lab.splash.placing") : step === "failed" ? (error ?? "") : t("lab.splash.canvas");
  return (
    <div className={cn("lab-splash", gone && "gone", MEME && "meme")} role="status" aria-label={t("lab.splash.label")} data-testid="lab-splash">
      <div className="sp-top">
        {MEME && <img className="sp-meme" src="meme-no.jpg" alt="" />}
        <div className="sp-pane">
          <svg className="sp-mark" viewBox="0 0 120 120" aria-hidden="true">
            <circle cx="60" cy="60" r="54" fill="var(--surface)" fillOpacity=".14" stroke="var(--surface)" strokeOpacity=".6" strokeWidth="2" />
            <g fill="none" stroke="var(--surface)" strokeOpacity=".75" strokeWidth="2.4" strokeLinecap="round">
              <path d="M30 78 60 40 90 72 30 78" />
              <path d="M60 40v-14" />
            </g>
            <g fill="var(--surface)">
              <circle cx="30" cy="78" r="6" />
              <circle cx="90" cy="72" r="6" />
              <circle cx="60" cy="26" r="5" />
            </g>
            <g transform="translate(43 44)">
              <rect width="34" height="24" rx="3" fill="var(--surface)" />
              <path d="M2 3l15 11 15-11" fill="none" stroke="var(--accent)" strokeWidth="2.6" strokeLinejoin="round" />
            </g>
          </svg>
          <div className="sp-word">
            <span className="sp-light">Packet</span> <span className="sp-bold">Tracer</span>
          </div>
          <div className="sp-sub">{t("lab.splash.style")}</div>
        </div>
      </div>
      <div className="sp-bottom">
        {MEME && <img className="sp-meme" src="meme-yes.jpg" alt="" />}
        <div className="sp-pane">
          <img src={plateUrl} alt="" width={84} height={84} />
          <div className="sp-los">
            LosOS <span>Lab</span>
          </div>
          <div className="sp-load">
            <span className={cn(step === "failed" && "max-w-[min(520px,90vw)] text-center text-crit")}>{msg}</span>
            {step !== "failed" && (
              <i className="sp-bar">
                <b />
              </i>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
