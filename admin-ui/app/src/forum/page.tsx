import * as React from "react";
import { Link, Route, Routes, useNavigate, useParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  Add01Icon,
  Alert01Icon,
  ArrowLeft01Icon,
  BubbleChatIcon,
  MessageMultiple01Icon,
  Notebook01Icon,
  RefreshIcon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";
import { Dialog, DialogBody, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Field, FieldDescription, FieldError, FieldGroup, FieldLabel } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Textarea } from "@/components/ui/textarea";
import { ScrollArea } from "@/components/ui/scroll-area";
import { Skeleton } from "@/components/ui/skeleton";
import { Spinner } from "@/components/ui/spinner";
import { toast } from "@/components/ui/toast";
import { ApiError } from "@/lib/api";
import { forum, type Channel, type ForumStatus, type Message, type Post, type Space, type Topic } from "./api";

function formatTime(value?: string) {
  if (!value) return "";
  const d = new Date(value);
  if (Number.isNaN(d.getTime())) return value;
  return d.toLocaleString(undefined, { dateStyle: "medium", timeStyle: "short" });
}

function slugify(input: string) {
  return input
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 64) || "item";
}

function ErrorBanner({ error, onRetry }: { error: unknown; onRetry?: () => void }) {
  const msg = error instanceof ApiError ? error.message : error instanceof Error ? error.message : "Something went wrong.";
  return (
    <Alert className="border-crit/30">
      <AlertTitle className="flex items-center gap-2"><HugeiconsIcon icon={Alert01Icon} className="size-4" /> Could not load forum</AlertTitle>
      <AlertDescription className="flex flex-wrap items-center justify-between gap-2">
        <span>{msg}</span>
        {onRetry ? <Button variant="secondary" size="sm" onClick={onRetry}><HugeiconsIcon icon={RefreshIcon} className="size-4" /> Try again</Button> : null}
      </AlertDescription>
    </Alert>
  );
}

function LoadingRows({ count = 3 }: { count?: number }) {
  return (
    <div className="space-y-2" aria-hidden="true">
      {Array.from({ length: count }).map((_, i) => (
        <div key={i} className="rounded-md border border-line p-3">
          <Skeleton className="mb-2 h-4 w-1/3" />
          <Skeleton className="h-3 w-2/3" />
        </div>
      ))}
    </div>
  );
}

function GateNotice({ status }: { status: ForumStatus }) {
  if (status.enabled !== false && status.storageReady !== false) return null;
  return (
    <Alert>
      <AlertTitle>Forum is not fully available</AlertTitle>
      <AlertDescription>
        The forum service is scaffold-gated or its storage is not ready. You can review status here; create and read actions may be unavailable until <code>losos.forum</code> is enabled and storage is ready.
      </AlertDescription>
    </Alert>
  );
}

export default function ForumPage() {
  const [status, setStatus] = React.useState<ForumStatus | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [loading, setLoading] = React.useState(true);

  const loadStatus = React.useCallback(async () => {
    setLoading(true); setError(null);
    try { setStatus(await forum.status()); }
    catch (e) { setError(e); }
    finally { setLoading(false); }
  }, []);

  React.useEffect(() => { void loadStatus(); }, [loadStatus]);

  return (
    <div className="mx-auto flex h-full min-h-0 w-full max-w-6xl flex-col gap-4 p-4 md:p-6">
      <div className="flex flex-wrap items-center gap-2">
        <HugeiconsIcon icon={BubbleChatIcon} className="size-5 text-ink" aria-hidden="true" />
        <h1 className="text-xl font-semibold text-ink">LosOS Forum</h1>
        {status ? <Badge variant={status.enabled ? "ok" : "warn"}>{status.enabled ? "Enabled" : "Gated / scaffold"}</Badge> : null}
        {status?.storageReady === false ? <Badge variant="warn">Storage not ready</Badge> : null}
        <div className="ml-auto">
          <Button variant="ghost" size="sm" onClick={() => void loadStatus()} aria-label="Refresh forum status">
            <HugeiconsIcon icon={RefreshIcon} className="size-4" /> Refresh
          </Button>
        </div>
      </div>

      {loading ? <div className="flex items-center gap-2 text-sm text-muted"><Spinner /> Loading forum status…</div> : null}
      {error ? <ErrorBanner error={error} onRetry={() => void loadStatus()} /> : null}
      {status ? <GateNotice status={status} /> : null}

      {!loading ? (
        <Routes>
          <Route index element={<SpacesView status={status} />} />
          <Route path="spaces/:spaceId" element={<SpaceView />} />
          <Route path="topics/:topicId" element={<TopicView />} />
          <Route path="channels/:channelId" element={<ChannelView />} />
          <Route path="*" element={<SpacesView status={status} />} />
        </Routes>
      ) : null}
    </div>
  );
}

