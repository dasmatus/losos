# LosOS Tag — design

Date: 2026-11-03. Status: approved direction (user-directed), implementation under way.

## Intent

LosOS Chat gains an agentic assistant, **LosOS Tag**. People tag it in a
thread the way they tag `@losos-git` today; it answers in the thread with
what the unified search over the box found, and can act on the box within
its owner-defined clearance. It behaves like Claude's tag or Grok bot: a
mention-triggered participant that reads the conversation's context.

Four capabilities ride with it:

1. **Context-aware suggestion.** When the room's recent messages hint an
   answer needs the box's data (summaries, "what did we decide", when
   questions pile up unanswered), the composer offers a Tag pill the person
   can accept instead of typing `@LosOS` themselves.
2. **User-defined assistants.** Anyone with a LosOS account can define
   assistants — name, persona (system prompt), clearance, colour — posted
   as real Matrix users of the app service namespace. Every assistant can
   be tagged as a participant and can be tagged into conversation with
   other assistants: a message naming two assistants fans out to both, and
   a reply that names another assistant passes the turn on (bounded
   depth), so assistants talk to each other in the room.
3. **User-defined confidentiality levels.** The box's owners define named
   levels (`everyday`, `personal`, `secret`, …), ordered, and attach areas
   to them (LosOS Cloud paths, LosOS Git repositories, chat rooms, Mail
   folders). An assistant has a clearance: it may touch an area only when
   the area's level is at or **below** its clearance — the more private
   the area, the higher (a lower, stronger level) needed. Levels and areas
   are per-box configuration, defined in the SPA's Settings screen, and
   the same gate services the companion tools and the unified search.
4. **Queue with memory-pressure semantics.** Asks run on a persistent
   queue that pauses rather than drops while the box is low on memory,
   and reaps idle holders aggressively under pressure — the shape the
   T3 Code fork built (`HostResources`, `BackgroundPolicy`,
   `ProviderSessionReaper`).

## The two layers the harness and the model sit on

- The **tag agent's turn** (the agentic part: reading tools, deciding what
  to search, writing the reply) runs on an **agent harness**. Harnesses are
  pluggable processes; the **built-in default is T3 Code**
  (open-source, pingdotgg/t3code), with **OpenCode** the fallback when T3
  Code is not configured on the box. T3 Code has no Ollama provider, so
  harness turns take their model from the harness's own provider settings;
  nothing in this feature wires Ollama into T3 Code.
- The **box's own language model** is unified: one Ollama model for the
  assistant plugin's "Ask AI" features and the Builder — not one coder and
  one talker. The built-in model is **Bonsai 27B**
  (`MichelRosselli/bonsai-27b`: 4.4 GB Q1_0 weights of Qwen3.6-27B,
  262k context, text + vision + **tools + thinking**, Apache 2.0,
  Ollama ≥ 0.32.5). Bonsai's tool-calling answers the tag when the
  harness is not configured *and* for the lowest-traffic path (plain
  "Ask AI" questions stay exactly as they are, re-pointed at the unified
  model). The builder keeps its own prompts and loop; only the model tag
  changes.

### Spillover (50–75)

A 4.4 GB model does not fit the worst box (4 GiB RAM, no GPU). When the
selected model's memory need exceeds the box's headroom, the runner
spills **50% (normal) to 75% (under memory pressure)** of the layers to
the same ggml-RPC lenders the mesh already runs
(`modules/inference-lend.nix`, `ggml-rpc-server -d CPU`), via the head's
existing `--rpc … -ts` tensor split (`modules/edge-inference.nix`). On a
GPU box the same knob becomes the CPU↔GPU split (Ollama's loader already
falls back layer-by-layer; the config sets the *target* spill). The
effective spill climbs under the **same memory-pressure predicate the
queue uses** — normal `start` (50), pressured `pressured` (75),
`losos.ai.spillover.*`.

## Backend

`backend/src/tag/` (lososd core routes, not a WASM plugin: the queue, the
harness spawner and the chat listener need processes and loopback sockets
— the reader-plugin exception, argued in the module docs; sections.json
gets entries):

