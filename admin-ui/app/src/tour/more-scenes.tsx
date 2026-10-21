/* The tour's scenes for what a box can add to the six steps every box has:
 * writing together in Office, lending memory to AI models, being paid by
 * name, chat, building a widget and the profile. Each one is a step only on
 * a box that has the feature (features.ts). Same rules as scenes.tsx: each
 * is a pure function of its frame on the 120 BPM grid (a beat is 30
 * frames), drawn in the 460 by 304 content box, every word from the
 * catalogue so it reads in the language on screen, and the same box
 * throughout: mattbox, where Matus and Eva both have an account. */

import {
  Alert02Icon,
  AtIcon,
  BubbleChatIcon,
  CheckmarkCircle02Icon,
  Doc01Icon,
  LockIcon,
  SparklesIcon,
  Video01Icon,
} from "@hugeicons/core-free-icons";
import { formatNumber } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { paneById } from "@/screens/settings/panes";
import { BEAT, E, clamp, fd, hitPulse, hold, mix, prog } from "./motion";
import { Glyph, Hatch, useSvgId } from "./mini-page";
import { Caption, Face, Heading, Panel, Pill, W, type SceneProps, typed, wrap } from "./parts";

/* ── Together: one document in Office, two people typing at once ───────── */

export function TogetherScene({ frame: f }: SceneProps) {
  const t = useT();

  const mine = t("tour.scene.typedMine");
  const hers = t("tour.scene.typedHers");
  // Both type from beat 1; at the end of the loop both lines are taken back.
  const back = (text: string, at: number) => clamp(text.length - fd((f - at) / 2), 0, text.length);
  const mineNow = f < 10 * BEAT ? typed(f, BEAT, mine, 5) : mine.slice(0, back(mine, 10 * BEAT));
  const hersNow = f < 10 * BEAT ? typed(f, BEAT + 20, hers, 4) : hers.slice(0, back(hers, 10 * BEAT + 10));
  const blink = fd(f / 16) % 2 === 0;
  const flag = hold(f, BEAT + 10, 10 * BEAT + 30, 14);

  const page = { x: 60, y: 58, w: 340, h: 168 };
  const left = page.x + 28;
  const bars: { y: number; w: number }[] = [
    { y: 92, w: 250 },
    { y: 106, w: 210 },
    { y: 156, w: 236 },
    { y: 170, w: 190 },
  ];
  const mineY = 134;
  const hersY = 198;

  return (
    <>
      <g className="text-accent">
        <Glyph icon={Doc01Icon} x={0} y={2} size={14} />
      </g>
      <text x="20" y="14" fontSize="14" fontWeight="600" className="fill-ink">
        {t("tour.scene.docName")}
      </text>
      <Face cx={W - 34} cy={9} r={8} initial="M" />
      <Face cx={W - 14} cy={9} r={8} initial="E" tone="warn" />

      <Panel y={28} h={210} sunk />
      <rect x={page.x + 0.5} y={page.y + 0.5} width={page.w - 1} height={page.h - 1} rx="3" className="fill-surface stroke-line" />
      <text x={left} y="80" fontSize="11" fontWeight="600" className="fill-ink">
        {t("tour.scene.docHeading")}
      </text>
      {bars.map((bar) => (
        <rect key={bar.y} x={left} y={bar.y} width={bar.w} height="5" rx="2.5" className="fill-sunk" />
      ))}

      {/* Matus and Eva, each with a caret in their colour; her name rides
          at the start of her line. The carets are text, so they sit where
          the letters end in any language. */}
      <text x={left} y={mineY} fontSize="9.5" className="fill-ink">
        {mineNow}
        <tspan className="fill-accent" opacity={blink ? 1 : 0}>
          |
        </tspan>
      </text>
      <g opacity={flag}>
        <rect x={left - 2} y={hersY - 23} width="24" height="12" rx="3" className="fill-warn" />
        <text x={left + 2} y={hersY - 14.2} fontSize="8" className="fill-surface">
          Eva
        </text>
      </g>
      <text x={left} y={hersY} fontSize="9.5" className="fill-ink">
        {hersNow}
        <tspan className="fill-warn">|</tspan>
      </text>

      <text x="2" y="262" fontSize="9" className="fill-muted">
        {wrap(t("tour.scene.togetherCaption"), 88).map((line, i) => (
          <tspan key={i} x="2" dy={i === 0 ? 0 : 13}>
            {line}
          </tspan>
        ))}
      </text>
    </>
  );
}

