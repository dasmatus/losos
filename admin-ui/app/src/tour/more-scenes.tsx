/* The tour's scenes for what LosOS cloud, LosOS Git and the mesh add to a
 * box: the Board, writing together, building an app, being paid, chat and
 * the profile. Same rules as scenes.tsx: each is a pure function of its
 * frame on the 120 BPM grid (a beat is 30 frames), drawn in the 460 by 304
 * content box, every word from the catalogue so it reads in the language on
 * screen, and the same box throughout: mattbox, with Eva on evabox. */

import {
  Alert02Icon,
  AtIcon,
  BubbleChatIcon,
  CheckmarkCircle02Icon,
  Doc01Icon,
  GitBranchIcon,
  GitCommitIcon,
  LockIcon,
  Mail01Icon,
  SparklesIcon,
  Video01Icon,
  Xls01Icon,
} from "@hugeicons/core-free-icons";
import { formatNumber } from "@/lib/i18n";
import { useLocale, useT } from "@/lib/i18n-react";
import { paneById } from "@/screens/settings/panes";
import { BEAT, E, clamp, fd, hitPulse, hold, mix, prog } from "./motion";
import { Glyph, Hatch, useSvgId } from "./mini-page";
import { Caption, Face, Heading, Panel, Pill, W, type SceneProps, typed, wrap } from "./parts";

/* ── Board: an issue in LosOS Git becomes a card, a commit closes it ────── */

const COLUMN = { w: 148, gap: 8, top: 42, h: 196 } as const;
const CARD = { h: 40, gap: 6 } as const;

function columnX(i: number): number {
  return i * (COLUMN.w + COLUMN.gap);
}

function cardY(row: number): number {
  return COLUMN.top + 26 + row * (CARD.h + CARD.gap);
}

function BoardCard({ x, y, title, issue, opacity = 1, lit = 0 }: { x: number; y: number; title: string; issue?: string; opacity?: number; lit?: number }) {
  return (
    <g opacity={opacity}>
      <rect x={x + 6.5} y={y + 0.5} width={COLUMN.w - 13} height={CARD.h - 1} rx="6" className="fill-surface stroke-line" />
      <rect x={x + 6.5} y={y + 0.5} width={COLUMN.w - 13} height={CARD.h - 1} rx="6" fill="none" strokeWidth="1.5" className="stroke-accent" opacity={lit} />
      <text x={x + 14} y={y + 16} fontSize="9.5" className="fill-ink">
        {title}
      </text>
      {issue !== undefined && (
        <g>
          <g className="text-muted">
            <Glyph icon={GitBranchIcon} x={x + 13} y={y + 22} size={10} />
          </g>
          <text x={x + 27} y={y + 31} fontSize="8.5" className="fill-muted font-mono">
            {issue}
          </text>
        </g>
      )}
    </g>
  );
}

