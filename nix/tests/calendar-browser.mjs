// The browser half of nix/tests/calendar.nix: LosOS Calendar in Chromium,
// and the pictures of it.
//
//   node calendar-browser.mjs before <out>  the admin pages before LosOS Calendar
//   node calendar-browser.mjs on     <out>  the calendar, an invitation and its answer
//   node calendar-browser.mjs film   <out>  the launch film's states, in <out>/film/
//
// As in nix/tests/contacts-browser.mjs the box is reached through a SOCKS
// proxy on the LAN, so pages load at https://mattbox.local/. Pictures go to
// <out>/pr/. It exits non-zero at the first step that does not happen,
// after a picture of every open page.

const { chromium } = await import(Bun.env.CALENDAR_PLAYWRIGHT ?? "playwright");

const [phase, out] = Bun.argv.slice(2);
const HOST = Bun.env.CALENDAR_HOST ?? "mattbox.local";
const ORIGIN = `https://${HOST}`;
const PROXY = Bun.env.CALENDAR_PROXY ?? "socks5://127.0.0.1:1080";
const MATUS = Bun.env.CALENDAR_MATUS_PASSWORD;
const EVA = Bun.env.CALENDAR_EVA_PASSWORD;
const DOMAIN = "mattbox.home.arpa";
const ZONE = "Europe/Bratislava";
Bun.spawnSync(["mkdir", "-p", `${out}/pr`]);

const browser = await chromium.launch({ chromiumSandbox: false, proxy: { server: PROXY } });
const problems = [];
const pages = [];

async function person(name, password, { film = false, phone = false } = {}) {
  const context = await browser.newContext({
    viewport: phone ? { width: 390, height: 844 } : film ? { width: 1280, height: 900 } : { width: 1280, height: 860 },
    deviceScaleFactor: 2,
    colorScheme: "dark",
    locale: "en-US",
    timezoneId: ZONE,
    reducedMotion: "reduce",
    ignoreHTTPSErrors: true,
  });
  const page = await context.newPage();
  page.setDefaultTimeout(120_000);
  page.on("pageerror", (error) => problems.push(`${name}: ${error.message}`));
  page.on("console", (msg) => {
    if (msg.type() === "error" && msg.text().includes("Content Security Policy") && !msg.text().includes("inline style")) {
      problems.push(`${name}: ${msg.text()}`);
    }
  });
  const answer = await context.request.post(`${ORIGIN}/auth/api/firstfactor`, { data: { username: name, password } });
  if (!answer.ok()) throw new Error(`${name} could not sign in: ${answer.status()} ${await answer.text()}`);
  pages.push(page);
  return page;
}

async function settle(page) {
  await page.evaluate(() => document.fonts.ready);
  await page.evaluate(() => {
    document.documentElement.style.caretColor = "transparent";
  });
  await page.waitForFunction(() => !document.querySelector(".animate-spin"), null, { timeout: 30_000 }).catch(() => {});
  await page.waitForTimeout(1000);
}

async function shot(page, name) {
  await settle(page);
  await page.screenshot({ path: `${out}/pr/${name}.png`, timeout: 300_000 });
  console.log(`shot ${name}`);
}

/** `ms` as the page's wall clock in Bratislava, `2026-10-22T14:05`. */
function local(ms) {
  const parts = Object.fromEntries(
    new Intl.DateTimeFormat("en-CA", {
      timeZone: ZONE,
      year: "numeric",
      month: "2-digit",
      day: "2-digit",
      hour: "2-digit",
      minute: "2-digit",
      hourCycle: "h23",
    })
      .formatToParts(new Date(ms))
      .map((p) => [p.type, p.value]),
  );
  return `${parts.year}-${parts.month}-${parts.day}T${parts.hour}:${parts.minute}`;
}
const tomorrow = local(Date.now() + 86_400_000).slice(0, 10);

const calendarLink = (page) => page.getByRole("link", { name: "Calendar", exact: true });

/** The overview once its app tiles are in, so Before and After show the
 *  same state rather than one of them still looking for the apps. */