function SpacesView({ status }: { status: ForumStatus | null }) {
  const [items, setItems] = React.useState<Space[] | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [open, setOpen] = React.useState(false);
  const load = React.useCallback(async () => {
    setError(null); setItems(null);
    try { setItems(await forum.listSpaces()); }
    catch (e) { setError(e); setItems([]); }
  }, []);
  React.useEffect(() => { void load(); }, [load]);

  return (
    <div className="grid gap-4 lg:grid-cols-[1fr_320px]">
      <Card>
        <CardHeader className="flex-row items-center justify-between gap-2">
          <div>
            <CardTitle>Spaces</CardTitle>
            <p className="text-sm text-muted">Discourse-style spaces hold channels and topics.</p>
          </div>
          <Button size="sm" onClick={() => setOpen(true)}><HugeiconsIcon icon={Add01Icon} className="size-4" /> New space</Button>
        </CardHeader>
        <CardContent>
          {items == null ? <LoadingRows /> : error ? <ErrorBanner error={error} onRetry={() => void load()} /> : items.length === 0 ? (
            <Empty className="border-dashed">
              <EmptyHeader>
                <EmptyMedia variant="icon"><HugeiconsIcon icon={Notebook01Icon} /></EmptyMedia>
                <EmptyTitle>No spaces yet</EmptyTitle>
                <EmptyDescription>Create a space to organize channels and topics. If creation fails, forum storage or permissions may not be ready.</EmptyDescription>
              </EmptyHeader>
            </Empty>
          ) : (
            <ul className="space-y-2">
              {items.map((s) => (
                <li key={s.id}>
                  <Link to={`/forum/spaces/${s.id}`} className="block rounded-md border border-line p-3 transition-colors hover:bg-sunk focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent">
                    <div className="flex items-start justify-between gap-2">
                      <div className="min-w-0">
                        <div className="font-medium text-ink">{s.name}</div>
                        <div className="text-sm text-muted">{s.description || `/${s.slug}`}</div>
                      </div>
                      <Badge variant="neutral">Space</Badge>
                    </div>
                  </Link>
                </li>
              ))}
            </ul>
          )}
        </CardContent>
      </Card>

      <Card>
        <CardHeader><CardTitle>Status</CardTitle><p className="text-sm text-muted">Phase 1 foundation wiring.</p></CardHeader>
        <CardContent className="space-y-2 text-sm">
          <FeatureRow label="API phase" detail={status ? `Phase ${status.phase ?? 0}${status.implemented ? ", implemented" : ""}` : "Unknown"} ok={!!status?.implemented} />
          <FeatureRow label="Edge-local routing" detail={status?.edgeLocalRouting ? "On" : "Off / unknown"} ok={!!status?.edgeLocalRouting} />
          <FeatureRow label="ActivityPub" detail={status?.activityPub ? "Enabled (stub contracts)" : "Disabled"} ok={false} />
          <FeatureRow label="Matrix DM bridge" detail={status?.matrixDmBridge ? "Enabled (stub contracts)" : "Disabled"} ok={false} />
          <FeatureRow label="@LosOS assistant" detail={status?.assistant ? "Enabled hook" : "Disabled"} ok={false} />
        </CardContent>
      </Card>

      <CreateSpaceDialog open={open} onOpenChange={setOpen} onCreated={(s) => { setOpen(false); toast.success("Space created", s.name); void load(); }} />
    </div>
  );
}