/* ── Builder: a sentence becomes a widget, written, checked and fixed ──── */

export function BuilderScene({ frame: f }: SceneProps) {
  const t = useT();
  const prompt = t("tour.scene.prompt");

  // The sentence is typed over the first two beats and sent on beat 3; the
  // steps arrive one a beat, the widget draws itself on beat 7, a to-do is
  // ticked on beat 8, and the end of the loop clears it all. The steps are
  // what Build with AI does on this box: one file, checked, one fix.
  const fade = 1 - prog(f, 11 * BEAT, 11 * BEAT + 24, E.smooth);
  const text = typed(f, 10, prompt, 2);
  const send = hitPulse(f - 3 * BEAT, 2, 5);
  const caret = f < 3 * BEAT && fd(f / 16) % 2 === 0;
  const steps = [
    { label: t("tour.scene.stepWrite", { file: "index.html" }), at: 3 * BEAT + 20 },
    { label: t("tour.scene.stepCheck"), at: 4 * BEAT + 20, warn: true },
    { label: t("tour.scene.stepFix"), at: 5 * BEAT + 20 },
  ];
  const app = prog(f, 6 * BEAT + 10, 7 * BEAT + 4, E.out);
  const tick = prog(f, 8 * BEAT, 8 * BEAT + 10, E.ui);
  const spin = (f * 6) % 360;

  const preview = { x: 248, y: 104, w: 212, h: 150 };
  const todos = [t("tour.scene.todo1"), t("tour.scene.todo2"), t("tour.scene.todo3")];

  return (
    <>
      <Heading>{t("look.editor.tab.ai")}</Heading>
      <Caption y={34}>{t("tour.scene.builderOnBox")}</Caption>

      {/* The sentence, and its send button. */}
      <Panel y={42} h={46} />
      <g className="text-accent">
        <Glyph icon={SparklesIcon} x={12} y={57} size={14} />
      </g>
      <text x="34" y="68.5" fontSize="10" className="fill-ink" opacity={f < 11 * BEAT ? 1 : fade}>
        {text}
        <tspan className="fill-ink" opacity={caret ? 1 : 0}>
          |
        </tspan>
      </text>
      <Pill cx={W - 40} cy={65} w={56} label={t("tour.scene.build")} press={send} />

      {/* What the builder did, one line a beat. */}
      <Panel y={104} w={236} h={150} />
      {steps.map((step, i) => {
        const o = prog(f, step.at, step.at + 14, E.out) * fade;
        const done = f > step.at + 18 || step.warn === true;
        const y = 116 + i * 30;
        return (
          <g key={i} opacity={o}>
            {done ? (
              <g className={step.warn === true ? "text-warn" : "text-ok"}>
                <Glyph icon={step.warn === true ? Alert02Icon : CheckmarkCircle02Icon} x={12} y={y + 3} size={13} />
              </g>
            ) : (
              <circle cx="18.5" cy={y + 9.5} r="5" fill="none" strokeWidth="1.4" strokeDasharray="20 12" className="stroke-accent" transform={`rotate(${spin} 18.5 ${y + 9.5})`} />
            )}
            <text x="32" y={y + 13} fontSize="9.5" className="fill-ink">
              {step.label}
            </text>
          </g>
        );
      })}

      {/* The widget, in its own frame on the page. */}
      <Panel x={preview.x} y={preview.y} w={preview.w} h={preview.h} sunk />
      <g opacity={app * fade} transform={`translate(0 ${mix(6, 0, app)})`}>
        <rect x={preview.x + 10.5} y={preview.y + 10.5} width={preview.w - 21} height={preview.h - 21} rx="5" className="fill-surface stroke-line" />
        <text x={preview.x + 22} y={preview.y + 32} fontSize="10.5" fontWeight="600" className="fill-ink">
          {t("tour.scene.appTitle")}
        </text>
        {todos.map((todo, i) => {
          const y = preview.y + 46 + i * 24;
          const checked = i === 2 ? 1 : i === 0 ? tick : 0;
          return (
            <g key={todo}>
              <rect x={preview.x + 22.5} y={y + 0.5} width="11" height="11" rx="3" className="fill-surface stroke-line" />
              <rect x={preview.x + 22} y={y} width="12" height="12" rx="3" className="fill-accent" opacity={checked} />
              <path
                d={`M ${preview.x + 25} ${y + 6} l 2.4 2.4 l 4.4 -4.8`}
                fill="none"
                strokeWidth="1.5"
                strokeLinecap="round"
                strokeLinejoin="round"
                className="stroke-surface"
                opacity={checked}
              />
              <text x={preview.x + 42} y={y + 9.5} fontSize="9.5" className={checked > 0.5 ? "fill-muted" : "fill-ink"}>
                {todo}
              </text>
            </g>
          );
        })}
      </g>
      <text x="2" y="276" fontSize="9" className="fill-muted">
        {wrap(t("tour.scene.builderCaption"), 88).map((line, i) => (
          <tspan key={i} x="2" dy={i === 0 ? 0 : 13}>
            {line}
          </tspan>
        ))}
      </text>
    </>
  );
}

