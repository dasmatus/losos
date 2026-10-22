import * as React from "react";
import { HugeiconsIcon } from "@hugeicons/react";
import { UserMultipleIcon } from "@hugeicons/core-free-icons";
import { PersonAvatar, RoleBadge } from "@/components/person";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogBody,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import {
  Empty,
  EmptyContent,
  EmptyDescription,
  EmptyHeader,
  EmptyMedia,
  EmptyTitle,
} from "@/components/ui/empty";
import { FieldError } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { NativeSelect, NativeSelectOption } from "@/components/ui/native-select";
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group";
import { Spinner } from "@/components/ui/progress";
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from "@/components/ui/table";
import type { CustomRole, LoginRules, PeopleResponse, PersonEntry, Role } from "@/lib/api";
import { paneHref } from "@/lib/routes";
import { Link } from "react-router-dom";
import type { MessageKey } from "@/lib/i18n";
import { useT } from "@/lib/i18n-react";
import { MIN_PASSWORD_CHARS, passwordRules } from "@/screens/wizard/api";
import { Group, GroupCaption, GroupTitle, PaneSection, Row, RowText, StackRow, SwitchRow } from "./rows";
import { usePeople, type PeopleData } from "./use-people";

/* Users: the people who may sign in to this box's own LosOS Login.
 *
 * Shown only on a box that signs people in itself (`paneOpen` in panes.ts);
 * a box whose sign-in runs elsewhere has nobody to add here, and says so if
 * someone opens the address anyway.
 *
 * Access is a role over backend/sections.json's groups (backend/src/roles.rs):
 * an administrator (losos-admins), a moderator (losos-moderation) or a
 * member with the parts switched on for them (losos-<section>), either of
 * whom may also see every part (losos-viewers); a guest (losos-guests),
 * who gets their own account and nothing else; or a role an administrator
 * saved under a name of their own (losos-role-<id>), which lososd turns
 * into its parts. The owner and the person at the keyboard have no
 * buttons: lososd refuses both, and a button that can only fail is noise. */

/** The wizard's rules: the password opens the admin pages too. */
function strong(password: string): boolean {
  return passwordRules(password).every((rule) => rule.met);
}

const ADMINS = "losos-admins";
const VIEWERS = "losos-viewers";
const OWNER = "losos-owner";
const GUESTS = "losos-guests";
const MODERATORS = "losos-moderation";
const CUSTOM = "losos-role-";
/* Marks lososd keeps in step itself (backend/src/people.rs, MARKS): a
 * second factor, and a rule's hold. Nobody hands them out. */
const MARKS = ["losos-two-factor", "losos-locked"];

/** A built-in role, or `custom:<id>` for one saved under a name. */
type RoleChoice = "admin" | "moderator" | "member" | "guest" | `custom:${string}`;

interface AccessDraft {
  role: RoleChoice;
  /** Sees every part (losos-viewers); a member's or a moderator's. */
  viewer: boolean;
  /** Section names, without the `losos-` prefix. */
  parts: string[];
}

const ROLES: { id: Exclude<RoleChoice, `custom:${string}`>; label: MessageKey; detail: MessageKey }[] = [
  { id: "member", label: "roles.member", detail: "roles.detail.member" },
  { id: "moderator", label: "roles.moderator", detail: "roles.detail.moderator" },
  { id: "guest", label: "roles.guest", detail: "roles.detail.guest" },
  { id: "admin", label: "roles.admin", detail: "roles.detail.admin" },
];

/** Parts the role picker gives, so never a switch of their own. */
const ROLE_SECTIONS = ["moderation"];

/* Each section by the name of the pane it opens, so the switch reads the
 * way the sidebar does. */
const SECTION_LABELS: Record<string, MessageKey> = {
  storage: "settings.panes.storage.label",
  backup: "settings.panes.backup.label",
  apps: "settings.panes.apps.label",
  market: "settings.panes.market.label",
  network: "settings.panes.network.label",
  look: "settings.panes.look.label",
  lab: "shell.nav.lab",
  access: "settings.panes.access.label",
  account: "users.section.account",
  moderation: "settings.panes.moderation.label",
};

/** The access a list of groups gives, as the pane draws it. */
export function accessOf(groups: readonly string[]): AccessDraft {
  const special = [ADMINS, VIEWERS, OWNER, GUESTS, MODERATORS, ...MARKS];
  const parts = groups
    .filter((g) => g.startsWith("losos-") && !special.includes(g) && !g.startsWith(CUSTOM))
    .map((g) => g.slice("losos-".length));
  const custom = groups.find((g) => g.startsWith(CUSTOM));
  if (groups.includes(ADMINS)) return { role: "admin", viewer: false, parts: [] };
  if (groups.includes(GUESTS)) return { role: "guest", viewer: false, parts: [] };
  if (custom !== undefined) return { role: `custom:${custom.slice(CUSTOM.length)}`, viewer: false, parts };
  return {
    role: groups.includes(MODERATORS) ? "moderator" : "member",
    viewer: groups.includes(VIEWERS),
    parts,
  };
}

/** The groups lososd is asked for. A custom role is its mark alone: lososd
 *  adds the parts the role gives. */