async function overview(page) {
  await page.goto(`${ORIGIN}/`);
  await page
    .getByText("Looking for the apps on this box.")
    .waitFor({ state: "detached", timeout: 120_000 })
    .catch(() => {});
}

/** A message in the person's inbox, open. */
async function openMail(page, subject) {
  await page.goto(`${ORIGIN}/mail`);
  const row = page.getByRole("button", { name: new RegExp(subject) }).first();
  for (let i = 0; i < 20 && (await row.count()) === 0; i++) {
    await page.waitForTimeout(3000);
    await page.reload();
  }
  await row.click();
  await page.getByRole("heading", { name: subject }).first().waitFor();
}

async function before() {
  const owner = await person("matus", MATUS);
  await overview(owner);
  await owner.getByRole("link", { name: "Contacts", exact: true }).waitFor();
  if ((await calendarLink(owner).count()) !== 0) throw new Error("the page before already has Calendar");
  await shot(owner, "sidebar-before");
  await owner.goto(`${ORIGIN}/calendar`);
  await shot(owner, "calendar-before");
  const eva = await person("eva", EVA);
  await openMail(eva, "Invitation: Review the box");
  await shot(eva, "mail-invite-before");
}

async function on() {
  const owner = await person("matus", MATUS);
  await overview(owner);
  await calendarLink(owner).waitFor();
  await shot(owner, "sidebar-after");

  // A subscription to a timetable's webcal:// address.
  await calendarLink(owner).click();
  await owner.waitForURL(`${ORIGIN}/calendar`);
  await owner.getByRole("heading", { level: 1, name: "Calendar" }).waitFor();
  await owner.getByRole("region", { name: "Calendars" }).getByText("Tax deadlines").waitFor();
  await owner.getByRole("button", { name: "Subscribe", exact: true }).first().click();
  const subscribe = owner.getByRole("dialog");
  await subscribe.locator("#calendar-subscribe-url").fill("webcal://feeds.example/timetable.ics");
  await subscribe.locator("#calendar-subscribe-name").fill("Timetable (EduPage)");
  await subscribe.locator("#calendar-subscribe-refresh").selectOption("21600");
  await shot(owner, "calendar-subscribe");
  await subscribe.getByRole("button", { name: "Subscribe", exact: true }).click();
  await subscribe.waitFor({ state: "detached" });
  await owner.getByRole("region", { name: "Calendars" }).getByText("Timetable (EduPage)").waitFor();
  await owner.getByRole("button", { name: /^Mathematics/ }).first().waitFor();
  await shot(owner, "calendar-week");

  // A new event with Eva, a reminder and a video call; the invitation goes by Mail.
  await owner.getByRole("button", { name: "New event", exact: true }).click();
  const editor = owner.getByRole("dialog");
  await editor.locator("#calendar-edit-summary").fill("Review the box");
  await editor.locator("#calendar-edit-start").fill(tomorrow);
  await editor.getByLabel("Start time").fill("14:00");
  const guests = editor.locator("#calendar-edit-guests");
  await guests.click();
  await guests.pressSequentially("ev", { delay: 80 });
  await owner.getByRole("option", { name: /Eva/ }).first().waitFor({ timeout: 30_000 }).catch(() => {});
  await shot(owner, "calendar-editor-guests");
  await owner.keyboard.press("Escape");
  if (!(await editor.isVisible())) throw new Error("Escape in the guests' picker closed the editor");
  await guests.fill(`eva@${DOMAIN}`);
  await editor.locator('label[for="calendar-edit-call"]').click();
  await editor.locator("#calendar-edit-alarm").selectOption("15");
  await editor.locator("#calendar-edit-location").fill("LosOS Chat");
  const invite = editor.getByRole("checkbox");
  await invite.waitFor();
  if (!(await invite.isChecked())) throw new Error("the invitation is not offered for a new guest");
  await shot(owner, "calendar-editor");
  await editor.getByRole("button", { name: "Save", exact: true }).click();
  await editor.waitFor({ state: "detached" });
  const review = owner.getByRole("button", { name: /Review the box/ }).first();
  await review.waitFor();
  await shot(owner, "calendar-saved");

  await review.click();
  const details = owner.getByRole("dialog");
  await details.getByRole("heading", { name: "Review the box" }).waitFor();
  await details.getByRole("button", { name: "Join the call" }).waitFor();
  await details.getByText(`eva@${DOMAIN}`).waitFor();
  await shot(owner, "calendar-event");
  await owner.keyboard.press("Escape");

  await owner.getByRole("button", { name: "Month", exact: true }).click();
  await owner.getByRole("button", { name: /Review the box/ }).first().waitFor();
  await shot(owner, "calendar-month");
  await owner.getByRole("button", { name: "List", exact: true }).click();
  await owner.getByRole("button", { name: /Review the box/ }).first().waitFor();
  await shot(owner, "calendar-list");
  await owner.getByRole("button", { name: "Week", exact: true }).click();

  // Published as a feed, with the address a calendar app takes.
  await owner.getByRole("button", { name: "Settings of Personal" }).click();
  const settings = owner.getByRole("dialog");
  await settings.locator("[data-slot=switch]").click();
  const address = settings.getByRole("textbox", { name: "Subscription address" });
  await address.waitFor();
  if (!/^webcal:\/\/mattbox\.local\/cloud\/feed\/[\w-]+\.ics$/.test(await address.inputValue())) {
    throw new Error(`the feed address is ${await address.inputValue()}`);
  }
  await shot(owner, "calendar-publish");
  await settings.locator("[data-slot=switch]").click();
  await address.waitFor({ state: "detached" });
  await owner.keyboard.press("Escape");

  // Eva answers in Mail; the event lands in her calendar.
  const eva = await person("eva", EVA);
  await openMail(eva, "Invitation: Review the box");
  const card = eva.getByRole("region", { name: "Invitation" });
  await card.getByText("Invitation from Matus").waitFor();
  await card.getByRole("button", { name: "Yes", exact: true }).waitFor();
  await shot(eva, "mail-invite-after");
  await card.getByRole("button", { name: "Yes", exact: true }).click();
  await eva.getByText("Going: Review the box is in your calendar").first().waitFor();
  await shot(eva, "mail-invite-answered");
  await card.getByRole("button", { name: "Open in Calendar" }).click();
  await eva.waitForURL(`${ORIGIN}/calendar**`);
  await eva.getByRole("dialog").getByRole("heading", { name: "Review the box" }).waitFor();
  await shot(eva, "calendar-eva");

  // Her answer reaches Matus by mail and is put on his copy.
  await openMail(owner, "Accepted: Review the box");
  const reply = owner.getByRole("region", { name: "Invitation" });
  await reply.getByText("Eva is going.").waitFor();
  await reply.getByText("In your calendar").waitFor();
  await shot(owner, "mail-reply");
  await owner.goto(`${ORIGIN}/calendar`);
  await owner.getByRole("button", { name: /Review the box/ }).first().click();
  await owner.getByRole("dialog").getByText("Going").first().waitFor();
  await shot(owner, "calendar-guest-going");
  await owner.keyboard.press("Escape");

  // A reminder, ten minutes before a call that starts in a little over ten.
  const at = Date.now() + 10 * 60_000 + 45_000;
  const made = await owner.context().request.post(`${ORIGIN}/cloud/api/calendar/events`, {
    data: {
      calendar: "personal",
      summary: "Call with Eva",
      location: "",
      description: "",
      start: local(at),
      end: local(at + 30 * 60_000),
      zone: ZONE,
      alarms: [10],
      call: "video",
      organizer: { email: `matus@${DOMAIN}`, name: "Matus" },
      attendees: [{ email: `eva@${DOMAIN}`, name: "Eva", matrix: "@eva:mattbox.local" }],
    },
  });
  if (!made.ok()) throw new Error(`the call was not saved: ${made.status()} ${await made.text()}`);
  const call = await made.json();
  // Made behind the page's back, so the page reads the week again.
  await owner.reload();
  await owner.getByRole("button", { name: /Call with Eva/ }).first().waitFor();
  await owner.getByText("Coming up: Call with Eva").first().waitFor({ timeout: 240_000 });
  await owner.getByRole("button", { name: "Join the call" }).first().waitFor();
  await shot(owner, "calendar-reminder");
  await owner.context().request.delete(`${ORIGIN}/cloud/api/calendar/events/${encodeURIComponent(call.id)}`);

  const phone = await person("matus", MATUS, { phone: true });
  await phone.goto(`${ORIGIN}/calendar`);
  await phone.getByRole("button", { name: /Review the box/ }).first().waitFor();
  const width = await phone.evaluate(() => document.documentElement.scrollWidth);
  if (width > 390) throw new Error(`the phone page scrolls sideways: ${width}px`);
  await shot(phone, "calendar-phone");
}