/* ── Lend: memory lent to an AI model, paid by the token ───────────────── */

export function LendScene({ frame: f }: SceneProps) {
  const t = useT();
  const locale = useLocale();
  const usdc = (n: number, digits = 2): string =>
    formatNumber(n, locale, { minimumFractionDigits: digits, maximumFractionDigits: digits });
  const hatch = useSvgId("tour-hatch");

  // The model's four boxes light one a half-beat from beat 1, this one
  // first; the tokens flow and the earnings count up while it answers, and
  // the end of the loop puts everything back.
  const back = 1 - prog(f, 11 * BEAT, 11 * BEAT + 20, E.smooth);
  const lit = (i: number) => prog(f, BEAT + i * 15, BEAT + i * 15 + 12, E.ui) * back;
  const flowing = hold(f, 3 * BEAT, 10 * BEAT, 16);
  const tokens = Math.round(clamp((f - 3 * BEAT) / (7 * BEAT)) * 41200) * (back > 0.5 ? 1 : 0);
  const earned = (tokens * 0.9) / 1_000_000 / 4;
  const gap = 30;
  const travel = (f * 1.2) % gap;
  const boxes = ["mattbox", "evabox", "tomasbox", "lucibox"];

  return (
    <>
      <defs>
        <Hatch id={hatch} />
      </defs>
      <Heading>{t(paneById("market").labelKey)}</Heading>

      <Caption y={36}>{t("tour.scene.aiModels")}</Caption>
      <Panel y={42} h={150} />
      <text x="16" y="64" fontSize="10.5" fontWeight="600" className="fill-ink">
        Llama 3.3 70B
      </text>
      <text x="16" y="78" fontSize="8.5" className="fill-muted">
        {t("tour.scene.price", { in: usdc(0.6), out: usdc(0.9) })}
      </text>
      <text x={W - 16} y="64" fontSize="9" textAnchor="end" className="fill-muted">
        {t("tour.scene.onBoxes", { count: boxes.length })}
      </text>
      <line x1="16" y1="90.5" x2={W - 16} y2="90.5" className="stroke-hair" />
      {boxes.map((name, i) => {
        const x = 16 + i * 108;
        return (
          <g key={name}>
            <rect x={x + 0.5} y="100.5" width="96" height="24" rx="6" className="fill-sunk stroke-line" />
            <rect x={x + 1} y="101" width="95" height="23" rx="5.5" fill={`url(#${hatch})`} opacity={lit(i) * 0.9} />
            <text x={x + 48.5} y="116" fontSize="9" textAnchor="middle" className={lit(i) > 0.5 ? "fill-surface font-mono" : "fill-ink font-mono"}>
              {name}
            </text>
          </g>
        );
      })}
      <line x1="16" y1="150" x2="200" y2="150" strokeWidth="1.5" className="stroke-line" />
      <g opacity={flowing}>
        {Array.from({ length: Math.ceil(184 / gap) + 1 }, (_, i) => {
          const x = 16 + travel + i * gap - gap;
          if (x < 18 || x > 190) return null;
          return <rect key={i} x={x} y="147" width="8" height="6" rx="1.5" fill={`url(#${hatch})`} />;
        })}
      </g>
      <text x="16" y="170" fontSize="8.5" className="fill-muted font-mono">
        {t("tour.scene.tokens", { count: tokens })}
      </text>

      <Caption y={214}>{t("tour.scene.thisBoxEarned")}</Caption>
      <Panel y={220} h={56} />
      <text x="16" y="252" fontSize="9.5" className="fill-ink font-mono">
        mattbox
      </text>
      <text x={W - 16} y="246" fontSize="14" textAnchor="end" className="fill-accent font-mono">
        +{usdc(earned, 4)} USDC
      </text>
      <text x={W - 16} y="262" fontSize="8.5" textAnchor="end" className="fill-muted">
        {t("tour.scene.paidByTheToken")}
      </text>
    </>
  );
}

