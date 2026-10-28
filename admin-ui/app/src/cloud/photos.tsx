import * as React from "react";
import { useSearchParams } from "react-router-dom";
import { HugeiconsIcon } from "@hugeicons/react";
import {
  ArrowLeft01Icon,
  ArrowRight01Icon,
  Cancel01Icon,
  Download01Icon,
  Image01Icon,
  InformationCircleIcon,
} from "@hugeicons/core-free-icons";
import { Alert, AlertDescription } from "@/components/ui/alert";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from "@/components/ui/empty";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { Spinner } from "@/components/ui/progress";
import { useT } from "@/lib/i18n-react";
import { cn } from "@/lib/utils";
import {
  cloud,
  contentUrl,
  faceUrl,
  isAbort,
  parentOf,
  previewUrl,
  type Person,
  type Photo,
  type PhotoDetails,
  type PhotoField,
  type PhotoIndex,
} from "./api";
import { explain } from "./dialogs";
import { formatBytes, formatFull, formatMonth } from "./format";
import { useRevision } from "./store";

type By = "date" | "person" | "camera" | "place";
const BY: readonly By[] = ["date", "person", "camera", "place"];

interface Data {
  photos: Photo[];
  people: Person[];
  index: PhotoIndex;
}

/** When a photo was taken, by the camera's clock, else when its file last
 * changed. Seconds, for sorting and the month headings. */
function when(p: Photo): number {
  if (p.taken !== undefined) {
    // No zone on purpose: the camera's clock is read as local time.
    const ms = new Date(p.taken).getTime();
    if (Number.isFinite(ms)) return ms / 1000;
  }
  return p.mtime;
}

function coordinate(value: number, positive: string, negative: string, digits: number): string {
  return `${Math.abs(value).toFixed(digits)}° ${value >= 0 ? positive : negative}`;
}

function place(lat: number, lon: number, digits = 4): string {
  return `${coordinate(lat, "N", "S", digits)}, ${coordinate(lon, "E", "W", digits)}`;
}

interface Section {
  key: string;
  label: string;
  face?: number;
  items: { photo: Photo; index: number }[];
}

/** Every picture in the cloud, grouped by when it was taken, who is in it,
 * the camera or where; and the people the box has told apart. */