// ── The launch film ────────────────────────────────────────────────────────
// One 1280x900 picture at 2x per state, and the whole walk as one 60 fps
// clip at the same size, as nix/tests/contacts-browser.mjs makes them.

const steps = [];
const frames = [];
let clipStart = null;

async function box(locator) {
  const b = await locator.boundingBox();
  if (b === null) throw new Error("a film target is not on the screen");
  return { x: Math.round(b.x), y: Math.round(b.y), w: Math.round(b.width), h: Math.round(b.height) };
}

async function frame(page, title, action = { kind: "none" }, targets = {}) {
  await settle(page);
  const file = `${String(steps.length + 1).padStart(2, "0")}-${title.toLowerCase().replace(/[^a-z0-9]+/g, "-").replace(/^-|-$/g, "")}.png`;
  await page.screenshot({ path: `${out}/film/${file}`, timeout: 300_000 });
  const url = new URL(page.url());
  const shown = {};
  for (const [name, locator] of Object.entries(targets)) shown[name] = await box(locator);
  const clipAt = clipStart === null ? null : Math.round((Date.now() / 1000 - clipStart) * 1000) / 1000;
  steps.push({ file, url: `${HOST}${url.pathname}${url.search}`, title, action, targets: shown, clipAt });
  console.log(`film ${file}`);
}