function FeatureRow({ label, detail, ok }: { label: string; detail: string; ok: boolean }) {
  return (
    <div className="flex items-start justify-between gap-3 rounded-md border border-line p-2">
      <div><div className="font-medium text-ink">{label}</div><div className="text-muted">{detail}</div></div>
      <Badge variant={ok ? "ok" : "neutral"}>{ok ? "Active" : "Planned"}</Badge>
    </div>
  );
}

function CreateSpaceDialog({ open, onOpenChange, onCreated }: { open: boolean; onOpenChange: (v: boolean) => void; onCreated: (s: Space) => void }) {
  const [name, setName] = React.useState(""); const [slug, setSlug] = React.useState(""); const [description, setDescription] = React.useState("");
  const [busy, setBusy] = React.useState(false); const [error, setError] = React.useState<string | null>(null);
  const id = React.useId();
  React.useEffect(() => { if (!slug && name) setSlug(slugify(name)); }, [name, slug]);
  const submit = async (e: React.FormEvent) => {
    e.preventDefault(); setError(null); setBusy(true);
    try { onCreated(await forum.createSpace({ name: name.trim(), slug: slug.trim() || slugify(name), description: description.trim() })); setName(""); setSlug(""); setDescription(""); }
    catch (e) { setError(e instanceof ApiError ? e.message : "Could not create space."); }
    finally { setBusy(false); }
  };
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="create-space-title">
      <DialogHeader><DialogTitle id="create-space-title">Create space</DialogTitle><DialogDescription>Spaces group channels and topics. You can change content later; choose a clear name.</DialogDescription></DialogHeader>
      <form onSubmit={submit}>
        <DialogBody><FieldGroup>
          <Field><FieldLabel htmlFor={`${id}-name`}>Name</FieldLabel><Input id={`${id}-name`} value={name} onChange={(e) => setName(e.target.value)} required aria-invalid={!!error} /><FieldDescription>A short, recognizable name, such as “General”.</FieldDescription></Field>
          <Field><FieldLabel htmlFor={`${id}-slug`}>Slug</FieldLabel><Input id={`${id}-slug`} value={slug} onChange={(e) => setSlug(e.target.value)} required /><FieldDescription>Used in identifiers/URLs; lowercase letters, numbers, and hyphens work best.</FieldDescription></Field>
          <Field><FieldLabel htmlFor={`${id}-desc`}>Description</FieldLabel><Textarea id={`${id}-desc`} value={description} onChange={(e) => setDescription(e.target.value)} rows={3} /><FieldDescription>Optional. Explain what belongs in this space.</FieldDescription></Field>
          {error ? <FieldError>{error}</FieldError> : null}
        </FieldGroup></DialogBody>
        <DialogFooter>
          <Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>Cancel</Button>
          <Button type="submit" disabled={busy || !name.trim() || !slug.trim()}>{busy ? <><Spinner /> Creating…</> : "Create space"}</Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

function SpaceView() {
  const { spaceId } = useParams();
  const sid = Number(spaceId);
  const [channels, setChannels] = React.useState<Channel[] | null>(null);
  const [topics, setTopics] = React.useState<Topic[] | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [chanOpen, setChanOpen] = React.useState(false);
  const [topicOpen, setTopicOpen] = React.useState(false);
  const load = React.useCallback(async () => {
    if (!Number.isFinite(sid)) return;
    setError(null); setChannels(null); setTopics(null);
    try {
      const [c, t] = await Promise.all([forum.listChannels(sid), forum.listTopics()]);
      setChannels(c);
      // Phase 1 backend lists topics globally/by channel; filter to channels in this space.
      const cids = new Set(c.map((x) => x.id));
      setTopics(t.filter((x) => cids.has(x.channelId)));
    } catch (e) { setError(e); setChannels([]); setTopics([]); }
  }, [sid]);
  React.useEffect(() => { void load(); }, [load]);
  if (!Number.isFinite(sid)) return <Alert><AlertTitle>Invalid space</AlertTitle><AlertDescription>The space address is not valid.</AlertDescription></Alert>;

  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Link to="/forum" className="inline-flex h-8 items-center gap-2 rounded-control px-3 text-[13px] font-medium text-muted hover:bg-sunk hover:text-ink"><HugeiconsIcon icon={ArrowLeft01Icon} className="size-4" /> Spaces</Link>
        <h2 className="text-lg font-semibold text-ink">Space {sid}</h2>
      </div>
      {error ? <ErrorBanner error={error} onRetry={() => void load()} /> : null}
      <div className="grid gap-4 lg:grid-cols-2">
        <Card>
          <CardHeader className="flex-row items-center justify-between gap-2">
            <div><CardTitle>Channels</CardTitle><p className="text-sm text-muted">Slack-style timelines and topic containers.</p></div>
            <Button size="sm" variant="secondary" onClick={() => setChanOpen(true)}><HugeiconsIcon icon={Add01Icon} className="size-4" /> New channel</Button>
          </CardHeader>
          <CardContent>
            {channels == null ? <LoadingRows /> : channels.length === 0 ? <p className="text-sm text-muted">No channels in this space yet.</p> : (
              <ul className="space-y-2">{channels.map((c) => (
                <li key={c.id}>
                  <Link to={`/forum/channels/${c.id}`} className="block rounded-md border border-line p-3 hover:bg-sunk focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent">
                    <div className="flex items-center justify-between gap-2"><span className="font-medium text-ink">{c.name}</span><Badge variant="neutral">{c.kind}</Badge></div>
                    <div className="text-sm text-muted">/{c.slug}</div>
                  </Link>
                </li>
              ))}</ul>
            )}
          </CardContent>
        </Card>
        <Card>
          <CardHeader className="flex-row items-center justify-between gap-2">
            <div><CardTitle>Topics</CardTitle><p className="text-sm text-muted">Discourse-style discussions in this space’s channels.</p></div>
            <Button size="sm" variant="secondary" onClick={() => setTopicOpen(true)} disabled={!channels?.some((c) => c.kind === "topic" || c.kind === "channel")}><HugeiconsIcon icon={Add01Icon} className="size-4" /> New topic</Button>
          </CardHeader>
          <CardContent>
            {topics == null ? <LoadingRows /> : topics.length === 0 ? <p className="text-sm text-muted">No topics yet. Create a channel, then start a topic.</p> : (
              <ul className="space-y-2">{topics.map((t) => (
                <li key={t.id}>
                  <Link to={`/forum/topics/${t.id}`} className="block rounded-md border border-line p-3 hover:bg-sunk focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-accent">
                    <div className="font-medium text-ink">{t.title}</div>
                    <div className="text-sm text-muted">By {t.createdBy} · {formatTime(t.updatedAt || t.createdAt)}{t.closed ? " · Closed" : ""}</div>
                  </Link>
                </li>
              ))}</ul>
            )}
          </CardContent>
        </Card>
      </div>
      <CreateChannelDialog open={chanOpen} onOpenChange={setChanOpen} spaceId={sid} onCreated={() => { setChanOpen(false); toast.success("Channel created"); void load(); }} />
      <CreateTopicDialog open={topicOpen} onOpenChange={setTopicOpen} channels={(channels ?? []).filter((c) => c.kind === "topic" || c.kind === "channel")} onCreated={(r) => { setTopicOpen(false); toast.success("Topic created", r.mentions.some((m) => m.isAssistant) ? "@LosOS was mentioned (assistant handling is Phase 2)." : undefined); void load(); }} />
    </div>
  );
}