export function BoardScene({ frame: f }: SceneProps) {
  const t = useT();
  const columns = [t("tour.scene.todo"), t("tour.scene.doing"), t("tour.scene.done")];

  // Beat 1 the issue arrives at the top of To do and the card under it
  // makes room; beat 3.5 the commit that closes it is pushed; beat 5 the
  // card glides to Done; the end of the loop puts everything back.
  const arrive = prog(f, BEAT, 2 * BEAT, E.out) * (1 - prog(f, 11 * BEAT, 11 * BEAT + 20, E.smooth));
  const room = prog(f, BEAT - 6, 2 * BEAT - 6, E.glide) * (1 - prog(f, 5 * BEAT, 6 * BEAT, E.glide));
  const commit = hold(f, 3 * BEAT + 15, 10 * BEAT + 20, 16);
  const push = hitPulse(f - (4 * BEAT + 10), 2, 5);
  const move = prog(f, 5 * BEAT, 6 * BEAT + 10, E.glide);
  const doneShift = prog(f, 5 * BEAT + 6, 6 * BEAT, E.glide);
  const lit = hold(f, 6 * BEAT + 10, 9 * BEAT, 14);

  const issue = { title: t("tour.scene.issue"), id: "#14" };
  const card = {
    x: mix(columnX(0), columnX(2), move),
    y: cardY(0) - mix(8, 0, prog(f, BEAT, 2 * BEAT, E.out)) * (1 - move),
  };

  return (
    <>
      <Heading>{t("tour.scene.board")}</Heading>
      <text x={W} y="14" fontSize="9" textAnchor="end" className="fill-muted font-mono">
        matus/website
      </text>
      <Caption y={34}>LosOS Git</Caption>

      {columns.map((name, i) => (
        <g key={name}>
          <Panel x={columnX(i)} y={COLUMN.top} w={COLUMN.w} h={COLUMN.h} sunk />
          <text x={columnX(i) + 12} y={COLUMN.top + 17} fontSize="9.5" fontWeight="600" className="fill-ink">
            {name}
          </text>
        </g>
      ))}

      <BoardCard x={columnX(0)} y={cardY(room)} title={t("tour.scene.cardAbout")} issue="#9" />
      <BoardCard x={columnX(1)} y={cardY(0)} title={t("tour.scene.cardLogo")} issue="#11" />
      <BoardCard x={columnX(2)} y={cardY(doneShift)} title={t("tour.scene.cardDomain")} issue="#3" />
      <BoardCard x={card.x} y={card.y} title={issue.title} issue={issue.id} opacity={arrive} lit={lit} />

      {/* The commit, as LosOS Git's push shows it. */}
      <g opacity={commit} transform={`translate(0 ${mix(6, 0, prog(f, 3 * BEAT + 15, 4 * BEAT, E.out))})`}>
        <g transform={`translate(${W / 2} 266) scale(${1 - 0.02 * push}) translate(${-W / 2} -266)`}>
          <Panel y={248} h={36} />
          <g className="text-accent">
            <Glyph icon={GitCommitIcon} x={14} y={259} size={14} />
          </g>
          <text x="36" y="270" fontSize="9.5" className="fill-muted font-mono">
            a41c9e0
          </text>
          <text x="86" y="270" fontSize="9.5" className="fill-ink font-mono">
            closes {issue.id}
          </text>
          <text x={W - 14} y="270" fontSize="9" textAnchor="end" className="fill-muted font-mono">
            main
          </text>
        </g>
      </g>
    </>
  );
}

/* ── Together: one document, two people on two boxes typing at once ─────── */

export function TogetherScene({ frame: f }: SceneProps) {
  const t = useT();
  const hatch = useSvgId("tour-hatch");

  const mine = t("tour.scene.typedMine");
  const hers = t("tour.scene.typedHers");
  // Both type from beat 1; at the end of the loop both lines are taken back.
  const back = (text: string, at: number) => clamp(text.length - fd((f - at) / 2), 0, text.length);
  const mineNow = f < 10 * BEAT ? typed(f, BEAT, mine, 5) : mine.slice(0, back(mine, 10 * BEAT));
  const hersNow = f < 10 * BEAT ? typed(f, BEAT + 20, hers, 4) : hers.slice(0, back(hers, 10 * BEAT + 10));
  const blink = fd(f / 16) % 2 === 0;
  const flag = hold(f, BEAT + 10, 10 * BEAT + 30, 14);
  const lineOn = hold(f, BEAT, 11 * BEAT, 18);
  const gap = 34;
  const travel = (f * 0.9) % gap;

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
      <defs>
        <Hatch id={hatch} />
      </defs>
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

      {/* Matus, here, and Eva on her own box, each with a caret in their
          colour; her name rides at the start of her line. The carets are
          text, so they sit where the letters end in any language. */}
      <text x={left} y={mineY} fontSize="9.5" className="fill-ink">
        {mineNow}
        <tspan className="fill-accent" opacity={blink ? 1 : 0}>
          |
        </tspan>
      </text>
      <g opacity={flag}>
        <rect x={left - 2} y={hersY - 23} width="62" height="12" rx="3" className="fill-warn" />
        <text x={left + 2} y={hersY - 14.2} fontSize="8" className="fill-surface">
          Eva · evabox
        </text>
      </g>
      <text x={left} y={hersY} fontSize="9.5" className="fill-ink">
        {hersNow}
        <tspan className="fill-warn">|</tspan>
      </text>

      {/* The two boxes, and the edits travelling between them. */}
      <Panel y={250} h={46} />
      <rect x="16.5" y="262.5" width="80" height="22" rx="6" className="fill-sunk stroke-line" />
      <text x="56.5" y="277" fontSize="9.5" textAnchor="middle" className="fill-ink font-mono">
        mattbox
      </text>
      <rect x={W - 96.5} y="262.5" width="80" height="22" rx="6" className="fill-sunk stroke-line" />
      <text x={W - 56.5} y="277" fontSize="9.5" textAnchor="middle" className="fill-ink font-mono">
        evabox
      </text>
      <line x1="98" y1="273.5" x2={W - 98} y2="273.5" strokeWidth="1.5" className="stroke-line" />
      <line x1="98" y1="273.5" x2={W - 98} y2="273.5" strokeWidth="1.5" className="stroke-accent" opacity={lineOn} />
      <g opacity={lineOn}>
        {Array.from({ length: 6 }, (_, i) => {
          // Both ways along one line: Matus's letters go right, Eva's come back left.
          const right = i % 2 === 0;
          const d = (travel + Math.floor(i / 2) * gap * 2.4) % (W - 216);
          const x = right ? 104 + d : W - 112 - d;
          return <rect key={i} x={x} y="270.5" width="8" height="6" rx="1.5" fill={`url(#${hatch})`} />;
        })}
        <text x={W / 2} y="262" fontSize="8.5" textAnchor="middle" className="fill-accent">
          {t("tour.scene.live")}
        </text>
      </g>
    </>
  );
}