export function PhotosView() {
  const t = useT();
  const revision = useRevision();
  const [search, setSearch] = useSearchParams();
  const [data, setData] = React.useState<Data | null>(null);
  const [error, setError] = React.useState<string | null>(null);
  const [open, setOpen] = React.useState<number | null>(null);
  const [tick, setTick] = React.useState(0);

  const by: By = BY.find((b) => b === search.get("by")) ?? "date";
  const personParam = Number(search.get("person"));
  const personId = Number.isInteger(personParam) && personParam > 0 ? personParam : null;

  React.useEffect(() => {
    const ctl = new AbortController();
    cloud.photos(ctl.signal).then(
      (r) => {
        setData(r);
        setError(null);
      },
      (err: unknown) => {
        if (!isAbort(err)) setError(explain(err));
      },
    );
    return () => ctl.abort();
  }, [revision, tick]);

  // While the box is still reading pictures, look again now and then so
  // the dates, cameras and people fill in.
  const reading = data !== null && data.index.done < data.index.total;
  React.useEffect(() => {
    if (!reading) return;
    const timer = window.setTimeout(() => setTick((n) => n + 1), 10_000);
    return () => window.clearTimeout(timer);
  }, [reading, data]);

  const set = (key: string, value: string | null) => {
    const next = new URLSearchParams(search);
    if (value === null) next.delete(key);
    else next.set(key, value);
    setSearch(next, { replace: key === "by" });
  };

  const people = React.useMemo(() => new Map((data?.people ?? []).map((p) => [p.id, p])), [data]);
  const person = personId === null ? undefined : people.get(personId);

  const photos = React.useMemo(() => {
    const all = (data?.photos ?? []).filter((p) => personId === null || p.people.includes(personId));
    return all.sort((a, b) => when(b) - when(a));
  }, [data, personId]);

  const sections = React.useMemo(
    () => group(photos, by, people, t),
    [photos, by, people, t],
  );

  // The people worth a place in the strip: named, or in more than one photo.
  const strip = (data?.people ?? []).filter((p) => p.name !== "" || p.photos > 1);

  const header = (
    <div className="flex flex-wrap items-center gap-3">
      {person !== undefined || personId !== null ? (
        <Button variant="ghost" size="sm" className="-ml-2" onClick={() => set("person", null)}>
          <HugeiconsIcon icon={ArrowLeft01Icon} size={16} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
          {t("cloud.photos.allPhotos")}
        </Button>
      ) : (
        <h1 className="flex min-h-9 items-center text-[19px] leading-tight font-semibold tracking-tight">{t("cloud.nav.photos")}</h1>
      )}
      {data !== null && data.photos.length > 0 && (
        <label className="ml-auto flex items-center gap-2 text-[13px] text-muted">
          {t("cloud.photos.groupBy")}
          <NativeSelect
            value={by}
            onChange={(event) => set("by", event.target.value === "date" ? null : event.target.value)}
            className="[&_select]:h-8 [&_select]:text-[13px]"
            data-photos-by
          >
            {BY.filter((b) => b !== "person" || data.index.faces).map((b) => (
              <NativeSelectOption key={b} value={b}>
                {t(`cloud.photos.by.${b}`)}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </label>
      )}
    </div>
  );

  return (
    <div className="flex flex-col gap-4">
      {header}
      {error !== null ? (
        <Alert variant="crit">
          <AlertDescription>{error}</AlertDescription>
        </Alert>
      ) : data === null ? (
        <div className="grid min-h-60 place-items-center">
          <Spinner />
        </div>
      ) : data.photos.length === 0 ? (
        <Empty className="min-h-72">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={Image01Icon} size={22} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("cloud.photos.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("cloud.photos.emptyBody")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      ) : (
        <>
          {person !== undefined ? (
            <PersonHeader
              key={person.id}
              person={person}
              onNamed={(id) => {
                if (id !== person.id) set("person", String(id));
                setTick((n) => n + 1);
              }}
            />
          ) : (
            strip.length > 0 && (
              <section aria-labelledby="cloud-photos-people" className="flex flex-col gap-2">
                <h2 id="cloud-photos-people" className="text-[13px] font-medium text-muted">
                  {t("cloud.photos.people")}
                </h2>
                <ul className="-mx-1 flex gap-1 overflow-x-auto px-1 pb-1" data-photos-people>
                  {strip.map((p) => (
                    <li key={p.id}>
                      <button
                        type="button"
                        onClick={() => set("person", String(p.id))}
                        className="group flex w-[5.5rem] flex-col items-center gap-1.5 rounded-[10px] p-1.5 transition-colors hover:bg-sunk focus-visible:ring-2 focus-visible:ring-accent focus-visible:outline-none"
                        data-person={p.id}
                      >
                        <img
                          src={faceUrl(p.face)}
                          alt=""
                          className="size-16 rounded-full bg-sunk object-cover ring-1 ring-hair transition-transform duration-200 group-hover:scale-[1.04]"
                        />
                        <span className={cn("w-full truncate text-center text-[12.5px]", p.name === "" && "text-faint")}>
                          {p.name === "" ? t("cloud.photos.addName") : p.name}
                        </span>
                      </button>
                    </li>
                  ))}
                </ul>
              </section>
            )
          )}
          {reading && (
            <p className="flex items-center gap-2 text-[12.5px] text-muted" role="status" data-photos-reading>
              <Spinner className="size-3.5" />
              {t("cloud.photos.reading", { done: data.index.done, total: data.index.total })}
            </p>
          )}
          {sections.map((section) => (
            <section key={section.key} aria-label={section.label} className="flex flex-col gap-2" data-section={section.key}>
              <h2 className="flex items-center gap-2 text-[13px] font-medium text-muted">
                {section.face !== undefined && <img src={faceUrl(section.face)} alt="" className="size-6 rounded-full bg-sunk object-cover" />}
                <span className={cn(section.face !== undefined && "text-ink")}>{section.label}</span>
                <span className="text-faint">{t("cloud.photos.count", { count: section.items.length })}</span>
              </h2>
              <ul className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-1.5" data-photos>
                {section.items.map(({ photo, index }) => (
                  <li key={`${section.key}-${photo.id}`}>
                    <button
                      type="button"
                      onClick={() => setOpen(index)}
                      className="block aspect-square w-full overflow-hidden rounded-[8px] bg-sunk focus-visible:ring-2 focus-visible:ring-accent focus-visible:outline-none"
                      aria-label={photo.name}
                    >
                      <img src={previewUrl(photo, 256)} alt="" loading="lazy" className="size-full object-cover transition-transform duration-200 hover:scale-[1.03]" />
                    </button>
                  </li>
                ))}
              </ul>
            </section>
          ))}
          {data.index.faces && <p className="text-[12px] text-faint">{t("cloud.photos.private")}</p>}
        </>
      )}
      <Viewer photos={photos} people={people} index={open} onIndexChange={setOpen} />
    </div>
  );
}

/** The sections of the grid. `index` is the photo's place in `photos`, which
 * the viewer steps through. */
function group(photos: readonly Photo[], by: By, people: Map<number, Person>, t: ReturnType<typeof useT>): Section[] {
  const out = new Map<string, Section>();
  const add = (key: string, label: string, photo: Photo, index: number, face?: number) => {
    let s = out.get(key);
    if (s === undefined) {
      s = { key, label, items: [] };
      if (face !== undefined) s.face = face;
      out.set(key, s);
    }
    s.items.push({ photo, index });
  };
  photos.forEach((photo, index) => {
    switch (by) {
      case "date": {
        const label = formatMonth(when(photo));
        add(label, label, photo, index);
        break;
      }
      case "person": {
        // Strangers seen once are nobody in particular.
        const known = photo.people
          .map((id) => people.get(id))
          .filter((p): p is Person => p !== undefined && (p.name !== "" || p.photos > 1));
        if (known.length === 0) add("nobody", t("cloud.photos.noOne"), photo, index);
        for (const p of known) add(`p${p.id}`, p.name === "" ? t("cloud.photos.unnamed") : p.name, photo, index, p.face);
        break;
      }
      case "camera":
        add(photo.camera ?? "", photo.camera ?? t("cloud.photos.noCamera"), photo, index);
        break;
      case "place":
        if (photo.lat !== undefined && photo.lon !== undefined) {
          // About eleven kilometres a step: one town, one section.
          const lat = Math.round(photo.lat * 10) / 10;
          const lon = Math.round(photo.lon * 10) / 10;
          add(`${lat},${lon}`, place(lat, lon, 1), photo, index);
        } else add("", t("cloud.photos.noPlace"), photo, index);
        break;
    }
  });
  const sections = [...out.values()];
  if (by === "date") return sections;
  // The biggest groups first; named people before strangers; the photos
  // with nothing to group by last.
  const rank = (s: Section) => {
    if (s.key === "" || s.key === "nobody") return 2;
    if (by === "person" && people.get(Number(s.key.slice(1)))?.name === "") return 1;
    return 0;
  };
  return sections.sort((a, b) => rank(a) - rank(b) || b.items.length - a.items.length || a.label.localeCompare(b.label));
}

/** A person's face and name, which the owner can change. */
function PersonHeader({ person, onNamed }: { person: Person; onNamed: (id: number) => void }) {
  const t = useT();
  const [name, setName] = React.useState(person.name);
  const [busy, setBusy] = React.useState(false);
  const [error, setError] = React.useState<string | null>(null);

  const save = () => {
    const next = name.trim();
    if (next === person.name || busy) return;
    setBusy(true);
    cloud.namePerson(person.id, next).then(
      (r) => {
        setBusy(false);
        setError(null);
        onNamed(r.id);
      },
      (err: unknown) => {
        setBusy(false);
        setError(explain(err));
      },
    );
  };

  return (
    <div className="flex items-center gap-4">
      <img src={faceUrl(person.face)} alt="" className="size-20 shrink-0 rounded-full bg-sunk object-cover ring-1 ring-hair" />
      <div className="flex min-w-0 flex-1 flex-col gap-1">
        <label htmlFor="cloud-person-name" className="sr-only">
          {t("cloud.photos.name")}
        </label>
        <Input
          id="cloud-person-name"
          value={name}
          maxLength={80}
          placeholder={t("cloud.photos.namePlaceholder")}
          onChange={(event) => setName(event.target.value)}
          onBlur={save}
          onKeyDown={(event) => {
            if (event.key === "Enter") {
              event.preventDefault();
              save();
            }
          }}
          className="h-10 max-w-sm text-[17px] font-semibold"
          data-person-name
        />
        <p className="text-[12.5px] text-muted">
          {t("cloud.photos.count", { count: person.photos })} · {t("cloud.photos.nameHint")}
        </p>
        {error !== null && <p className="text-[12.5px] text-crit">{error}</p>}
      </div>
    </div>
  );
}

function Viewer({
  photos,
  people,
  index,
  onIndexChange,
}: {
  photos: readonly Photo[];
  people: Map<number, Person>;
  index: number | null;
  onIndexChange: (i: number | null) => void;
}) {
  const t = useT();
  const photo = index === null ? undefined : photos[index];
  const [info, setInfo] = React.useState(false);
  const [details, setDetails] = React.useState<PhotoDetails | null>(null);
  const go = React.useCallback(
    (step: number) => {
      if (index === null) return;
      const next = index + step;
      if (next >= 0 && next < photos.length) onIndexChange(next);
    },
    [index, photos.length, onIndexChange],
  );

  React.useEffect(() => {
    if (index === null) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.target instanceof HTMLInputElement) return;
      if (event.key === "ArrowLeft") go(-1);
      if (event.key === "ArrowRight") go(1);
      if (event.key === "i") setInfo((v) => !v);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [index, go]);

  const id = photo?.id;
  const etag = photo?.etag;
  React.useEffect(() => {
    setDetails(null);
    if (id === undefined || !info) return;
    const ctl = new AbortController();
    cloud.photo(id, ctl.signal).then(setDetails, () => undefined);
    return () => ctl.abort();
  }, [id, etag, info]);

  const icon = (glyph: typeof Cancel01Icon) => <HugeiconsIcon icon={glyph} size={18} strokeWidth={1.5} color="currentColor" aria-hidden="true" />;

  return (
    <Dialog
      open={photo !== undefined}
      onOpenChange={(o) => !o && onIndexChange(null)}
      labelledBy="cloud-viewer-title"
      dialogClassName={cn("max-w-[calc(100vw-2rem)]", info ? "w-[min(80rem,calc(100vw-2rem))]" : "w-[min(64rem,calc(100vw-2rem))]")}
      className="bg-ground"
    >
      {photo !== undefined && (
        <div className="flex flex-col">
          <div className="flex items-center gap-2 border-b border-hair px-4 py-2.5">
            <div className="min-w-0 flex-1">
              <h2 id="cloud-viewer-title" className="truncate text-sm font-medium">
                {photo.name}
              </h2>
              <p className="truncate text-[12px] text-muted">
                {parentOf(photo.path) || t("cloud.nav.files")} · {formatFull(when(photo))}
              </p>
            </div>
            <Button
              size="icon-sm"
              variant={info ? "secondary" : "ghost"}
              aria-label={t("cloud.photos.details")}
              aria-pressed={info}
              title={`${t("cloud.photos.details")} (i)`}
              onClick={() => setInfo((v) => !v)}
              data-photo-info
            >
              {icon(InformationCircleIcon)}
            </Button>
            <Button size="icon-sm" variant="ghost" aria-label={t("cloud.download")} title={t("cloud.download")} onClick={() => (window.location.href = contentUrl(photo.path, true))}>
              {icon(Download01Icon)}
            </Button>
            <Button size="icon-sm" variant="ghost" aria-label={t("cloud.close")} title={t("cloud.close")} onClick={() => onIndexChange(null)}>
              {icon(Cancel01Icon)}
            </Button>
          </div>
          <div className={cn("grid", info && "md:grid-cols-[minmax(0,1fr)_20rem]")}>
            <div className="relative grid min-h-[50vh] place-items-center bg-sunk">
              <div className="relative">
                <img src={previewUrl(photo, 1024)} alt={photo.name} className="block max-h-[75vh] max-w-full object-contain" />
                {info &&
                  details?.faces.map((f) => (
                    <span
                      key={f.id}
                      className="pointer-events-none absolute rounded-[6px] ring-2 ring-surface/90 shadow-[0_0_0_1px_rgb(0_0_0/0.35)]"
                      style={{ left: `${f.x * 100}%`, top: `${f.y * 100}%`, width: `${f.w * 100}%`, height: `${f.h * 100}%` }}
                      data-face-box
                    />
                  ))}
              </div>
              <Button
                size="icon"
                variant="secondary"
                className="absolute top-1/2 left-3 -translate-y-1/2"
                aria-label={t("cloud.photos.previous")}
                disabled={index === 0}
                onClick={() => go(-1)}
              >
                {icon(ArrowLeft01Icon)}
              </Button>
              <Button
                size="icon"
                variant="secondary"
                className="absolute top-1/2 right-3 -translate-y-1/2"
                aria-label={t("cloud.photos.next")}
                disabled={index === photos.length - 1}
                onClick={() => go(1)}
              >
                {icon(ArrowRight01Icon)}
              </Button>
            </div>
            {info && <DetailsPanel photo={photo} details={details} people={people} />}
          </div>
        </div>
      )}
    </Dialog>
  );
}

const GROUPS: readonly PhotoField["group"][] = ["picture", "camera", "place", "other"];

/** Everything the camera wrote, the useful parts first. */
function DetailsPanel({ photo, details, people }: { photo: Photo; details: PhotoDetails | null; people: Map<number, Person> }) {
  const t = useT();
  if (details === null) {
    return (
      <aside className="grid min-h-40 place-items-center border-t border-hair md:border-t-0 md:border-l">
        <Spinner />
      </aside>
    );
  }
  const s = details.summary;
  const settings = s === undefined ? [] : [s.aperture, s.exposure, s.iso !== undefined ? `ISO ${s.iso}` : undefined, s.focal].filter((v): v is string => v !== undefined);
  const rows: [string, React.ReactNode][] = [];
  rows.push(s?.taken !== undefined ? [t("cloud.photos.taken"), formatFull(when(photo))] : [t("cloud.photos.changed"), formatFull(photo.mtime)]);
  if (s?.camera !== undefined) rows.push([t("cloud.photos.camera"), s.camera]);
  if (s?.lens !== undefined) rows.push([t("cloud.photos.lens"), s.lens]);
  if (settings.length > 0) rows.push([t("cloud.photos.settings"), settings.join(" · ")]);
  rows.push([t("cloud.photos.size"), s !== undefined ? `${s.width} × ${s.height} · ${formatBytes(photo.size)}` : formatBytes(photo.size)]);
  if (s?.lat !== undefined && s.lon !== undefined)
    rows.push([
      t("cloud.photos.place"),
      <>
        {place(s.lat, s.lon)}
        {s.altitude !== undefined && <span className="block text-muted">{t("cloud.photos.altitude", { m: Math.round(s.altitude) })}</span>}
      </>,
    ]);
  const fields = details.fields ?? [];
  const inPhoto = details.faces.filter((f) => f.person !== null);

  return (
    <aside className="flex max-h-[75vh] flex-col gap-5 overflow-y-auto border-t border-hair px-4 py-4 md:border-t-0 md:border-l" data-photo-details>
      {!details.indexed && <p className="text-[12.5px] text-muted">{t("cloud.photos.notRead")}</p>}
      <dl className="grid grid-cols-[auto_minmax(0,1fr)] gap-x-4 gap-y-2.5 text-[13px]">
        {rows.map(([label, value]) => (
          <React.Fragment key={label}>
            <dt className="text-muted">{label}</dt>
            <dd className="break-words">{value}</dd>
          </React.Fragment>
        ))}
      </dl>
      {inPhoto.length > 0 && (
        <section className="flex flex-col gap-2">
          <h3 className="text-[12.5px] font-medium text-muted">{t("cloud.photos.inPhoto")}</h3>
          <ul className="flex flex-col gap-1.5">
            {inPhoto.map((f) => {
              const p = f.person === null ? undefined : people.get(f.person);
              return (
                <li key={f.id} className="flex items-center gap-2.5 text-[13px]">
                  <img src={faceUrl(f.id)} alt="" className="size-7 rounded-full bg-sunk object-cover" />
                  <span className={cn(f.name === "" && "text-faint")}>{f.name !== "" ? f.name : (p?.name ?? "") || t("cloud.photos.unnamed")}</span>
                </li>
              );
            })}
          </ul>
        </section>
      )}
      {details.indexed &&
        (fields.length === 0 ? (
          <p className="text-[12.5px] text-faint">{t("cloud.photos.noTags")}</p>
        ) : (
          <details className="group flex flex-col gap-3" data-photo-tags>
            <summary className="cursor-pointer text-[12.5px] font-medium text-muted select-none hover:text-ink">
              {t("cloud.photos.allTags", { count: fields.length })}
            </summary>
            <div className="mt-3 flex flex-col gap-4">
              {GROUPS.map((g) => {
                const list = fields.filter((f) => f.group === g);
                if (list.length === 0) return null;
                return (
                  <section key={g} className="flex flex-col gap-1.5">
                    <h4 className="text-[11.5px] font-medium tracking-wide text-faint uppercase">{t(`cloud.photos.group.${g}`)}</h4>
                    <dl className="grid grid-cols-[minmax(0,9rem)_minmax(0,1fr)] gap-x-3 gap-y-1 text-[12px]">
                      {list.map((f) => (
                        <React.Fragment key={f.name}>
                          <dt className="truncate text-muted" title={f.name}>
                            {f.name}
                          </dt>
                          <dd className="break-words">{f.value}</dd>
                        </React.Fragment>
                      ))}
                    </dl>
                  </section>
                );
              })}
            </div>
          </details>
        ))}
    </aside>
  );
}