export function groupsOf(draft: AccessDraft): string[] {
  if (draft.role === "admin") return [ADMINS];
  if (draft.role === "guest") return [GUESTS, "losos-account"];
  if (draft.role.startsWith("custom:")) return [CUSTOM + draft.role.slice("custom:".length)];
  const parts = draft.parts.filter((p) => !ROLE_SECTIONS.includes(p)).map((s) => `losos-${s}`);
  const seen = draft.viewer ? [VIEWERS, ...parts] : parts;
  return draft.role === "moderator" ? [MODERATORS, ...seen] : seen;
}

/** A person's role from their groups, for a lososd that does not say. */
export function roleOf(groups: readonly string[]): Role {
  if (groups.includes(OWNER)) return "owner";
  if (groups.includes(ADMINS)) return "admin";
  if (groups.includes(GUESTS)) return "guest";
  if (groups.includes(MODERATORS)) return "moderator";
  return "member";
}

export function UsersPane({ locked }: { locked: boolean }) {
  const t = useT();
  const people = usePeople(!locked);
  const { state } = people;

  if (state.kind === "loading") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("users.loading")} />
            <Spinner size={16} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "unavailable") {
    return (
      <PaneSection data-testid="users-unavailable">
        <Group>
          <Row last>
            <RowText title={t("users.unavailable")} />
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (state.kind === "failed") {
    return (
      <PaneSection>
        <Group>
          <Row last>
            <RowText title={t("users.failed")} detail={state.message} />
            <Button size="sm" variant="secondary" disabled={locked} onClick={people.refresh}>
              {t("users.retry")}
            </Button>
          </Row>
        </Group>
      </PaneSection>
    );
  }
  if (!state.view.standalone) {
    return (
      <PaneSection data-testid="users-elsewhere">
        <Empty className="bg-surface">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={UserMultipleIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("users.elsewhere.title")}</EmptyTitle>
            <EmptyDescription>{t("users.elsewhere.body")}</EmptyDescription>
          </EmptyHeader>
        </Empty>
      </PaneSection>
    );
  }

  return (
    <>
      <PeopleSection view={state.view} people={people} disabled={locked || people.busy} />
      <RoleTableSection />
      {state.view.roles !== undefined && (
        <CustomRolesSection view={state.view} people={people} disabled={locked || people.busy} />
      )}
      {state.view.rules !== undefined && (
        <RulesSection rules={state.view.rules} people={people} disabled={locked || people.busy} />
      )}
      <NewPersonSection view={state.view} people={people} locked={locked} />
    </>
  );
}

// ── The list ────────────────────────────────────────────────────────────

function useSectionName(): (section: string) => string {
  const t = useT();
  return (section) => {
    const key = SECTION_LABELS[section];
    return key === undefined ? section : t(key);
  };
}

function PeopleSection({
  view,
  people,
  disabled,
}: {
  view: PeopleResponse;
  people: PeopleData;
  disabled: boolean;
}) {
  const t = useT();
  const sectionName = useSectionName();
  const [editing, setEditing] = React.useState<PersonEntry | null>(null);
  const [resetting, setResetting] = React.useState<PersonEntry | null>(null);
  const [removing, setRemoving] = React.useState<PersonEntry | null>(null);
  const [unpairing, setUnpairing] = React.useState<PersonEntry | null>(null);
  const [aging, setAging] = React.useState<PersonEntry | null>(null);
  const titleId = React.useId();
  const bodyId = React.useId();
  const resetTitleId = React.useId();
  const resetBodyId = React.useId();

  return (
    <PaneSection data-testid="users-people">
      <GroupTitle>{t("users.list.title")}</GroupTitle>
      <Group>
        {view.people.map((person) => {
          const owner = person.groups.includes(OWNER);
          const you = person.name === view.you;
          const access = accessOf(person.groups);
          const parts = access.parts.filter((p) => !ROLE_SECTIONS.includes(p)).map(sectionName);
          const role = person.role ?? roleOf(person.groups);
          const custom = access.role.startsWith("custom:")
            ? view.roles?.find((r) => `custom:${r.id}` === access.role)
            : undefined;
          return (
            <StackRow key={person.name} data-testid="users-person" data-person={person.name} className="gap-1.5">
              <div className="flex flex-wrap items-center gap-x-3 gap-y-2">
                <PersonAvatar person={{ ...person, role }} size="md" showRole data-testid="users-avatar" />
                <div className="min-w-0 flex-1">
                  <p className="truncate text-sm leading-snug text-ink">{person.displayName}</p>
                  <p className="truncate text-[12.5px] leading-snug text-muted">
                    {person.name}
                    {person.email.length > 0 && ` · ${person.email}`}
                  </p>
                </div>
                <div className="flex shrink-0 flex-wrap items-center gap-1.5">
                  {you && <Badge variant="outline">{t("users.badge.you")}</Badge>}
                  {person.suspended != null ? (
                    <Badge variant="crit" data-testid="users-suspended">
                      {t("moderation.suspended")}
                    </Badge>
                  ) : (
                    person.disabled && <Badge variant="warn">{t("users.badge.disabled")}</Badge>
                  )}
                  {person.muted != null && (
                    <Badge variant="warn" data-testid="users-muted">
                      {t("moderation.muted")}
                    </Badge>
                  )}
                  {person.twoFactor === true && (
                    <Badge variant="ok" data-testid="users-two-factor">
                      {t("users.badge.twoFactor")}
                    </Badge>
                  )}
                  {(person.locked?.length ?? 0) > 0 && (
                    <Badge variant="warn" data-testid="users-held">
                      {t("users.badge.held")}
                    </Badge>
                  )}
                  <RoleBadge
                    person={{ role, roleName: person.roleName ?? custom?.name ?? null }}
                    data-testid="users-level"
                  />
                  {person.age != null && (
                    <Badge variant="outline" data-testid="users-age">
                      {t(person.age.age === "child" ? "pictures.age.badgeChild" : "pictures.age.badgeAdult")}
                    </Badge>
                  )}
                </div>
              </div>
              {role !== "admin" && role !== "owner" && (
                <p className="text-[12.5px] leading-snug text-muted">
                  {role === "guest"
                    ? t("roles.guestCannot")
                    : custom !== undefined
                      ? custom.sections.length === 0
                        ? t("roles.custom.givesNothing")
                        : t("roles.custom.gives", { list: custom.sections.map(sectionName).join(", ") })
                      : parts.length === 0
                        ? access.viewer
                          ? t("users.seesEverything")
                          : t("users.canChangeNothing")
                        : t("users.canChange", { list: parts.join(", ") })}
                </p>
              )}
              {person.locked?.map((reason) => (
                <p key={reason} className="text-[12.5px] leading-snug text-warn">
                  {t(reason === "password" ? "users.held.password" : "users.held.twoFactor")}
                </p>
              ))}
              {!you && person.twoFactor === true && owner && (
                <div className="flex flex-wrap justify-end gap-2 pt-0.5">
                  <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setUnpairing(person)}>
                    {t("users.action.resetFactors")}
                  </Button>
                </div>
              )}
              {!owner && !you && (
                <div className="flex flex-wrap justify-end gap-2 pt-0.5">
                  {person.twoFactor === true && (
                    <Button
                      size="sm"
                      variant="secondary"
                      disabled={disabled}
                      data-testid="users-reset-factors"
                      onClick={() => setUnpairing(person)}
                    >
                      {t("users.action.resetFactors")}
                    </Button>
                  )}
                  <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setEditing(person)}>
                    {t("users.action.access")}
                  </Button>
                  <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setResetting(person)}>
                    {t("users.action.password")}
                  </Button>
                  <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setRemoving(person)}>
                    {t("users.action.remove")}
                  </Button>
                </div>
              )}
              {"age" in person && (
                <div className="flex flex-wrap justify-end gap-2 pt-0.5">
                  <Button size="sm" variant="ghost" disabled={disabled} data-testid="users-age-edit" onClick={() => setAging(person)}>
                    {t("pictures.age.edit")}
                  </Button>
                </div>
              )}
              {!you && (person.picture != null || person.banner != null) && (
                <div className="flex flex-wrap justify-end gap-2 pt-0.5">
                  {(["picture", "banner"] as const)
                    .filter((kind) => person[kind] != null)
                    .map((kind) => (
                      <Button
                        key={kind}
                        size="sm"
                        variant="ghost"
                        disabled={disabled}
                        data-testid={`users-take-down-${kind}`}
                        onClick={() => void people.takeDown(person.name, kind)}
                      >
                        {t(`profile.users.takeDown.${kind}`)}
                      </Button>
                    ))}
                </div>
              )}
            </StackRow>
          );
        })}
      </Group>
      <GroupCaption>{t("users.list.caption")}</GroupCaption>

      <AccessDialog
        person={editing}
        sections={view.sections}
        roles={view.roles ?? []}
        people={people}
        onClose={() => setEditing(null)}
      />
      <PasswordDialog person={resetting} people={people} onClose={() => setResetting(null)} />
      <AgeDialog person={aging} people={people} onClose={() => setAging(null)} />
      <Dialog
        open={unpairing !== null}
        onOpenChange={(open) => {
          if (!open) setUnpairing(null);
        }}
        labelledBy={resetTitleId}
        describedBy={resetBodyId}
      >
        <DialogHeader>
          <DialogTitle id={resetTitleId}>
            {t("users.resetDialog.title", { name: unpairing?.displayName ?? "" })}
          </DialogTitle>
          <DialogDescription id={resetBodyId}>{t("users.resetDialog.body")}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setUnpairing(null)}>
            {t("users.keep")}
          </Button>
          <Button
            data-testid="users-reset-factors-confirm"
            onClick={() => {
              const person = unpairing;
              setUnpairing(null);
              if (person !== null) void people.resetFactors(person.name);
            }}
          >
            {t("users.resetDialog.confirm")}
          </Button>
        </DialogFooter>
      </Dialog>
      <Dialog
        open={removing !== null}
        onOpenChange={(open) => {
          if (!open) setRemoving(null);
        }}
        labelledBy={titleId}
        describedBy={bodyId}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>
            {t("users.removeDialog.title", { name: removing?.displayName ?? "" })}
          </DialogTitle>
          <DialogDescription id={bodyId}>{t("users.removeDialog.body")}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setRemoving(null)}>
            {t("users.keep")}
          </Button>
          <Button
            data-testid="users-remove-confirm"
            onClick={() => {
              const person = removing;
              setRemoving(null);
              if (person !== null) void people.remove(person.name);
            }}
          >
            {t("users.action.remove")}
          </Button>
        </DialogFooter>
      </Dialog>
    </PaneSection>
  );
}