async function record(page) {
  Bun.spawnSync(["mkdir", "-p", `${out}/film/frames`]);
  const cdp = await page.context().newCDPSession(page);
  const writes = [];
  cdp.on("Page.screencastFrame", ({ data, metadata, sessionId }) => {
    const name = `frames/${String(frames.length).padStart(6, "0")}.jpg`;
    // Written as they come; the clip waits for every one before ffmpeg.
    writes.push(Bun.write(`${out}/film/${name}`, Uint8Array.fromBase64(data)));
    frames.push({ name, t: metadata.timestamp });
    if (clipStart === null) clipStart = metadata.timestamp;
    cdp.send("Page.screencastFrameAck", { sessionId }).catch(() => {});
  });
  await cdp.send("Page.startScreencast", { format: "jpeg", quality: 95, maxWidth: 2560, maxHeight: 1800, everyNthFrame: 1 });
  return async (title) => {
    await page.waitForTimeout(1000);
    const clipName = `${String(steps.length + 1).padStart(2, "0")}-walkthrough.mp4`;
    await cdp.send("Page.stopScreencast");
    await Promise.all(writes);
    const lines = ["ffconcat version 1.0"];
    for (const [i, f] of frames.entries()) {
      const next = frames[i + 1]?.t ?? f.t + 1;
      lines.push(`file '${f.name}'`, `duration ${Math.max(next - f.t, 0.001).toFixed(6)}`);
    }
    lines.push(`file '${frames.at(-1).name}'`);
    await Bun.write(`${out}/film/frames.ffconcat`, lines.join("\n") + "\n");
    const ffmpeg = Bun.spawnSync(
      [Bun.env.CALENDAR_FFMPEG ?? "ffmpeg",
       "-y", "-loglevel", "error", "-f", "concat", "-safe", "0", "-i", "frames.ffconcat",
       "-vf", "fps=60,scale=2560:1800:flags=lanczos,format=yuv420p", "-r", "60",
       "-c:v", "libx264", "-preset", "slow", "-crf", "16", "-movflags", "+faststart", clipName],
      { cwd: `${out}/film` },
    );
    if (ffmpeg.exitCode !== 0) throw new Error(`ffmpeg: ${new TextDecoder().decode(ffmpeg.stderr)}`);
    console.log(`film ${clipName} from ${frames.length} frames`);
    steps.push({ file: clipName, url: `${HOST}/`, title, action: { kind: "none" }, targets: {}, fps: 60, frames: frames.length });
  };
}

