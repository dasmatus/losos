import * as React from "react";
import { toast } from "@/components/ui/toast";
import { useT } from "@/lib/i18n-react";
import { chatRunsHere, git, makeChatRoom, roomLink, type Repo } from "./api";
import { failed, SwitchField } from "./parts";
import { useLoad, useMe } from "./store";

/* The repository's room in LosOS Chat, for its administrators on a box
 * that runs chat. Turned on, the box makes the room as this person and
 * LosOS Git posts pushes, pull requests, issues and checks into it
 * (backend/src/git_rooms.rs). Turned off, the repository forgets the room;
 * the room and what was said in it stay in chat, and turning it on again
 * links the same room. */

export function ChatRoomSetting({ repo, reload }: { repo: Repo; reload: () => void }) {
  const t = useT();
  const me = useMe();
  const runs = useLoad((signal) => chatRunsHere(signal), []);
  const [busy, setBusy] = React.useState(false);
  if (runs.data !== true || me === null || me.matrix_id) return null;
  const room = repo.chat_room ?? null;
  const on = room !== null;

  return (
    <section className="flex max-w-xl flex-col gap-3 rounded-card border border-line bg-surface p-4" data-chat-room-setting={on ? "on" : "off"}>
      <h2 className="text-[14px] font-semibold">{t("git.room.title")}</h2>
      <SwitchField
        label={t("git.room.label")}
        description={t(repo.visibility === "public" ? "git.room.hintPublic" : "git.room.hintPrivate")}
        checked={on}
        disabled={busy}
        onChange={(next) => {
          if (busy) return;
          setBusy(true);
          const done = next ? makeChatRoom(repo.owner.login, repo.name) : git.editRepo(repo.owner.login, repo.name, { chat_room: "" });
          done.then(
            () => {
              setBusy(false);
              toast.success(t(next ? "git.room.made" : "git.room.unlinked"));
              reload();
            },
            (err: unknown) => {
              setBusy(false);
              failed(t("git.room.failed"), err);
            },
          );
        }}
      />
      {room !== null && (
        <a href={roomLink(room, me)} className="self-start text-[13px] text-accent hover:underline" data-chat-room-open>
          {t("git.room.open")}
        </a>
      )}
    </section>
  );
}