// ── What each role can do ───────────────────────────────────────────────

type Cell = MessageKey;

/* Each role down the columns, each part of the box down the rows, in the
 * words the rest of the pane uses. The table states what lososd enforces
 * (backend/sections.json, backend/src/roles.rs); it decides nothing. */
const TABLE_ROLES: Role[] = ["owner", "admin", "moderator", "member", "guest"];
const TABLE: { what: MessageKey; cells: Record<Role, Cell> }[] = [
  {
    what: "roles.table.settings",
    cells: {
      owner: "roles.cell.all",
      admin: "roles.cell.all",
      moderator: "roles.cell.given",
      member: "roles.cell.given",
      guest: "roles.cell.ownAccount",
    },
  },
  {
    what: "roles.table.people",
    cells: {
      owner: "roles.cell.yes",
      admin: "roles.cell.yes",
      moderator: "roles.cell.no",
      member: "roles.cell.no",
      guest: "roles.cell.no",
    },
  },
  {
    what: "roles.table.moderation",
    cells: {
      owner: "roles.cell.all",
      admin: "roles.cell.all",
      moderator: "roles.cell.membersGuests",
      member: "roles.cell.no",
      guest: "roles.cell.no",
    },
  },
  {
    what: "roles.table.files",
    cells: {
      owner: "roles.cell.own",
      admin: "roles.cell.own",
      moderator: "roles.cell.own",
      member: "roles.cell.own",
      guest: "roles.cell.own",
    },
  },
  {
    what: "roles.table.mail",
    cells: {
      owner: "roles.cell.boxOnly",
      admin: "roles.cell.boxOnly",
      moderator: "roles.cell.boxOnly",
      member: "roles.cell.boxOnly",
      guest: "roles.cell.boxOnly",
    },
  },
  {
    what: "roles.table.ci",
    cells: {
      owner: "roles.cell.yes",
      admin: "roles.cell.yes",
      moderator: "roles.cell.yes",
      member: "roles.cell.yes",
      guest: "roles.cell.read",
    },
  },
  {
    what: "roles.table.profile",
    cells: {
      owner: "roles.cell.yes",
      admin: "roles.cell.yes",
      moderator: "roles.cell.yes",
      member: "roles.cell.yes",
      guest: "roles.cell.yes",
    },
  },
  {
    what: "roles.table.report",
    cells: {
      owner: "roles.cell.yes",
      admin: "roles.cell.yes",
      moderator: "roles.cell.yes",
      member: "roles.cell.yes",
      guest: "roles.cell.yes",
    },
  },
];