async function film() {
  Bun.spawnSync(["mkdir", "-p", `${out}/film`]);
  const owner = await person("matus", MATUS, { film: true });
  await overview(owner);
  await settle(owner);
  const stop = await record(owner);
  await owner.waitForTimeout(500);
  await frame(owner, "Overview with Calendar", { kind: "click", target: "calendar" }, { calendar: calendarLink(owner) });

  await calendarLink(owner).click();
  await owner.waitForURL(`${ORIGIN}/calendar`);
  const review = owner.getByRole("button", { name: /Review the box/ }).first();
  await review.waitFor();
  await owner.getByRole("button", { name: /^Mathematics/ }).first().waitFor();
  await frame(owner, "The week", { kind: "click", target: "review" }, {
    review,
    timetable: owner.getByRole("region", { name: "Calendars" }).getByText("Timetable (EduPage)"),
    taxes: owner.getByRole("region", { name: "Calendars" }).getByText("Tax deadlines"),
  });

  await review.click();
  const details = owner.getByRole("dialog");
  await details.getByRole("heading", { name: "Review the box" }).waitFor();
  await frame(owner, "A call with Eva", { kind: "none" }, {
    join: details.getByRole("button", { name: "Join the call" }),
    guest: details.getByText(`eva@${DOMAIN}`),
  });
  await owner.keyboard.press("Escape");

  await owner.getByRole("button", { name: "Month", exact: true }).click();
  await owner.getByRole("button", { name: /Review the box/ }).first().waitFor();
  await frame(owner, "The month", { kind: "none" });
  await owner.getByRole("button", { name: "Week", exact: true }).click();

  await owner.getByRole("button", { name: "Subscribe", exact: true }).first().click();
  const subscribe = owner.getByRole("dialog");
  const field = subscribe.locator("#calendar-subscribe-url");
  await field.click();
  await field.pressSequentially("webcal://feeds.example/tax.ics", { delay: 30 });
  await frame(owner, "Subscribe to a webcal address", { kind: "type", target: "field", text: "webcal://feeds.example/tax.ics" }, { field });
  await owner.keyboard.press("Escape");

  await openMail(owner, "Accepted: Review the box");
  const reply = owner.getByRole("region", { name: "Invitation" });
  await reply.getByText("Eva is going.").waitFor();
  await frame(owner, "Eva's answer in Mail", { kind: "none" }, { card: reply });
  await stop("The walk through Calendar and Mail");

  const eva = await person("eva", EVA, { film: true });
  await openMail(eva, "Invitation: Review the box");
  const card = eva.getByRole("region", { name: "Invitation" });
  await card.getByText("Invitation from Matus").waitFor();
  await frame(eva, "Eva's invitation in Mail", { kind: "none" }, {
    card,
    yes: card.getByRole("button", { name: "Yes", exact: true }),
  });

  await Bun.write(
    `${out}/film/manifest.json`,
    JSON.stringify(
      {
        feature: "calendar",
        viewport: { width: 1280, height: 900 },
        dpr: 2,
        commit: Bun.env.CALENDAR_COMMIT ?? "",
        test: "losos-calendar",
        steps,
      },
      null,
      2,
    ) + "\n",
  );
}

// Thrown at the very end, after the pictures of the failure are saved:
// an uncaught error is what makes Bun exit non-zero.
let failed = false;
try {
  await { before, on, film }[phase]();
  if (problems.length > 0) throw new Error(`the page reported: ${problems.join("; ")}`);
} catch (error) {
  for (const [i, page] of pages.entries()) {
    await page.screenshot({ path: `${out}/pr/failed-${phase}-${i}.png`, timeout: 300_000 }).catch(() => {});
  }
  console.error(error);
  failed = true;
} finally {
  await browser.close();
}
if (failed) throw new Error(`the ${phase} phase failed, see above`);