/* ── Builder: a sentence becomes an app, written, checked and fixed ─────── */

export function BuilderScene({ frame: f }: SceneProps) {
  const t = useT();
  const prompt = t("tour.scene.prompt");

  // The sentence is typed over the first two beats and sent on beat 3; the
  // steps arrive one a beat, the app draws itself on beat 7, a to-do is
  // ticked on beat 8, and the end of the loop clears it all.
  const fade = 1 - prog(f, 11 * BEAT, 11 * BEAT + 24, E.smooth);
  const text = typed(f, 10, prompt, 2);
  const send = hitPulse(f - 3 * BEAT, 2, 5);
  const caret = f < 3 * BEAT && fd(f / 16) % 2 === 0;
  const steps = [
    { label: t("tour.scene.stepWrite", { file: "index.html" }), at: 3 * BEAT + 20 },
    { label: t("tour.scene.stepWrite", { file: "app.js" }), at: 4 * BEAT + 10 },
    { label: t("tour.scene.stepCheck"), at: 5 * BEAT, warn: true },
    { label: t("tour.scene.stepFix"), at: 5 * BEAT + 20 },
  ];
  const app = prog(f, 6 * BEAT + 10, 7 * BEAT + 4, E.out);
  const tick = prog(f, 8 * BEAT, 8 * BEAT + 10, E.ui);
  const spin = (f * 6) % 360;

  const preview = { x: 248, y: 104, w: 212, h: 150 };
  const todos = [t("tour.scene.todo1"), t("tour.scene.todo2"), t("tour.scene.todo3")];

  return (
    <>
      <Heading>{t("tour.scene.builder")}</Heading>
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

      {/* The app, running in its own frame in the browser. */}
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

/* ── Paid: AI models paid by the token, and a payment sent to a name ────── */

const BALANCE = { before: 112.4, paid: 25 } as const;

export function PaidScene({ frame: f }: SceneProps) {
  const t = useT();
  const locale = useLocale();
  const usdc = (n: number, digits = 2): string =>
    formatNumber(n, locale, { minimumFractionDigits: digits, maximumFractionDigits: digits });
  const hatch = useSvgId("tour-hatch");

  // The model's four boxes light one a half-beat from beat 1; the tokens
  // flow and the spend counts up while it answers; on beat 6 a payment for
  // mattbox's name arrives and the balance counts up to meet it.
  const lit = (i: number) => prog(f, BEAT + i * 15, BEAT + i * 15 + 12, E.ui) * (1 - prog(f, 11 * BEAT, 11 * BEAT + 20, E.smooth));
  const flowing = hold(f, 3 * BEAT, 10 * BEAT, 16);
  const tokens = Math.round(clamp((f - 3 * BEAT) / (7 * BEAT)) * 41200);
  const spent = (tokens * 0.9) / 1_000_000;
  const arrive = hold(f, 6 * BEAT, 11 * BEAT, 14);
  const rise = mix(8, 0, prog(f, 6 * BEAT, 6 * BEAT + 20, E.out));
  const count = prog(f, 6 * BEAT + 10, 7 * BEAT + 20, E.outSoft) * (1 - prog(f, 11 * BEAT, 11 * BEAT + 20, E.smooth));
  const balance = BALANCE.before + BALANCE.paid * count;
  const gap = 30;
  const travel = (f * 1.2) % gap;

  return (
    <>
      <defs>
        <Hatch id={hatch} />
      </defs>
      <Heading>{t(paneById("market").labelKey)}</Heading>

      <Caption y={36}>{t("tour.scene.aiModels")}</Caption>
      <Panel y={42} h={104} />
      <text x="16" y="64" fontSize="10.5" fontWeight="600" className="fill-ink">
        Llama 3.3 70B
      </text>
      <text x="16" y="78" fontSize="8.5" className="fill-muted">
        {t("tour.scene.price", { in: usdc(0.6), out: usdc(0.9) })}
      </text>
      <text x={W - 16} y="64" fontSize="9" textAnchor="end" className="fill-muted">
        {t("tour.scene.onBoxes", { count: 4 })}
      </text>
      {Array.from({ length: 4 }, (_, i) => (
        <g key={i}>
          <rect x={W - 98.5 + i * 21} y="70.5" width="16" height="11" rx="3" className="fill-sunk stroke-line" />
          <rect x={W - 98 + i * 21} y="71" width="15" height="10" rx="2.5" fill={`url(#${hatch})`} opacity={lit(i)} />
        </g>
      ))}
      <line x1="16" y1="94.5" x2={W - 16} y2="94.5" className="stroke-hair" />
      <line x1="16" y1="118" x2="200" y2="118" strokeWidth="1.5" className="stroke-line" />
      <g opacity={flowing}>
        {Array.from({ length: Math.ceil(184 / gap) + 1 }, (_, i) => {
          const x = 16 + travel + i * gap - gap;
          if (x < 18 || x > 190) return null;
          return <rect key={i} x={x} y="115" width="8" height="6" rx="1.5" fill={`url(#${hatch})`} />;
        })}
      </g>
      <text x="16" y="136" fontSize="8.5" className="fill-muted font-mono">
        {t("tour.scene.tokens", { count: tokens })}
      </text>
      <text x={W - 16} y="122" fontSize="12" textAnchor="end" className="fill-ink font-mono">
        {usdc(spent, 4)} USDC
      </text>
      <text x={W - 16} y="136" fontSize="8.5" textAnchor="end" className="fill-muted">
        {t("tour.scene.paidByTheToken")}
      </text>

      <Caption y={168}>{t("tour.scene.byName")}</Caption>
      <Panel y={174} h={110} />
      <text x="16" y="196" fontSize="9.5" className="fill-ink font-mono">
        mattbox.boxes.losos.dasmat.us
      </text>
      <text x={W - 16} y="196" fontSize="9" textAnchor="end" className="fill-muted">
        {t("tour.scene.balance")}
        <tspan dx="6" className="fill-ink font-mono">
          {usdc(balance)} USDC
        </tspan>
      </text>
      <line x1="16" y1="208.5" x2={W - 16} y2="208.5" className="stroke-hair" />
      <g opacity={arrive} transform={`translate(0 ${rise})`}>
        <rect x="8" y="216" width={W - 16} height="56" rx="6" className="fill-accent-wash" />
        <text x="18" y="238" fontSize="10" className="fill-ink">
          {t("tour.scene.received")}
        </text>
        <text x="18" y="253" fontSize="8.5" className="fill-muted">
          {t("tour.scene.toName", { name: "mattbox.boxes.losos.dasmat.us" })}
        </text>
        <text x={W - 18} y="245" fontSize="12" textAnchor="end" className="fill-accent font-mono">
          +{usdc(BALANCE.paid)} USDC
        </text>
      </g>
    </>
  );
}

/* ── Chat: @ shares a file, the message is sealed, a call starts ────────── */

export function ChatScene({ frame: f }: SceneProps) {
  const t = useT();

  const draft = t("tour.scene.draft");
  // Beat 1 the reply is typed, beat 2.5 "@" opens the picker, beat 3.5 the
  // file is picked, beat 4.5 it is sent; beat 6.5 the call starts and holds,
  // and the end of the loop takes it all back.
  const end = 11 * BEAT;
  const fade = 1 - prog(f, end, end + 24, E.smooth);
  const words = f < 4 * BEAT + 15 ? typed(f, BEAT, draft, 3) : "";
  const at = f >= 2 * BEAT + 20 && f < 3 * BEAT + 20;
  const picker = hold(f, 2 * BEAT + 20, 3 * BEAT + 20, 10, E.ui);
  const pick = prog(f, 3 * BEAT, 3 * BEAT + 10, E.ui);
  const chip = f >= 3 * BEAT + 20 && f < 4 * BEAT + 15;
  const send = hitPulse(f - (4 * BEAT + 10), 2, 5);
  const sent = prog(f, 4 * BEAT + 15, 5 * BEAT + 5, E.out) * fade;
  const ring = hitPulse(f - 6 * BEAT, 2, 8) + hitPulse(f - (6 * BEAT + 15), 2, 8);
  const call = prog(f, 6 * BEAT + 20, 7 * BEAT + 16, E.glide) * fade;
  const caret = fd(f / 16) % 2 === 0;

  const list = { w: 118 };
  const room = { x: list.w + 8, w: W - list.w - 8 };
  const composerY = 252;
  const rooms = [
    { icon: BubbleChatIcon, name: "Eva", note: "evabox", on: true },
    { icon: BubbleChatIcon, name: t("tour.scene.roomBand"), note: "3" },
    { icon: Mail01Icon, name: t("tour.scene.mail"), note: "2" },
  ];

  return (
    <>
      <Heading>{t("tour.scene.chat")}</Heading>

      {/* Rooms, and Mail beside them. */}
      <Panel y={28} w={list.w} h={268} sunk />
      {rooms.map((r, i) => {
        const y = 36 + i * 30;
        return (
          <g key={r.name}>
            {r.on === true && <rect x="6" y={y} width={list.w - 12} height="26" rx="5" className="fill-surface" />}
            <g className={r.on === true ? "text-accent" : "text-muted"}>
              <Glyph icon={r.icon} x={14} y={y + 7} size={12} />
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

      {/* The conversation with Eva. */}
      <Panel x={room.x} y={28} w={room.w} h={268} />
      <Face cx={room.x + 20} cy={46} r={8} initial="E" tone="warn" />
      <text x={room.x + 34} y="49.5" fontSize="10" fontWeight="600" className="fill-ink">
        Eva
      </text>
      <g className="text-ok">
        <Glyph icon={LockIcon} x={room.x + 58} y={41} size={10} />
      </g>
      <g transform={`translate(${room.x + room.w - 22} 46) scale(${1 + 0.18 * ring}) translate(-8 -8)`}>
        <g className={call > 0.01 || ring > 0.01 ? "text-accent" : "text-muted"}>
          <Glyph icon={Video01Icon} x={0} y={0} size={16} />
        </g>
      </g>
      <line x1={room.x + 1} y1="62.5" x2={room.x + room.w - 1} y2="62.5" className="stroke-hair" />

      <rect x={room.x + 12} y="72" width="176" height="24" rx="8" className="fill-sunk" />
      <text x={room.x + 22} y="88" fontSize="9.5" className="fill-ink">
        {t("tour.scene.asked")}
      </text>

      {/* Matus's answer, with the file it names. */}
      <g opacity={sent} transform={`translate(0 ${mix(8, 0, sent)})`}>
        <rect x={room.x + room.w - 200} y="106" width="188" height="48" rx="8" className="fill-accent-wash" />
        <text x={room.x + room.w - 190} y="122" fontSize="9.5" className="fill-ink">
          {draft}
        </text>
        <rect x={room.x + room.w - 190.5} y="129.5" width="110" height="18" rx="4" className="fill-surface stroke-line" />
        <g className="text-ok">
          <Glyph icon={Xls01Icon} x={room.x + room.w - 186} y={133} size={10} />
        </g>
        <text x={room.x + room.w - 172} y="142" fontSize="8.5" className="fill-ink font-mono">
          {t("tour.scene.fileName")}
        </text>
      </g>

      {/* The picker "@" opens: files and repositories in one list. */}
      <g opacity={picker} transform={`translate(0 ${mix(6, 0, picker)})`}>
        <rect x={room.x + 12.5} y={composerY - 70.5} width="190" height="64" rx="6" className="fill-surface stroke-line" />
        <rect x={room.x + 16} y={composerY - 67} width="183" height="27" rx="4" className="fill-sunk" opacity={pick} />
        <g className="text-ok">
          <Glyph icon={Xls01Icon} x={room.x + 22} y={composerY - 61} size={12} />
        </g>
        <text x={room.x + 40} y="194" fontSize="9" className="fill-ink font-mono">
          {t("tour.scene.fileName")}
        </text>
        <text x={room.x + 196} y="194" fontSize="8" textAnchor="end" className="fill-muted">
          {t("tour.scene.fileKind")}
        </text>
        <g className="text-muted">
          <Glyph icon={GitBranchIcon} x={room.x + 22} y={composerY - 34} size={12} />
        </g>
        <text x={room.x + 40} y="225" fontSize="9" className="fill-ink font-mono">
          matus/website
        </text>
        <text x={room.x + 196} y="225" fontSize="8" textAnchor="end" className="fill-muted">
          LosOS Git
        </text>
      </g>

      {/* The composer. */}
      <rect x={room.x + 8.5} y={composerY + 0.5} width={room.w - 17} height="30" rx="6" className="fill-surface stroke-line" />
      <g className="text-faint">
        <Glyph icon={AtIcon} x={room.x + 16} y={composerY + 9} size={12} />
      </g>
      <text x={room.x + 34} y={composerY + 19} fontSize="9.5" className="fill-ink">
        {words}
        {at && <tspan className="fill-accent">{" @"}</tspan>}
        {chip && (
          <tspan dx="4" className="fill-accent font-mono">
            {t("tour.scene.fileName")}
          </tspan>
        )}
        <tspan className="fill-ink" opacity={caret && f < 4 * BEAT + 15 ? 1 : 0}>
          |
        </tspan>
      </text>
      <Pill cx={room.x + room.w - 40} cy={composerY + 15.5} w={50} label={t("tour.scene.send")} press={send} />

      {/* The call, sealed end to end, over the conversation. */}
      {call > 0.001 && (
        <g opacity={call} transform={`translate(0 ${mix(10, 0, call)})`}>
          <rect x={room.x + room.w - 180.5} y="166.5" width="168" height="76" rx="8" className="fill-black" opacity="0.85" />
          <rect x={room.x + room.w - 174} y="173" width="76" height="52" rx="5" className="fill-warn" opacity="0.85" />
          <rect x={room.x + room.w - 94} y="173" width="76" height="52" rx="5" className="fill-accent" opacity="0.85" />
          <text x={room.x + room.w - 136} y="205" fontSize="18" fontWeight="600" textAnchor="middle" className="fill-white">
            E
          </text>
          <text x={room.x + room.w - 56} y="205" fontSize="18" fontWeight="600" textAnchor="middle" className="fill-white">
            M
          </text>
          <g className="text-white">
            <Glyph icon={LockIcon} x={room.x + room.w - 174} y={229} size={9} />
          </g>
          <text x={room.x + room.w - 162} y="236.5" fontSize="8" className="fill-white">
            {t("tour.scene.sealed")}
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