function CreateChannelDialog({ open, onOpenChange, spaceId, onCreated }: { open: boolean; onOpenChange: (v: boolean) => void; spaceId: number; onCreated: () => void }) {
  const [name, setName] = React.useState(""); const [slug, setSlug] = React.useState(""); const [kind, setKind] = React.useState<Channel["kind"]>("topic");
  const [busy, setBusy] = React.useState(false); const [error, setError] = React.useState<string | null>(null);
  const id = React.useId();
  React.useEffect(() => { if (!slug && name) setSlug(slugify(name)); }, [name, slug]);
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="create-channel-title">
      <DialogHeader><DialogTitle id="create-channel-title">Create channel</DialogTitle><DialogDescription>Channels can hold topic discussions or real-time messages.</DialogDescription></DialogHeader>
      <form onSubmit={async (e) => { e.preventDefault(); setError(null); setBusy(true); try { await forum.createChannel({ spaceId, name: name.trim(), slug: slug.trim() || slugify(name), kind }); setName(""); setSlug(""); onCreated(); } catch (e) { setError(e instanceof ApiError ? e.message : "Could not create channel."); } finally { setBusy(false); } }}>
        <DialogBody><FieldGroup>
          <Field><FieldLabel htmlFor={`${id}-n`}>Name</FieldLabel><Input id={`${id}-n`} value={name} onChange={(e) => setName(e.target.value)} required /></Field>
          <Field><FieldLabel htmlFor={`${id}-s`}>Slug</FieldLabel><Input id={`${id}-s`} value={slug} onChange={(e) => setSlug(e.target.value)} required /></Field>
          <Field><FieldLabel htmlFor={`${id}-k`}>Kind</FieldLabel>
            <select id={`${id}-k`} className="h-9 rounded-control border border-line bg-surface px-3 text-sm text-ink" value={kind} onChange={(e) => setKind(e.target.value as Channel["kind"])}>
              <option value="topic">Topic channel</option><option value="channel">Message channel</option><option value="dm">Direct message (reserved)</option>
            </select>
            <FieldDescription>Use “Topic channel” for Discourse-style threads, “Message channel” for Slack-style messages.</FieldDescription>
          </Field>
          {error ? <FieldError>{error}</FieldError> : null}
        </FieldGroup></DialogBody>
        <DialogFooter><Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>Cancel</Button><Button type="submit" disabled={busy || !name.trim()}>{busy ? <><Spinner /> Creating…</> : "Create channel"}</Button></DialogFooter>
      </form>
    </Dialog>
  );
}