/* ── By name: a payment sent to the box's name lands in its wallet ──────── */

const BALANCE = { before: 112.4, paid: 25 } as const;

export function NameScene({ frame: f }: SceneProps) {
  const t = useT();
  const locale = useLocale();
  const usdc = (n: number): string => formatNumber(n, locale, { minimumFractionDigits: 2, maximumFractionDigits: 2 });

  // The name is typed into someone's wallet over the first beats, the
  // payment goes on beat 4, arrives on beat 6 and the balance counts up to
  // meet it; the end of the loop takes it all back.
  const name = "mattbox.boxes.losos.dasmat.us";
  const back = 1 - prog(f, 11 * BEAT, 11 * BEAT + 20, E.smooth);
  const sent = typed(f, BEAT, name, 2);
  const send = hitPulse(f - 4 * BEAT, 2, 5);
  const arrive = hold(f, 6 * BEAT, 11 * BEAT, 14);
  const rise = mix(8, 0, prog(f, 6 * BEAT, 6 * BEAT + 20, E.out));
  const count = prog(f, 6 * BEAT + 10, 7 * BEAT + 20, E.outSoft) * back;
  const balance = BALANCE.before + BALANCE.paid * count;

  return (
    <>
      <Heading>{t(paneById("network").labelKey)}</Heading>

      <Caption y={36}>{t("tour.scene.theirWallet")}</Caption>
      <Panel y={42} h={70} />
      <text x="16" y="62" fontSize="8.5" className="fill-muted">
        {t("tour.scene.payTo")}
      </text>
      <rect x="16.5" y="70.5" width={W - 120} height="26" rx="5" className="fill-surface stroke-line" />
      <text x="26" y="87.5" fontSize="9.5" className="fill-ink font-mono" opacity={back}>
        {sent}
      </text>
      <Pill cx={W - 50} cy={83.5} w={64} label={`${usdc(BALANCE.paid)} USDC`} press={send} />

      <Caption y={136}>{t("tour.scene.byName")}</Caption>
      <Panel y={142} h={110} />
      <text x="16" y="164" fontSize="9.5" className="fill-ink font-mono">
        {name}
      </text>
      <text x={W - 16} y="164" fontSize="9" textAnchor="end" className="fill-muted">
        {t("tour.scene.balance")}
        <tspan dx="6" className="fill-ink font-mono">
          {usdc(balance)} USDC
        </tspan>
      </text>
      <line x1="16" y1="176.5" x2={W - 16} y2="176.5" className="stroke-hair" />
      <g opacity={arrive} transform={`translate(0 ${rise})`}>
        <rect x="8" y="186" width={W - 16} height="56" rx="6" className="fill-accent-wash" />
        <text x="18" y="208" fontSize="10" className="fill-ink">
          {t("tour.scene.received")}
        </text>
        <text x="18" y="223" fontSize="8.5" className="fill-muted">
          {t("tour.scene.toName", { name })}
        </text>
        <text x={W - 18} y="215" fontSize="12" textAnchor="end" className="fill-accent font-mono">
          +{usdc(BALANCE.paid)} USDC
        </text>
      </g>
    </>
  );
}