function RoleTableSection() {
  const t = useT();
  return (
    <PaneSection data-testid="users-role-table">
      <GroupTitle>{t("roles.table.title")}</GroupTitle>
      <Group className="overflow-x-auto">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead>{t("roles.table.what")}</TableHead>
              {TABLE_ROLES.map((role) => (
                <TableHead key={role}>{t(`roles.${role}`)}</TableHead>
              ))}
            </TableRow>
          </TableHeader>
          <TableBody>
            {TABLE.map((row) => (
              <TableRow key={row.what}>
                <TableCell className="text-ink">{t(row.what)}</TableCell>
                {TABLE_ROLES.map((role) => (
                  <TableCell
                    key={role}
                    className={row.cells[role] === "roles.cell.no" ? "text-faint" : "text-muted"}
                  >
                    {t(row.cells[role])}
                  </TableCell>
                ))}
              </TableRow>
            ))}
          </TableBody>
        </Table>
      </Group>
      <GroupCaption>{t("roles.table.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── Roles of the box's own ──────────────────────────────────────────────

function CustomRolesSection({
  view,
  people,
  disabled,
}: {
  view: PeopleResponse;
  people: PeopleData;
  disabled: boolean;
}) {
  const t = useT();
  const sectionName = useSectionName();
  const roles = view.roles ?? [];
  const [editing, setEditing] = React.useState<CustomRole | "new" | null>(null);
  const [removing, setRemoving] = React.useState<CustomRole | null>(null);
  const titleId = React.useId();
  const bodyId = React.useId();
  return (
    <PaneSection data-testid="users-roles">
      <GroupTitle>{t("roles.custom.title")}</GroupTitle>
      {roles.length === 0 ? (
        <Empty className="bg-surface">
          <EmptyHeader>
            <EmptyMedia variant="icon">
              <HugeiconsIcon icon={UserMultipleIcon} strokeWidth={1.5} color="currentColor" aria-hidden="true" />
            </EmptyMedia>
            <EmptyTitle>{t("roles.custom.emptyTitle")}</EmptyTitle>
            <EmptyDescription>{t("roles.custom.emptyBody")}</EmptyDescription>
          </EmptyHeader>
          <EmptyContent>
            <Button size="sm" disabled={disabled} data-testid="users-role-new" onClick={() => setEditing("new")}>
              {t("roles.custom.add")}
            </Button>
          </EmptyContent>
        </Empty>
      ) : (
        <>
          <Group>
            {roles.map((role, i) => (
              <Row key={role.id} last={i === roles.length - 1} data-testid="users-role" data-role={role.id}>
                <RowText
                  title={role.name}
                  detail={
                    role.sections.length === 0
                      ? t("roles.custom.givesNothing")
                      : t("roles.custom.gives", { list: role.sections.map(sectionName).join(", ") })
                  }
                />
                <div className="flex shrink-0 gap-2">
                  <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setEditing(role)}>
                    {t("roles.custom.edit")}
                  </Button>
                  <Button size="sm" variant="secondary" disabled={disabled} onClick={() => setRemoving(role)}>
                    {t("roles.custom.remove")}
                  </Button>
                </div>
              </Row>
            ))}
          </Group>
          <div className="flex justify-end pt-3">
            <Button size="sm" disabled={disabled} data-testid="users-role-new" onClick={() => setEditing("new")}>
              {t("roles.custom.add")}
            </Button>
          </div>
        </>
      )}
      <GroupCaption>{t("roles.custom.caption")}</GroupCaption>
      <RoleDialog
        role={editing}
        sections={view.sections}
        people={people}
        onClose={() => setEditing(null)}
      />
      <Dialog
        open={removing !== null}
        onOpenChange={(open) => {
          if (!open) setRemoving(null);
        }}
        labelledBy={titleId}
        describedBy={bodyId}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>{t("roles.custom.removeTitle", { name: removing?.name ?? "" })}</DialogTitle>
          <DialogDescription id={bodyId}>{t("roles.custom.removeBody")}</DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="ghost" onClick={() => setRemoving(null)}>
            {t("roles.custom.keep")}
          </Button>
          <Button
            data-testid="users-role-remove-confirm"
            onClick={() => {
              const role = removing;
              setRemoving(null);
              if (role !== null) void people.removeRole(role.id, role.name);
            }}
          >
            {t("roles.custom.remove")}
          </Button>
        </DialogFooter>
      </Dialog>
    </PaneSection>
  );
}

function RoleDialog({
  role,
  sections,
  people,
  onClose,
}: {
  role: CustomRole | "new" | null;
  sections: string[];
  people: PeopleData;
  onClose: () => void;
}) {
  const t = useT();
  const sectionName = useSectionName();
  const titleId = React.useId();
  const bodyId = React.useId();
  const nameId = React.useId();
  const [name, setName] = React.useState("");
  const [parts, setParts] = React.useState<string[]>([]);
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (role === null) return;
    setName(role === "new" ? "" : role.name);
    setParts(role === "new" ? ["account"] : role.sections);
    setError(null);
  }, [role]);

  const ready = name.trim().length > 0;
  const save = async () => {
    if (role === null || !ready) return;
    const outcome = await people.saveRole({
      ...(role === "new" ? {} : { id: role.id }),
      name,
      sections: parts,
    });
    if (outcome.ok) onClose();
    else if (outcome.message.length > 0) setError(outcome.message);
  };

  return (
    <Dialog
      open={role !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(30rem,calc(100vw-2rem))]"
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>
            {role === null || role === "new"
              ? t("roles.custom.newTitle")
              : t("roles.custom.editTitle", { name: role.name })}
          </DialogTitle>
          <DialogDescription id={bodyId}>{t("roles.custom.dialogBody")}</DialogDescription>
        </DialogHeader>
        <DialogBody>
          <Group>
            <FieldRow id={nameId} title={t("roles.custom.name")}>
              <Input
                id={nameId}
                data-testid="users-role-name"
                value={name}
                maxLength={40}
                placeholder={t("roles.custom.namePlaceholder")}
                disabled={people.busy}
                aria-invalid={error !== null ? true : undefined}
                onChange={(event) => setName(event.target.value)}
              />
            </FieldRow>
            {sections.map((section, i) => (
              <SwitchRow
                key={section}
                id={`${nameId}-${section}`}
                title={sectionName(section)}
                checked={parts.includes(section)}
                disabled={people.busy}
                last={i === sections.length - 1}
                option={section}
                onCheckedChange={(on) =>
                  setParts((p) => (on ? [...p, section] : p.filter((s) => s !== section)))
                }
              />
            ))}
          </Group>
          {error !== null && <FieldError className="pt-2">{error}</FieldError>}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            {t("users.cancel")}
          </Button>
          <Button type="submit" data-testid="users-role-save" disabled={people.busy || !ready}>
            {t("roles.custom.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

// ── Access, in the form and in the dialog ───────────────────────────────

function AccessRows({
  draft,
  sections,
  roles,
  disabled,
  onChange,
}: {
  draft: AccessDraft;
  sections: string[];
  roles: CustomRole[];
  disabled: boolean;
  onChange: (draft: AccessDraft) => void;
}) {
  const t = useT();
  const sectionName = useSectionName();
  const id = React.useId();
  const custom = roles.find((r) => `custom:${r.id}` === draft.role);
  const detail =
    custom !== undefined
      ? custom.sections.length === 0
        ? t("roles.detail.customNothing")
        : t("roles.detail.custom", { list: custom.sections.map(sectionName).join(", ") })
      : t(ROLES.find((r) => r.id === draft.role)?.detail ?? "roles.detail.member");
  const parts = draft.role === "member" || draft.role === "moderator";
  const switches = sections.filter((s) => !ROLE_SECTIONS.includes(s));
  return (
    <>
      <Row last={!parts}>
        <RowText htmlFor={id} title={t("roles.label")} detail={detail} />
        <div className="w-[16rem] max-sm:w-[11rem]">
          <NativeSelect
            id={id}
            className="w-full"
            value={draft.role}
            disabled={disabled}
            data-testid="users-role-select"
            onChange={(event) => onChange({ ...draft, role: event.target.value as RoleChoice })}
          >
            {ROLES.map((role) => (
              <NativeSelectOption key={role.id} value={role.id}>
                {t(role.label)}
              </NativeSelectOption>
            ))}
            {roles.map((role) => (
              <NativeSelectOption key={role.id} value={`custom:${role.id}`}>
                {role.name}
              </NativeSelectOption>
            ))}
          </NativeSelect>
        </div>
      </Row>
      {parts && (
        <SwitchRow
          id={`${id}-viewer`}
          title={t("roles.viewer")}
          description={t("roles.viewerDetail")}
          checked={draft.viewer}
          disabled={disabled}
          option="viewer"
          onCheckedChange={(viewer) => onChange({ ...draft, viewer })}
        />
      )}
      {parts &&
        switches.map((section, i) => (
          <SwitchRow
            key={section}
            id={`${id}-${section}`}
            title={sectionName(section)}
            checked={draft.parts.includes(section)}
            disabled={disabled}
            last={i === switches.length - 1}
            onCheckedChange={(on) =>
              onChange({
                ...draft,
                parts: on ? [...draft.parts, section] : draft.parts.filter((p) => p !== section),
              })
            }
          />
        ))}
    </>
  );
}

function AccessDialog({
  person,
  sections,
  roles,
  people,
  onClose,
}: {
  person: PersonEntry | null;
  sections: string[];
  roles: CustomRole[];
  people: PeopleData;
  onClose: () => void;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const [draft, setDraft] = React.useState<AccessDraft>({ role: "member", viewer: false, parts: [] });
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    if (person !== null) {
      setDraft(accessOf(person.groups));
      setError(null);
    }
  }, [person]);

  const save = async () => {
    if (person === null) return;
    const outcome = await people.setAccess(person.name, groupsOf(draft));
    if (outcome.ok) onClose();
    else if (outcome.message.length > 0) setError(outcome.message);
  };

  return (
    <Dialog
      open={person !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      labelledBy={titleId}
      describedBy={bodyId}
      dialogClassName="w-[min(30rem,calc(100vw-2rem))]"
    >
      <DialogHeader>
        <DialogTitle id={titleId}>
          {t("users.accessDialog.title", { name: person?.displayName ?? "" })}
        </DialogTitle>
        <DialogDescription id={bodyId}>{t("users.accessDialog.body")}</DialogDescription>
      </DialogHeader>
      <DialogBody>
        <Group>
          <AccessRows draft={draft} sections={sections} roles={roles} disabled={people.busy} onChange={setDraft} />
        </Group>
        {error !== null && <FieldError className="pt-2">{error}</FieldError>}
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onClose}>
          {t("users.cancel")}
        </Button>
        <Button data-testid="users-access-save" disabled={people.busy} onClick={() => void save()}>
          {t("users.save")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}

function PasswordDialog({
  person,
  people,
  onClose,
}: {
  person: PersonEntry | null;
  people: PeopleData;
  onClose: () => void;
}) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const fieldId = React.useId();
  const [password, setPassword] = React.useState("");
  const [error, setError] = React.useState<string | null>(null);

  React.useEffect(() => {
    setPassword("");
    setError(null);
  }, [person]);

  const ready = strong(password);
  const save = async () => {
    if (person === null || !ready) return;
    const outcome = await people.setPassword(person.name, password);
    if (outcome.ok) onClose();
    else if (outcome.message.length > 0) setError(outcome.message);
  };

  return (
    <Dialog
      open={person !== null}
      onOpenChange={(open) => {
        if (!open) onClose();
      }}
      labelledBy={titleId}
      describedBy={bodyId}
    >
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <DialogHeader>
          <DialogTitle id={titleId}>
            {t("users.passwordDialog.title", { name: person?.displayName ?? "" })}
          </DialogTitle>
          <DialogDescription id={bodyId}>{t("users.passwordDialog.body")}</DialogDescription>
        </DialogHeader>
        <DialogBody className="flex flex-col gap-2">
          <label htmlFor={fieldId} className="text-sm text-ink">
            {t("users.form.password")}
          </label>
          <Input
            id={fieldId}
            type="password"
            autoComplete="new-password"
            value={password}
            disabled={people.busy}
            aria-invalid={error !== null ? true : undefined}
            onChange={(event) => setPassword(event.target.value)}
          />
          <p className="text-[12.5px] leading-snug text-muted">
            {t("users.form.passwordDetail", { count: MIN_PASSWORD_CHARS })}
          </p>
          {error !== null && <FieldError>{error}</FieldError>}
        </DialogBody>
        <DialogFooter>
          <Button type="button" variant="ghost" onClick={onClose}>
            {t("users.cancel")}
          </Button>
          <Button type="submit" disabled={people.busy || !ready}>
            {t("users.save")}
          </Button>
        </DialogFooter>
      </form>
    </Dialog>
  );
}

// ── Sign-in rules ───────────────────────────────────────────────────────

/** backend/src/login.rs, `MAX_PASSWORD_DAYS`. */
export const MAX_PASSWORD_DAYS = 1200;
/** What the days field starts at when the rule is switched on. */
const DEFAULT_PASSWORD_DAYS = 90;

function RulesSection({
  rules,
  people,
  disabled,
}: {
  rules: LoginRules;
  people: PeopleData;
  disabled: boolean;
}) {
  const t = useT();
  const ids = { twoFactor: React.useId(), expire: React.useId(), days: React.useId() };
  const [twoFactor, setTwoFactor] = React.useState(rules.twoFactor);
  const [expire, setExpire] = React.useState(rules.passwordDays > 0);
  const [days, setDays] = React.useState(String(rules.passwordDays > 0 ? rules.passwordDays : DEFAULT_PASSWORD_DAYS));
  const [error, setError] = React.useState<{ message: string; ownFactor: boolean } | null>(null);

  // The box's answer is the new starting point after every save.
  React.useEffect(() => {
    setTwoFactor(rules.twoFactor);
    setExpire(rules.passwordDays > 0);
    if (rules.passwordDays > 0) setDays(String(rules.passwordDays));
  }, [rules.twoFactor, rules.passwordDays]);

  const count = Number(days);
  const daysValid = Number.isInteger(count) && count >= 1 && count <= MAX_PASSWORD_DAYS;
  const next: LoginRules = { twoFactor, passwordDays: expire ? count : 0 };
  const changed = next.twoFactor !== rules.twoFactor || next.passwordDays !== rules.passwordDays;
  const ready = changed && (!expire || daysValid);

  const save = async () => {
    if (!ready) return;
    setError(null);
    const outcome = await people.setRules(next);
    if (!outcome.ok && outcome.message.length > 0) {
      const ownFactor = outcome.status === 409;
      setError({ message: ownFactor ? t("users.rules.needsOwnFactor") : outcome.message, ownFactor });
    }
  };

  return (
    <PaneSection data-testid="users-rules">
      <GroupTitle>{t("users.rules.title")}</GroupTitle>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void save();
        }}
      >
        <Group>
          <SwitchRow
            id={ids.twoFactor}
            title={t("users.rules.twoFactor")}
            description={t("users.rules.twoFactorDetail")}
            checked={twoFactor}
            disabled={disabled}
            onCheckedChange={setTwoFactor}
          />
          <SwitchRow
            id={ids.expire}
            title={t("users.rules.expire")}
            description={t("users.rules.expireDetail")}
            checked={expire}
            disabled={disabled}
            last={!expire}
            onCheckedChange={setExpire}
          />
          {expire && (
            <FieldRow
              id={ids.days}
              title={t("users.rules.days")}
              detail={t("users.rules.daysDetail", { max: String(MAX_PASSWORD_DAYS) })}
              last
            >
              <Input
                id={ids.days}
                data-testid="users-rules-days"
                type="number"
                inputMode="numeric"
                min={1}
                max={MAX_PASSWORD_DAYS}
                value={days}
                disabled={disabled}
                aria-invalid={!daysValid ? true : undefined}
                onChange={(event) => setDays(event.target.value)}
              />
            </FieldRow>
          )}
        </Group>
        <div className="flex flex-wrap items-center justify-end gap-3 pt-3">
          {error !== null && (
            <FieldError className="mr-auto" data-testid="users-rules-error">
              {error.message}{" "}
              {error.ownFactor && (
                <Link to={paneHref("account")} className="underline underline-offset-2">
                  {t("users.rules.openAccount")}
                </Link>
              )}
            </FieldError>
          )}
          <Button type="submit" size="sm" data-testid="users-rules-save" disabled={disabled || !ready}>
            {t("users.rules.save")}
          </Button>
        </div>
      </form>
      <GroupCaption>{t("users.rules.caption")}</GroupCaption>
    </PaneSection>
  );
}

// ── Adding someone ──────────────────────────────────────────────────────

interface Draft {
  name: string;
  displayName: string;
  password: string;
  access: AccessDraft;
}

/* A new person starts with their own account (name, password, second
 * factors), which most people need and an administrator rarely means to
 * withhold. */
const EMPTY: Draft = {
  name: "",
  displayName: "",
  password: "",
  access: { role: "member", viewer: false, parts: ["account"] },
};

/** The shape lososd accepts for a sign-in name (people.rs, validate_name). */
export const NAME_PATTERN = /^[a-z][a-z0-9._-]{0,31}$/;

function NewPersonSection({
  view,
  people,
  locked,
}: {
  view: PeopleResponse;
  people: PeopleData;
  locked: boolean;
}) {
  const t = useT();
  const ids = { name: React.useId(), display: React.useId(), password: React.useId() };
  const [draft, setDraft] = React.useState<Draft>(EMPTY);
  const [error, setError] = React.useState<string | null>(null);
  const disabled = locked || people.busy;
  const name = draft.name.trim();
  const ready = NAME_PATTERN.test(name) && strong(draft.password);

  const submit = async () => {
    if (!ready) return;
    setError(null);
    const outcome = await people.add({
      name,
      displayName: draft.displayName.trim(),
      password: draft.password,
      groups: groupsOf(draft.access),
    });
    if (outcome.ok) setDraft(EMPTY);
    else if (outcome.message.length > 0) setError(outcome.message);
  };

  return (
    <PaneSection data-testid="users-form">
      <GroupTitle>{t("users.form.title")}</GroupTitle>
      <form
        onSubmit={(event) => {
          event.preventDefault();
          void submit();
        }}
      >
        <Group>
          <FieldRow id={ids.name} title={t("users.form.name")} detail={t("users.form.nameDetail")}>
            <Input
              id={ids.name}
              data-testid="users-name"
              value={draft.name}
              placeholder={t("users.form.namePlaceholder")}
              maxLength={32}
              autoComplete="off"
              autoCapitalize="none"
              spellCheck={false}
              disabled={disabled}
              aria-invalid={error !== null ? true : undefined}
              onChange={(event) => setDraft((d) => ({ ...d, name: event.target.value.toLowerCase() }))}
            />
          </FieldRow>
          <FieldRow id={ids.display} title={t("users.form.displayName")}>
            <Input
              id={ids.display}
              data-testid="users-display-name"
              value={draft.displayName}
              placeholder={t("users.form.displayNamePlaceholder")}
              maxLength={64}
              autoComplete="off"
              disabled={disabled}
              onChange={(event) => setDraft((d) => ({ ...d, displayName: event.target.value }))}
            />
          </FieldRow>
          <FieldRow
            id={ids.password}
            title={t("users.form.password")}
            detail={t("users.form.passwordDetail", { count: MIN_PASSWORD_CHARS })}
          >
            <Input
              id={ids.password}
              data-testid="users-password"
              type="password"
              autoComplete="new-password"
              value={draft.password}
              disabled={disabled}
              onChange={(event) => setDraft((d) => ({ ...d, password: event.target.value }))}
            />
          </FieldRow>
          <AccessRows
            draft={draft.access}
            sections={view.sections}
            roles={view.roles ?? []}
            disabled={disabled}
            onChange={(access) => setDraft((d) => ({ ...d, access }))}
          />
        </Group>
        <div className="flex flex-wrap items-center justify-end gap-3 pt-3">
          {error !== null && (
            <FieldError className="mr-auto" data-testid="users-error">
              {error}
            </FieldError>
          )}
          <Button type="submit" size="sm" data-testid="users-add" disabled={disabled || !ready}>
            {t("users.form.add")}
          </Button>
        </div>
      </form>
      <GroupCaption>{t("users.form.caption")}</GroupCaption>
    </PaneSection>
  );
}

function FieldRow({
  id,
  title,
  detail,
  last,
  children,
}: {
  id: string;
  title: string;
  detail?: string | undefined;
  last?: boolean;
  children: React.ReactNode;
}) {
  return (
    <Row last={last}>
      <RowText htmlFor={id} title={title} detail={detail} />
      <div className="w-[16rem] max-sm:w-[11rem]">{children}</div>
    </Row>
  );
}

// ── The age mark for pictures (backend/src/pictures.rs) ─────────────────

const AGES = ["adult", "child", "none"] as const;

function AgeDialog({ person, people, onClose }: { person: PersonEntry | null; people: PeopleData; onClose: () => void }) {
  const t = useT();
  const titleId = React.useId();
  const bodyId = React.useId();
  const [age, setAge] = React.useState<(typeof AGES)[number]>("none");
  React.useEffect(() => setAge(person?.age?.age ?? "none"), [person]);
  return (
    <Dialog open={person !== null} onOpenChange={(open) => !open && onClose()} labelledBy={titleId} describedBy={bodyId}>
      <DialogHeader>
        <DialogTitle id={titleId}>{t("pictures.age.dialogTitle", { name: person?.displayName ?? "" })}</DialogTitle>
        <DialogDescription id={bodyId}>{t("pictures.age.dialogBody")}</DialogDescription>
      </DialogHeader>
      <DialogBody className="flex flex-col gap-3">
        <ToggleGroup
          aria-labelledby={titleId}
          value={age}
          onValueChange={(next) => {
            const found = AGES.find((a) => a === next);
            if (found !== undefined) setAge(found);
          }}
          data-testid="users-age-choice"
        >
          {AGES.map((a) => (
            <ToggleGroupItem key={a} value={a} className="px-3">
              {t(`pictures.age.choice.${a}`)}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
        <p className="text-[12.5px] leading-snug text-muted">{t(`pictures.age.choice.${age}.detail`)}</p>
      </DialogBody>
      <DialogFooter>
        <Button variant="ghost" onClick={onClose}>
          {t("users.keep")}
        </Button>
        <Button
          data-testid="users-age-save"
          onClick={() => {
            const name = person?.name;
            onClose();
            if (name !== undefined) void people.setAge(name, age === "none" ? null : age);
          }}
        >
          {t("pictures.age.save")}
        </Button>
      </DialogFooter>
    </Dialog>
  );
}