function CreateTopicDialog({ open, onOpenChange, channels, onCreated }: { open: boolean; onOpenChange: (v: boolean) => void; channels: Channel[]; onCreated: (r: { mentions: { isAssistant: boolean }[] }) => void }) {
  const [channelId, setChannelId] = React.useState<number | "">(""); const [title, setTitle] = React.useState(""); const [body, setBody] = React.useState("");
  const [busy, setBusy] = React.useState(false); const [error, setError] = React.useState<string | null>(null);
  const id = React.useId();
  React.useEffect(() => { if (channelId === "" && channels[0]) setChannelId(channels[0].id); }, [channels, channelId]);
  return (
    <Dialog open={open} onOpenChange={onOpenChange} labelledBy="create-topic-title">
      <DialogHeader><DialogTitle id="create-topic-title">Start a topic</DialogTitle><DialogDescription>Start a discussion with a clear title and first post. You can mention <code>@LosOS</code>; assistant handling arrives in a later phase.</DialogDescription></DialogHeader>
      <form onSubmit={async (e) => { e.preventDefault(); if (channelId === "") return; setError(null); setBusy(true); try { const r = await forum.createTopic({ channelId: Number(channelId), title: title.trim(), body: body.trim() }); setTitle(""); setBody(""); onCreated(r); } catch (e) { setError(e instanceof ApiError ? e.message : "Could not create topic."); } finally { setBusy(false); } }}>
        <DialogBody><FieldGroup>
          <Field><FieldLabel htmlFor={`${id}-c`}>Channel</FieldLabel>
            <select id={`${id}-c`} className="h-9 rounded-control border border-line bg-surface px-3 text-sm text-ink" value={channelId} onChange={(e) => setChannelId(e.target.value ? Number(e.target.value) : "")} required>
              <option value="">Select a channel</option>
              {channels.map((c) => <option key={c.id} value={c.id}>{c.name}</option>)}
            </select>
          </Field>
          <Field><FieldLabel htmlFor={`${id}-t`}>Title</FieldLabel><Input id={`${id}-t`} value={title} onChange={(e) => setTitle(e.target.value)} required /><FieldDescription>Say what the topic is about in plain language.</FieldDescription></Field>
          <Field><FieldLabel htmlFor={`${id}-b`}>First post</FieldLabel><Textarea id={`${id}-b`} value={body} onChange={(e) => setBody(e.target.value)} rows={5} required /><FieldDescription>Markdown/plain text for Phase 1. Be specific and respectful.</FieldDescription></Field>
          {error ? <FieldError>{error}</FieldError> : null}
        </FieldGroup></DialogBody>
        <DialogFooter><Button type="button" variant="secondary" onClick={() => onOpenChange(false)}>Cancel</Button><Button type="submit" disabled={busy || channelId === "" || !title.trim() || !body.trim()}>{busy ? <><Spinner /> Posting…</> : "Create topic"}</Button></DialogFooter>
      </form>
    </Dialog>
  );
}

