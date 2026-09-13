# Hosting installer ISO releases off-forge

This repository already released: a `v*` tag built the ISO and attached it to a
Forgejo release, proven on `v0.0.1-ci-test` and `v0.1.0`. What it could not do
was release *twice*. `flake/disk-images.nix:56-66` already said where this was
going — a larger image "cannot be published on Codeberg", and if it ever is
published "it goes somewhere that hosts large files".

This is that change. Release *creation* is kept and refactored, not rewritten;
only the destination of the bytes moves.

## The constraint that shapes everything

Codeberg asks a repository to stay under "1.5 GiB of packages, LFS and
attachments". One installer ISO is that entire allowance. Attaching a second
puts the repository over it permanently, so releases were either rationed to
near-zero or impossible.

The image therefore moves off-forge, and the release page becomes an index
rather than a store: it carries the download link, the sha256 and the size,
while the bytes live somewhere that hosts large files.

## Why buzzheavier

The upload is a single authenticated PUT:

```sh
curl -T image.iso -H "authorization: Bearer $TOKEN" \
  "https://w.buzzheavier.com/{parentId}/image.iso"
# -> {"data":{"id":"..."}}, and the download is https://buzzheavier.com/{id}
```

No two-step handshake, no presigning, no multipart protocol, no SDK — curl and
jq, both of which CI already has. The file-manager API supplies the rest:
`GET /api/fs/{id}` lists a directory *with names*, and `DELETE /api/fs/{id}`
removes one, so pruning is one list plus k deletes.

The API hosts answer plain curl. The HTML pages at `buzzheavier.com` sit behind
a Cloudflare interstitial and return 403 to a non-browser, but
`w.buzzheavier.com` and `buzzheavier.com/api/*` do not — verified before this
was built (`GET /api/locations` → 200 JSON, `GET /api/fs` → 401 JSON rather
than a challenge page). **Never add a step that fetches an HTML page from
buzzheavier; it will 403.**

Rejected alternatives:

- **A release attachment.** The thing this exists to avoid.
- **A self-hosted API on Vercel + object storage.** Built and deployed, then
  dropped: its Cloudflare R2 account became inaccessible, and the Vercel Blob
  replacement could not carry the load. Blob on the Hobby plan includes 1 GB of
  storage and 10 GB/month of transfer — one ISO exceeds the storage allowance
  by itself — and the documented overage behaviour is not billing but a 30-day
  lockout of the store, which would have taken every previously published link
  offline at once.

## The expiry treadmill

buzzheavier's free tier keeps a file for **8 days**, adds 2 days per download
(capped at 60), and only makes it permanent after **30 downloads inside 60
days**. Neither repository's ISO will see 30 downloads, so every published link
dies about a week after the release unless something refreshes it.

`.forgejo/workflows/refresh.yml` re-uploads the newest release's image and
rewrites that release's body with the new link. Design notes:

- **Twice weekly (Mon + Thu), not weekly.** Against an 8-day floor a weekly run
  leaves one day of margin, so a single failed run kills the link.
- **Only the newest release.** Older ones are allowed to expire. Their bodies
  keep the sha256, so an old release stays *verifiable* even once it stops
  being *downloadable*.
- **It never builds.** The release lane records the image's `/nix/store` path in
  a `release.json` asset; the refresh substitutes that path from the binary
  cache and re-uploads the bytes. No flake evaluation, no nixpkgs fetch, and no
  CPU spent reproducing bytes the cache already holds — which matters, because
  this runs forever on a shared, donation-funded runner.
- **It verifies before it publishes.** The substituted image's sha256 must match
  the one recorded in `release.json`, so a refresh can never quietly swap the
  release's link for different bytes.
- **It does not game the download counter.** Issuing fake downloads to reach the
  30-download permanence threshold would be abuse of a free service. The
  refresh is an honest re-upload.

If paying for storage ever becomes preferable to the treadmill, buzzheavier
sells permanent storage; the refresh workflow then simply gets deleted, and
nothing else in this design changes.

## Composite actions

The upload, release and prune steps are local composite actions under
`.forgejo/actions/`. Forgejo resolves `uses: ./…` from the checked-out tree, so
**every job using one must run `actions/checkout` first.**

- `publish-buzz` — upload, with an explicit retry loop (curl's `--retry` reports
  nothing per attempt, and a failed multi-GB upload is exactly where you want
  the attempt count in the log). Nothing needs rolling back between attempts:
  buzzheavier mints the id only on success. The token is passed through a
  `curl -K` config file under `umask 077`, never argv, because on a shared
  runner `ps` would otherwise expose it for the whole upload — and the token is
  the account id itself, which cannot be scoped.