- **State** rides in `state.json` (`model::State.tag`, serde default):
  `levels: [Level{name, rank}]`, `areas: [Area{kind, path, level}]`,
  `assistants: [Assistant{user_id, display_name, prompt, clearance,
  colour, enabled}]`, plus `queue` and `suggested: bool` bits.
- **Routes** (all `signed-in`, the owner's own actions admin-only):
  `GET /api/tag`, `POST /api/tag/levels|areas|assistants` (CRUD in the
  bodies), `POST /api/tag/asks` (a manual push, used by the suggestion
  pill and tests), `GET /api/tag/queue`, plus `GET /api/search` —
  **unified search**, the one query merged across the fts index
  (cloud text), git grep, mail JMAP (the person's mailboxes only) and
  the person's rooms' history (Synapse search event endpoint via the
  AS), every hit carrying the area's confidentiality grade.
- **Access rule.** An ask carries `person` and `assistant`. Every search
  hit is filtered: hit's area level `≤` assistant's clearance. The
  companion (`backend/access/tokens.rs`) keeps its fixed GET-only list;
  the confidential part is enforced where each source is queried, not
  once at the door.
- **Listener.** While chat is live (`lososInternal.chat`), a lososd thread
  polls Synapse `/sync` as the AS and, for messages that §mention an
  assistant, enqueues an ask with the thread's tail as context; reply
  posts go out exactly like the existing app service notices
  (`Chat::git_notice` generalized to an arbitrary agent user:
  `send_as(user, room, …)`). Depth cap of 3 assistant↔assistant
  handoffs per room per minute.
- **Queue.** `/var/lib/losos/tag/queue.json` (owned with `state.json`'s
  own rename discipline). Job: `queued → running → done|failed|stopped`,
  `paused` is a temporary overwrite while pressure holds, restored on
  the next sweep (never dropped). Reaper: idle-holder sweep every 5 min,
  compressed to 60 s while under pressure, always skipping a holder with
  an active turn; every reap logs `reason: memory_pressure |
  inactivity_threshold`. Memory predicate: `<10%` available (the fork's
  0.1 fraction), probed from `/proc/meminfo` with a ~5 s cache.
- **Harness adapter.** `tag/harness.rs`: `Harness::from_config()` picks
  T3 Code (`losos.tag.t3Command`, present ⇒ default) else OpenCode
  (`opencode run …`), else the built-in model path (Ollama chat with
  Bonsai's tool calls interpreted by lososd). Workspace per job under
  `/var/lib/losos-tag/<job>` as the `losos-tag` user.

## Modules / Nix

- `modules/tag.nix` (`losos.tag.*`): `enable` (gated on chat live like
  mail), `model = "MichelRosselli/bonsai-27b"`, `harness = "t3-code" |
  "opencode"`, `t3Command`, `spillover.start = 50`, `spillover.pressured
  = 75`. Pulls the model once through the loader pattern. options.nix
  entries; sections.json lines; builder.nix switch to the unified model
  for both `LOSOS_ASSISTANT_MODEL` and `LOSOS_BUILDER_MODEL`.
- The SPA: Settings › Tag (levels + areas + assistants), Chat suggestion
  pill, i18n (en/sk/de all three), handbook page (en/sk/de).

## Non-goals

- No write actions before levels exist: an assistant's *mutable* verbs
  stay out of the first cut; read/search/answer only, clearance decides
  **what it may read**.
- T3 Code keeps its own provider keys; the box adds none. Where T3 Code
  is not configured, OpenCode runs with the owner's own provider config.
- No cross-box assistant federation in this cut.

## Tests

- Unit: fine-search filter, level ordering, queue pause/restore + reaper
  thresholds, assistant CRUD validation, mention fan-out depth.
- `backend/tests/` daemon harness: ask lifecycle with a fake harness
  (`LOSOS_TAG_HARNESS=fake`), unified search across two sources with one
  area gated, assistant replies into a fake homeserver.
- VM tests not run in this session (no KVM): stated in the PR.