function TopicView() {
  const { topicId } = useParams();
  const tid = Number(topicId);
  const navigate = useNavigate();
  const [posts, setPosts] = React.useState<Post[] | null>(null);
  const [topics, setTopics] = React.useState<Topic[] | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [body, setBody] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const load = React.useCallback(async () => {
    if (!Number.isFinite(tid)) return;
    setError(null);
    try { const [p, t] = await Promise.all([forum.listPosts(tid), forum.listTopics()]); setPosts(p); setTopics(t); }
    catch (e) { setError(e); setPosts([]); }
  }, [tid]);
  React.useEffect(() => { void load(); }, [load]);
  const topic = topics?.find((t) => t.id === tid);
  if (!Number.isFinite(tid)) return <Alert><AlertTitle>Invalid topic</AlertTitle><AlertDescription>The topic address is not valid.</AlertDescription></Alert>;
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={() => navigate(-1)}><HugeiconsIcon icon={ArrowLeft01Icon} className="size-4" /> Back</Button>
        <h2 className="min-w-0 truncate text-lg font-semibold text-ink">{topic?.title ?? `Topic ${tid}`}</h2>
        {topic?.closed ? <Badge variant="warn">Closed</Badge> : null}
      </div>
      {error ? <ErrorBanner error={error} onRetry={() => void load()} /> : null}
      <Card>
        <CardHeader><CardTitle>Posts</CardTitle></CardHeader>
        <CardContent>
          {posts == null ? <LoadingRows /> : posts.length === 0 ? <p className="text-sm text-muted">No posts yet.</p> : (
            <ol className="space-y-3">{posts.map((p) => (
              <li key={p.id} className="rounded-md border border-line p-3">
                <div className="mb-1 flex flex-wrap items-center gap-2 text-sm"><span className="font-medium text-ink">{p.author}</span><span className="text-muted">{formatTime(p.createdAt)}</span></div>
                <pre className="whitespace-pre-wrap font-sans text-sm text-ink">{p.body}</pre>
              </li>
            ))}</ol>
          )}
        </CardContent>
      </Card>
      <Card>
        <CardHeader><CardTitle>Reply</CardTitle><p className="text-sm text-muted">Add a post to this topic. Use <code>@LosOS</code> for the assistant hook (Phase 2).</p></CardHeader>
        <CardContent>
          <form onSubmit={async (e) => { e.preventDefault(); if (!body.trim()) return; setBusy(true); try { await forum.createPost(tid, { body: body.trim() }); setBody(""); toast.success("Reply posted"); void load(); } catch (e) { toast.error("Could not post reply", e instanceof ApiError ? e.message : "Try again.", { help: "index" }); } finally { setBusy(false); } }} className="space-y-3">
            <Field><FieldLabel htmlFor="topic-reply">Your reply</FieldLabel><Textarea id="topic-reply" value={body} onChange={(e) => setBody(e.target.value)} rows={4} required disabled={topic?.closed} /><FieldDescription>{topic?.closed ? "This topic is closed." : "Be clear and constructive."}</FieldDescription></Field>
            <Button type="submit" disabled={busy || !body.trim() || !!topic?.closed}>{busy ? <><Spinner /> Posting…</> : "Post reply"}</Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}