/* ── Chat: @ names someone, the room is sealed, a call starts ─────────── */

export function ChatScene({ frame: f }: SceneProps) {
  const t = useT();

  const draft = t("tour.scene.draft");
  // Beat 1 "@" opens the people picker, beat 2 Tomas is picked, the rest of
  // the reply is typed and sent on beat 4.5; beat 6.5 the call starts and
  // holds, and the end of the loop takes it all back.
  const end = 11 * BEAT;
  const fade = 1 - prog(f, end, end + 24, E.smooth);
  const typing = f < 4 * BEAT + 15;
  const at = typing && f >= BEAT && f < 2 * BEAT;
  const picker = hold(f, BEAT, 2 * BEAT, 10, E.ui);
  const pick = prog(f, BEAT + 15, BEAT + 25, E.ui);
  const chip = typing && f >= 2 * BEAT;
  const words = typing ? typed(f, 2 * BEAT + 10, draft, 3) : "";
  const send = hitPulse(f - (4 * BEAT + 10), 2, 5);
  const sent = prog(f, 4 * BEAT + 15, 5 * BEAT + 5, E.out) * fade;
  const ring = hitPulse(f - 6 * BEAT, 2, 8) + hitPulse(f - (6 * BEAT + 15), 2, 8);
  const call = prog(f, 6 * BEAT + 20, 7 * BEAT + 16, E.glide) * fade;
  const caret = fd(f / 16) % 2 === 0;

  const list = { w: 118 };
  const room = { x: list.w + 8, w: W - list.w - 8 };
  const composerY = 252;
  const band = t("tour.scene.roomBand");
  const rooms = [
    { name: band, note: "3", on: true },
    { name: "Eva", note: "" },
    { name: "Tomas", note: "1" },
  ];
  const people = [
    { name: "Tomas", initial: "T", tone: "ink" as const },
    { name: "Eva", initial: "E", tone: "warn" as const },
  ];

  return (
    <>
      <Heading>{t("tour.scene.chat")}</Heading>

      {/* Rooms. */}
      <Panel y={28} w={list.w} h={268} sunk />
      {rooms.map((r, i) => {
        const y = 36 + i * 30;
        return (
          <g key={r.name}>
            {r.on === true && <rect x="6" y={y} width={list.w - 12} height="26" rx="5" className="fill-surface" />}
            <g className={r.on === true ? "text-accent" : "text-muted"}>
              <Glyph icon={BubbleChatIcon} x={14} y={y + 7} size={12} />
            </g>
            <text x="32" y={y + 17} fontSize="9.5" className="fill-ink">
              {r.name}
            </text>
            <text x={list.w - 14} y={y + 17} fontSize="8.5" textAnchor="end" className="fill-muted font-mono">
              {r.note}
            </text>
          </g>
        );
      })}

      {/* The band's room, end-to-end encrypted. */}
      <Panel x={room.x} y={28} w={room.w} h={268} />
      <text x={room.x + 14} y="49.5" fontSize="10" fontWeight="600" className="fill-ink">
        {band}
      </text>
      <g className="text-ok">
        <Glyph icon={LockIcon} x={room.x + 18 + band.length * 6} y={41} size={10} />
      </g>
      <g transform={`translate(${room.x + room.w - 22} 46) scale(${1 + 0.18 * ring}) translate(-8 -8)`}>
        <g className={call > 0.01 || ring > 0.01 ? "text-accent" : "text-muted"}>
          <Glyph icon={Video01Icon} x={0} y={0} size={16} />
        </g>
      </g>
      <line x1={room.x + 1} y1="62.5" x2={room.x + room.w - 1} y2="62.5" className="stroke-hair" />

      <Face cx={room.x + 22} cy={84} r={8} initial="E" tone="warn" />
      <rect x={room.x + 36} y="72" width="160" height="24" rx="8" className="fill-sunk" />
      <text x={room.x + 46} y="88" fontSize="9.5" className="fill-ink">
        {t("tour.scene.asked")}
      </text>

      {/* Matus's answer, naming Tomas. */}
      <g opacity={sent} transform={`translate(0 ${mix(8, 0, sent)})`}>
        <rect x={room.x + room.w - 176} y="106" width="164" height="24" rx="8" className="fill-accent-wash" />
        <text x={room.x + room.w - 166} y="122" fontSize="9.5" className="fill-ink">
          <tspan className="fill-accent">@Tomas</tspan> {draft}
        </text>
      </g>

      {/* The picker "@" opens: the people in this room. */}
      <g opacity={picker} transform={`translate(0 ${mix(6, 0, picker)})`}>
        <rect x={room.x + 12.5} y={composerY - 70.5} width="150" height="64" rx="6" className="fill-surface stroke-line" />
        <rect x={room.x + 16} y={composerY - 67} width="143" height="27" rx="4" className="fill-sunk" opacity={pick} />
        {people.map((person, i) => (
          <g key={person.name}>
            <Face cx={room.x + 30} cy={composerY - 53.5 + i * 29} r={7} initial={person.initial} tone={person.tone} />
            <text x={room.x + 44} y={composerY - 50 + i * 29} fontSize="9" className="fill-ink">
              {person.name}
            </text>
          </g>
        ))}
      </g>

      {/* The composer. */}
      <rect x={room.x + 8.5} y={composerY + 0.5} width={room.w - 17} height="30" rx="6" className="fill-surface stroke-line" />
      <g className="text-faint">
        <Glyph icon={AtIcon} x={room.x + 16} y={composerY + 9} size={12} />
      </g>
      <text x={room.x + 34} y={composerY + 19} fontSize="9.5" className="fill-ink">
        {at && <tspan className="fill-accent">@</tspan>}
        {chip && <tspan className="fill-accent">@Tomas </tspan>}
        {words}
        <tspan className="fill-ink" opacity={caret && typing ? 1 : 0}>
          |
        </tspan>
      </text>
      <Pill cx={room.x + room.w - 40} cy={composerY + 15.5} w={50} label={t("tour.scene.send")} press={send} />

      {/* The call, over the conversation. */}
      {call > 0.001 && (
        <g opacity={call} transform={`translate(0 ${mix(10, 0, call)})`}>
          <rect x={room.x + room.w - 180.5} y="140.5" width="168" height="96" rx="8" className="fill-ink" opacity="0.92" />
          <rect x={room.x + room.w - 174} y="147" width="76" height="62" rx="5" className="fill-warn" opacity="0.85" />
          <rect x={room.x + room.w - 94} y="147" width="76" height="62" rx="5" className="fill-accent" opacity="0.85" />
          <text x={room.x + room.w - 136} y="184" fontSize="18" fontWeight="600" textAnchor="middle" className="fill-surface">
            E
          </text>
          <text x={room.x + room.w - 56} y="184" fontSize="18" fontWeight="600" textAnchor="middle" className="fill-surface">
            M
          </text>
          <g className="text-surface">
            <Glyph icon={Video01Icon} x={room.x + room.w - 174} y={217} size={10} />
          </g>
          <text x={room.x + room.w - 160} y="225" fontSize="8" className="fill-surface">
            {t("tour.scene.videoCall")}
          </text>
        </g>
      )}
    </>
  );
}