- `forgejo-release` — create a release for a tag, or replace an existing body
  (`mode: patch`, which the refresh depends on: it replaces outright rather than
  appending, so a dead link leaves the page). Every call checks its HTTP status
  explicitly, because `curl -sS` exits 0 on a 4xx.
- `prune-buzz` — keeps the newest N matching a prefix. It **never fails the
  job**: a release whose bytes are published and whose page is written is a
  successful release, and failing it afterwards over housekeeping would turn a
  tidy-up problem into a tag that has to be deleted and re-pushed. It also
  refuses to guess — if the listing cannot be ordered confidently it skips,
  because keeping too many files costs nothing on a tier where files expire
  anyway, while deleting the wrong one destroys a published link.

Tokens are resolved in shell (`"${TOKEN:-$TOKEN_ALT}"`) rather than with a `||`
expression, matching the idiom of the release block this was lifted from: it
works on this runner, and the expression engine's handling of `||` is unverified
here.

## Setup

1. A buzzheavier account. Its **account id doubles as the bearer token** — treat
   it as a secret even though it is an identifier.
2. A directory per repository (`POST /api/fs/{parentId}` with `{"name":"…"}`),
   so the two repositories never collide and pruning can scope to one.
3. Repository secret `BUZZHEAVIER_TOKEN`; repository variables
   `BUZZ_PARENT_ID` and `BUZZ_LOCATION_ID` (a region id from
   `GET /api/locations` — neither is secret).

## When it breaks

| Symptom | Cause |
|---|---|
| `BUZZHEAVIER_TOKEN is unset` | Secret missing, or the run is a fork PR. |
| `BUZZ_PARENT_ID is unset` | Variable missing — or this Forgejo does not support the `vars` context, in which case hardcode it in the workflow `env:`. |
| `release … has no release.json asset` | The release predates this workflow. Re-run the release lane for that tag. |
| `nix copy` fails in refresh | The image was evicted from the cache. Re-run the release lane for that tag. |
| `substituted image does not match the release` | The cache served different bytes than the release recorded. Do not publish; investigate. |
| Download link 404s and no refresh ran | The 8-day window lapsed. Check the refresh workflow's last run. |

## Specific to this repository

**Scope is the host, not the catalogue.** `.#losos-disk-iso` — the
closure-carrying image `flake/disk-images.nix` has been waiting to publish —
stays out. Its exclusion is a CI *build*-budget question as much as a hosting
one (its own comment says it cannot be built in CI), so unblocking it is
separate work that this change merely makes possible.

**The 7-day `upload-artifact` for manual runs is gone.** It existed because a
release asset is permanent and an artifact is not, which was a way to test the
path without spending the attachment budget. That budget is no longer at stake,
and a throwaway tag — the idiom this repository already used for
`v0.0.1-ci-test` — tests the real path instead of a parallel one.

**The image filename now carries the tag.** It was `losos-installer.iso`; it is
`losos-installer-<tag>.iso`. `prune-buzz` matches on the prefix, and two
releases must not collide in one buzzheavier directory.

**Four copies of the Cachix push became one action.** `cachix-push` takes the
installables as an input. The one real difference between the copies survives as
`require-token`: three lanes skip when the secret is absent, because a fork's
pull request gets no secrets and failing there would turn every outside
contribution red for a reason the contributor cannot fix — while the Nextcloud
image lane fails, because there is no point spending ten minutes building an
image with nowhere to put it.

The cache name stays `losos`. `modules/cache.nix:24-28` is explicit that CI and
the appliance must agree on it and that the module is the authoritative copy.

**The ISO is deliberately not pushed to Cachix by the ordinary lanes** — only by
the release lane, so the refresh can substitute it. `devenv.nix:359-363` warns
about spending cache quota on files only you read; a released image is the
exception, because something else has to fetch it twice a week.

## Follow-up, not done here

`CLAUDE.md:26-33` requires that CI commands and `devenv.nix` `scripts.*` be kept
in step by hand. The publishing steps have no `devenv` mirror, because there was
no local publish command to mirror. If the refresh ever moves off Codeberg — and
it arguably should, since this repository is an always-on appliance that already
runs a nightly rebuild, and a systemd timer there would cost the shared runners
nothing — that mirror becomes the natural place for it.