function ChannelView() {
  const { channelId } = useParams();
  const cid = Number(channelId);
  const navigate = useNavigate();
  const [messages, setMessages] = React.useState<Message[] | null>(null);
  const [channels, setChannels] = React.useState<Channel[] | null>(null);
  const [error, setError] = React.useState<unknown>(null);
  const [body, setBody] = React.useState("");
  const [busy, setBusy] = React.useState(false);
  const load = React.useCallback(async () => {
    if (!Number.isFinite(cid)) return;
    setError(null);
    try { const [m, c] = await Promise.all([forum.listMessages(cid), forum.listChannels()]); setMessages(m); setChannels(c); }
    catch (e) { setError(e); setMessages([]); }
  }, [cid]);
  React.useEffect(() => { void load(); }, [load]);
  const channel = channels?.find((c) => c.id === cid);
  if (!Number.isFinite(cid)) return <Alert><AlertTitle>Invalid channel</AlertTitle><AlertDescription>The channel address is not valid.</AlertDescription></Alert>;
  return (
    <div className="space-y-4">
      <div className="flex items-center gap-2">
        <Button variant="ghost" size="sm" onClick={() => navigate(-1)}><HugeiconsIcon icon={ArrowLeft01Icon} className="size-4" /> Back</Button>
        <HugeiconsIcon icon={MessageMultiple01Icon} className="size-4 text-muted" aria-hidden="true" />
        <h2 className="min-w-0 truncate text-lg font-semibold text-ink">{channel?.name ?? `Channel ${cid}`}</h2>
        {channel ? <Badge variant="neutral">{channel.kind}</Badge> : null}
      </div>
      {error ? <ErrorBanner error={error} onRetry={() => void load()} /> : null}
      <Card className="min-h-[320px]">
        <CardHeader className="flex-row items-center justify-between gap-2"><div><CardTitle>Messages</CardTitle><p className="text-sm text-muted">Channel timeline. Real-time updates arrive in a later phase.</p></div><Button variant="ghost" size="sm" onClick={() => void load()} aria-label="Refresh messages"><HugeiconsIcon icon={RefreshIcon} className="size-4" /></Button></CardHeader>
        <CardContent>
          {messages == null ? <LoadingRows /> : messages.length === 0 ? <p className="text-sm text-muted">No messages yet. Start the conversation below.</p> : (
            <ScrollArea className="max-h-[420px] pr-2"><ol className="space-y-3">{messages.map((m) => (
              <li key={m.id} className="rounded-md border border-line p-3">
                <div className="mb-1 flex flex-wrap items-center gap-2 text-sm"><span className="font-medium text-ink">{m.author}</span><span className="text-muted">{formatTime(m.createdAt)}</span></div>
                <pre className="whitespace-pre-wrap font-sans text-sm text-ink">{m.body}</pre>
              </li>
            ))}</ol></ScrollArea>
          )}
        </CardContent>
      </Card>
      <Card>
        <CardHeader><CardTitle>Send message</CardTitle></CardHeader>
        <CardContent>
          <form onSubmit={async (e) => { e.preventDefault(); if (!body.trim()) return; setBusy(true); try { await forum.createMessage(cid, { body: body.trim() }); setBody(""); void load(); } catch (e) { toast.error("Could not send message", e instanceof ApiError ? e.message : "Try again.", { help: "index" }); } finally { setBusy(false); } }} className="space-y-3">
            <Field><FieldLabel htmlFor="chan-msg">Message</FieldLabel><Textarea id="chan-msg" value={body} onChange={(e) => setBody(e.target.value)} rows={3} required placeholder="Write a message…" /><FieldDescription>Press “Send message” to post. Use <code>@LosOS</code> to mark an assistant mention hook.</FieldDescription></Field>
            <Button type="submit" disabled={busy || !body.trim()}>{busy ? <><Spinner /> Sending…</> : "Send message"}</Button>
          </form>
        </CardContent>
      </Card>
    </div>
  );
}