/* ── Profile: a banner and a picture, and the name under them ───────────── */

export function ProfileScene({ frame: f }: SceneProps) {
  const t = useT();
  const clip = useSvgId("tour-banner");

  // Beat 1 the banner button is pressed and the banner comes up; beat 3 the
  // picture's; the end of the loop hands both back.
  const end = 7 * BEAT;
  const bannerPress = hitPulse(f - BEAT, 2, 5);
  const banner = prog(f, BEAT + 6, 2 * BEAT + 10, E.out) * (1 - prog(f, end, end + 24, E.smooth));
  const facePress = hitPulse(f - 3 * BEAT, 2, 5);
  const face = prog(f, 3 * BEAT + 6, 4 * BEAT, E.out) * (1 - prog(f, end, end + 24, E.smooth));
  const scale = mix(0.6, 1, face);

  const card = { y: 32, h: 156 };
  const strip = { y: card.y, h: 86 };

  return (
    <>
      <defs>
        <clipPath id={clip}>
          <rect x="1" y={strip.y + 1} width={W - 2} height={strip.h} rx="7.5" />
        </clipPath>
      </defs>
      <Heading>{t(paneById("account").labelKey)}</Heading>
      <Panel y={card.y} h={card.h} />

      {/* The banner: the sunk empty strip, then a picture of hills on the water. */}
      <g clipPath={`url(#${clip})`}>
        <rect x="0" y={strip.y} width={W} height={strip.h} className="fill-sunk" />
        <g opacity={banner}>
          <rect x="0" y={strip.y} width={W} height={strip.h} className="fill-accent-wash" />
          <path d={`M 0 ${strip.y + 62} Q 90 ${strip.y + 22} 180 ${strip.y + 54} T 340 ${strip.y + 40} T ${W} ${strip.y + 50} V ${strip.y + strip.h} H 0 Z`} className="fill-accent" opacity="0.55" />
          <path d={`M 0 ${strip.y + 74} Q 120 ${strip.y + 52} 240 ${strip.y + 70} T ${W} ${strip.y + 64} V ${strip.y + strip.h} H 0 Z`} className="fill-accent" />
          <circle cx={W - 170} cy={strip.y + 26} r="10" className="fill-warn" opacity="0.8" />
        </g>
      </g>

      {/* The picture, over the banner's edge. */}
      <circle cx="56" cy={strip.y + strip.h} r="31" className="fill-surface" />
      <circle cx="56" cy={strip.y + strip.h} r="27" className="fill-sunk stroke-line" />
      <g transform={`translate(56 ${strip.y + strip.h}) scale(${scale}) translate(-56 ${-(strip.y + strip.h)})`} opacity={face}>
        <Face cx={56} cy={strip.y + strip.h} r={27} initial="M" />
      </g>

      <text x="96" y={strip.y + strip.h + 22} fontSize="13" fontWeight="600" className="fill-ink">
        Matus
      </text>
      <text x="96" y={strip.y + strip.h + 37} fontSize="9" className="fill-muted font-mono">
        @matus:mattbox.local
      </text>
      <Pill cx={W - 70} cy={strip.y + strip.h + 30} w={112} label={t("tour.scene.changePicture")} press={facePress} primary={false} />
      <Pill cx={W - 70} cy={strip.y + 18} w={112} label={t("tour.scene.changeBanner")} press={bannerPress} primary={false} />

      <text x="2" y="208" fontSize="9" className="fill-muted">
        {wrap(t("tour.scene.profileCaption"), 88).map((line, i) => (
          <tspan key={i} x="2" dy={i === 0 ? 0 : 13}>
            {line}
          </tspan>
        ))}
      </text>
    </>
  );
}
