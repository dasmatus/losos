# lososd daemon/facade split + nspawn containers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Split `losos-ctl` into a D-Bus/HTTP systemd daemon (`lososd`) + a thin facade CLI, move the OS settings UI out of Nextcloud to a standalone Nginx-fronted admin endpoint (retiring the Nextcloud plugin), and replace rootless Podman with declarative systemd-nspawn NixOS Containers behind Nginx.

**Architecture:** One cabal package, two executables (`lososd`, `losos-ctl` facade keeping its CLI/JSON contract) sharing the existing `Lib` effect-class core. The daemon owns state.json, exposes methods on the system bus (`org.losos1`, interface `org.losos.Control1`) and a Bearer-authed loopback JSON HTTP API (warp). Rebuilds run as `systemd-run` transient units polled by a daemon watcher thread. Containers run the native NixOS service stacks (`services.nextcloud`, `services.forgejo`) via `containers.*` with `privateNetwork`; Nginx proxies to container IPs and fronts the admin SPA and the Tahoe web UI, so only Nginx holds public ports.

**Tech Stack:** Haskell 2010 (ghc 9.10, `dbus` 1.4.2, `wai`/`warp`, `aeson`, `optparse-applicative`), NixOS modules (`containers`, `services.nginx`), vanilla-JS static SPA (no build step).

Reference spec: `docs/superpowers/specs/2026-08-12-lososd-split-design.md`
(One deviation: bus name is `org.losos1` / interface `org.losos.Control1`, not `org.nixos.*`.)

## Global Constraints

- Modules never read bare `config.X`; they read `config.losos.X`. New options only in `modules/options.nix`.
- `system.stateVersion = "26.11"` everywhere, including container configs.
- The facade CLI surface and its JSON responses (`backend/schema.json`) MUST stay valid: `state`, `change --mode`, `status`, `settings`, `apply` (stdin), `factory-reset`; `--json` accepted as no-op.
- Wire JSON keys for settings change (breaking, sanctioned by spec): `aioApachePort` → `apachePort`, `aioInterfacePort` deleted; `nextcloudMode` enum `"native"|"container"` (`"aio"` accepted as legacy read alias only).
- All paths env-overridable in the Haskell IO code (existing pattern: `LOSOS_STATE_DIR`, `LOSOS_CONFIG`, `LOSOS_OVERRIDES`, `LOSOS_FLAKE`; new: `LOSOS_ADMIN_PORT`, `LOSOS_ADMIN_TOKEN_FILE`, `LOSOS_DBUS_NAME`? — no, bus name is fixed).
- No new Haskell dependencies beyond: `dbus`, `wai`, `warp`, `http-types`. (facade side needs only `dbus`.)
- Haskell tests run via `cd backend && cabal test` or `nix build .#losos-ctl` (`doCheck` runs the suite).
- Tahoes' `pkgs.tahoe-lafs` python312 overlay in `modules/overrides.nix`/`configuration.nix` must not be dropped.
- Every task's commit message ends with `Co-Authored-By: Claude <noreply@anthropic.com>`.

---

### Task 1: Core refactor — commands return JSON, delete rebuild-done, settings rename

**Files:**
- Modify: `backend/src/Lib.hs`
- Modify: `backend/test/Spec.hs`
- Modify: `backend/app/Main.hs` (mechanical: print returned JSON instead of relying on `emit`; drop `rebuild-done` subcommand)
- Modify: `backend/schema.json` (settingsResponse keys; remove `rebuild-done` command)

**Interfaces:**
- Produces (used by Tasks 2–4):
  - `cmdState, cmdStatus, cmdSettings :: Losos m => m BL.ByteString`
  - `cmdChange :: Losos m => Mode -> m BL.ByteString`
  - `cmdApply :: Losos m => Text -> m BL.ByteString` (receives already-validated Nix)
  - `cmdFactoryReset :: Losos m => m BL.ByteString`
  - `data Settings = Settings { setSharingMyStorage :: Bool, setNextcloudMode :: Text, setForgejoMode :: Text, setHostName :: Text, setHttps :: Bool, setGpuEnable :: Bool, setApachePort :: Int }`
  - `parseSettings :: Text -> Settings` — reads keys `sharingMyStorage`, `nextcloud.mode`, `forgejo.mode`, `hostName`, `nextcloud.https`, `gpu.enable`, `nextcloud.apachePort`; falls back to legacy `aio.apachePort` for the port; maps legacy mode value `"aio"` → `"container"`.
  - `spawnUnit :: Text -> m ()` replaces `spawnRebuild` intent but keeps the SAME class method name `spawnRebuild :: Text -> m ()` (now meaning: start rebuild, completion recorded by the supervisor — see Task 3).
  - The `Losos` class loses `emit :: BL.ByteString -> m ()`. `cmdRebuildDone` and the `RebuildState`-from-exitcode logic move to a pure function `unitOutcome :: Int -> Text -> (RebuildState, Text, Int)` (exitCode, logTail) → (state, message, progress) used by the watcher in Task 3.

Concrete edits to `Lib.hs`:

1. Remove `emit` from `class Losos`; commands build and `pure` their JSON (`jsonLn` output minus trailing newline is fine — facade/daemon add framing):
   - `cmdState = do s <- loadState; pure $ A.encode $ object ["mode" .= modeText (stMode s), "sharing" .= stSharing s]`
   - Same shape for the rest; `cmdChange`/`cmdApply`/`cmdFactoryReset` keep their current bodies but `pure` the object instead of `emit`-ing it.
2. Delete `cmdRebuildDone` (spec: rebuild-done subcommand dies). Add:

```haskell
-- | Map a finished rebuild unit's exit code + log tail to the terminal
-- rebuild record fields. Pure so the watcher thread's policy is testable.
unitOutcome :: Int -> Text -> (RebuildState, Int, Text)
unitOutcome code logTail
  | code == 0 = (Done, 100, "rebuild complete")
  | otherwise =
      ( Failed
      , 0
      , let base = "rebuild failed (exit " <> T.pack (show code) <> ")"
         in if T.null logTail then base else base <> ": " <> logTail
      )
```

3. Settings rename (see Interfaces above); update `defaultSettings`, `defaultOverridesNix` (new body below), ToJSON keys, `parseSettings` incl. legacy alias.

```haskell
defaultOverridesNix :: Text
defaultOverridesNix =
  T.unlines
    [ "{ ... }:",
      "",
      "{",
      "  losos.sharingMyStorage = true;",
      "  losos.nextcloud.mode = \"container\";",
      "  losos.forgejo.mode = \"container\";",
      "  losos.hostName = \"mattbox\";",
      "  losos.nextcloud.https = false;",
      "  losos.gpu.enable = true;",
      "  losos.nextcloud.apachePort = 11000;",
      "}"
    ]
```

`parseSettings` port line: `setApachePort = readIntDef (setApachePort defaultSettings) (lookupNix "nextcloud.apachePort" content <|> lookupNix "aio.apachePort" content)` (import `(<|>)` from Control.Applicative). Mode: `readModeDef d = maybe d (\v -> let s = stripQuotes (T.strip v) in if s == "aio" then "container" else s)`.

4. `instance Losos IO`: drop `emit`; keep everything else. (spawnRebuild stays setsid-based until Task 3 swaps its body — keeps this task build green.)
5. `instance Losos TestM`: drop `emit`/`tsOutput`; tests read return values.

**Spec.hs** changes: every `tsOutput <$> runTest …` assertion becomes the returned `BL.ByteString` (e.g. `fst` of a tuple if you run via `runStateT` directly — adjust `runTest` to return `(a, TestState)`):

```haskell
runTest :: TestState -> (forall m. Losos m => m a) -> (a, TestState)
runTest initSt m = runIdentity $ do
  (a, st) <- runStateT (unTestM m) initSt
  pure (a, st)
```

- Delete all `rebuild-done` test cases.
- Update settings tests to the new keys; ADD cases: legacy `losos.aio.apachePort = 12345;` parses to `apachePort=12345` when `nextcloud.apachePort` absent; `losos.nextcloud.mode = "aio";` parses as `"container"`.
- ADD `unitOutcome` tests: `(0,"") -> (Done,100,"rebuild complete")`, `(1,"boom") -> (Failed,0,"rebuild failed (exit 1): boom")`, `(1,"") -> (Failed,0,"rebuild failed (exit 1)")`.

**Main.hs**: drop `CmdRebuildDone`/`rebuildDoneP`; each `cmdX` call sites become `BL.putStr . (<> "\n")` wrappers (or `jsonLn`-style: `BL.putStrLn` on lazy ByteString works — use `BL.putStr bs >> BL.putStr "\n"`). Keep CLI byte-compatible.

**schema.json**: settingsResponse = required keys `["sharingMyStorage","nextcloudMode","forgejoMode","hostName","https","gpuEnable","apachePort"]` with `nextcloudMode.enum = ["native","container"]`; delete the `rebuild-done` command entry; change description text: facade talks D-Bus (transport section added in Task 2).

- [ ] **Step 1: Write the failing tests** — edit `backend/test/Spec.hs` as above (return-value style, new keys, unitOutcome, legacy aliases).
- [ ] **Step 2: Run to verify red** — `cd backend && cabal test` → compile errors/failures (`cmdRebuildDone` gone, `tsOutput` gone).
- [ ] **Step 3: Implement Lib.hs/Main.hs edits** above until green.
- [ ] **Step 4: Re-run** `cd backend && cabal test` → PASS.
- [ ] **Step 5: schema.json** update + `git add backend schema.json && git commit -m "refactor(backend): commands return JSON; drop rebuild-done; settings rename to container/apachePort"`.

---

### Task 2: D-Bus daemon core + facade client

**Files:**
- Create: `backend/src/Daemon.hs`
- Create: `backend/src/Facade.hs`
- Create: `backend/app/DaemonMain.hs`
- Modify: `backend/app/Main.hs` (facade: replace direct `cmdX` calls with Facade calls; keep `install` local)
- Modify: `backend/losos-ctl.cabal` (library deps + executables)
- Modify: `backend/schema.json` (add `dbus` section)
- Test: `backend/test/Spec.hs` (pure pieces only: e.g. `settingsJson` round-trip)

**Interfaces:**
- Consumes: Task 1's `cmd*` (return `BL.ByteString`), `unitOutcome`, `parseSettings`.
- Produces (consumed by Tasks 3, 4, 6):
  - `Daemon.runDaemon :: IO ()` — connects system bus, exports the interface, installs the rebuild supervisor hook, starts the warp API (added in Task 4; for now dbus only + `threadDelay maxBound` loop).
  - `Daemon.busNameText :: IsString a => a` — `"org.losos1"`; `Daemon.objectPath :: ObjectPath` — `"/org/losos1"`; `Daemon.interfaceName :: InterfaceName` — `"org.losos.Control1"`.
  - `Facade.callBackend :: Text -> [Variant] -> IO BL.ByteString` — method name (camelCase: `State|Settings|Change|Apply|FactoryReset|Status`), args; returns the reply's single JSON string body re-encoded as bytes; throws `BackendFailure` (newtype over String, `instance Exception`) on MethodError (facade Main catches, prints stderr, exitFailure).
  - Every method body is a single `s` (string) carrying the JSON document. Server handlers have type `IO (Either Reply Text)`; client decodes the first Variant via `fromVariant :: Variant -> Maybe Text`.

**Daemon.hs** (key code):

```haskell
{-# LANGUAGE OverloadedStrings #-}
module Daemon (runDaemon, busNameText, objectPath', interfaceName) where

import DBus
import DBus.Client
... 

errReply :: T.Text -> Either Reply a
errReply msg = Left (ReplyError (errorName_ "org.losos.Error.Failed") [toVariant msg])

wrap :: IO BL.ByteString -> IO (Either Reply T.Text)
wrap io = Right . TE.decodeUtf8 . BL.toStrict <$> io

hState, hStatus, hSettings :: IO (Either Reply T.Text)
hState = wrap cmdState ...
hChange :: T.Text -> IO (Either Reply T.Text)
hChange modeTxt = case parseMode modeTxt of
  Nothing -> pure (errReply "mode must be 'local' or 'mesh'")
  Just m  -> wrap (cmdChange m)
hApply :: T.Text -> IO (Either Reply T.Text)
hApply body = case validateApply body of
  Left err  -> pure (errReply err)
  Right code -> wrap (cmdApply code)
hFactoryReset = wrap cmdFactoryReset

runDaemon :: IO ()
runDaemon = do
  client <- connectSystem
  _ <- requestName client busName [nameAllowReplacement, nameReplaceExisting, nameDoNotQueue] >>= \r ->
         case r of NamePrimaryOwner -> pure r; _ -> die "lososd: could not acquire org.losos1"
  export client objectPath' defaultInterface
    { interfaceName = interfaceName
    , interfaceMethods =
        [ autoMethod "State" hState
        , autoMethod "Settings" hSettings
        , autoMethod "Change" hChange
        , autoMethod "Apply" hApply
        , autoMethod "FactoryReset" hFactoryReset
        , autoMethod "Status" hStatus
        ]
    }
  startSupervisor client   -- Task 3 wires the real one; Task 2: `pure ()`
  startHttp                -- Task 4 wires the real one; Task 2: `pure ()`
  forever (threadDelay maxBound)
```

(`errorName_ :: String -> ErrorName` exists in `DBus.Internal.Message` re-exported by `DBus`; `wrap` on `cmdX` works because the IO `Losos` instance is imported from `Lib`.)

**Facade.hs**:

```haskell
module Facade (callBackend, BackendFailure(..)) where

callBackend :: Text -> [Variant] -> IO BL.ByteString
callBackend member args = do
  client <- connectSystem
  let mc = (methodCall objectPath' interfaceName (memberName_ memberTxt))
             { methodCallDestination = Just busName, methodCallBody = args }
  res <- call client mc
  case res of
    Left err -> throwIO (BackendFailure (methodErrorMessage err))
    Right ret -> case methodReturnBody ret of
      (v:_) | Just t <- fromVariant v -> pure (BL.fromStrict (TE.encodeUtf8 t))
      _ -> throwIO (BackendFailure "malformed reply from lososd")
```

(`memberName_ :: String -> MemberName`; for Text, convert via `T.unpack`.)

**Main.hs**: `CmdState -> callBackend "State" [] >>= BL.putStr >> putStrLn ""` etc.; `CmdApply` still `TIO.getContents` then `callBackend "Apply" [toVariant input]`; wrap `main` body in `handle (\(BackendFailure msg) -> hPutStrLn stderr msg >> exitFailure)`. `CmdInstall` unchanged (local).

**cabal**: library `build-depends` += `dbus >= 1.4`; new stanza:

```cabal
executable lososd
    import:           warnings
    main-is:          DaemonMain.hs
    hs-source-dirs:   app
    build-depends:    base, losos-ctl, dbus, text, bytestring, aeson
    default-language: Haskell2010
```

(`wai`, `warp`, `http-types` get added to the library in Task 4 — keeping this task's build minimal is fine since unused deps are legal.)

Facade/daemon share constants: define busName/objectPath'/interfaceName in a small `src/Losos/Dbus.hs`? Keep it simple: define them in `Daemon.hs`, `Facade.hs` imports from `Daemon` (library-internal module, exposed so the facade can import: exposed-modules += Daemon, Facade).

- [ ] **Step 1: cabal + skeleton** — cabal edits, `DaemonMain.hs` (`module Main where; main = runDaemon`), stub `Daemon.runDaemon`/`Facade.callBackend` as above (supervisor/http stubs).
- [ ] **Step 2: build** — `cd backend && cabal build all` (deps: add `dbus` rows to cabal first; if cabal's solver can't see dbus in the devshell's ghcWithPackages, note it and rely on `nix build .#losos-ctl` in Task 9; devshell uses ghcWithPackages driven by the cabal file so it should pick it up).
- [ ] **Step 3: rewrite Main.hs to facade calls**; build green; existing tests still pass (`cabal test` — pure core untouched by transport).
- [ ] **Step 4: schema.json** — add top-level `"dbus": { "name": "org.losos1", "path": "/org/losos1", "interface": "org.losos.Control1", "methods": {… each with in/out single-string JSON …} }` documentation block.
- [ ] **Step 5: Commit** — `feat(backend): lososd daemon on system D-Bus + losos-ctl facade client`.

---

### Task 3: Rebuild supervision via systemd-run + watcher thread

**Files:**
- Modify: `backend/src/Lib.hs` (`ioSpawnRebuild` body + new `ioWatchUnit`)
- Modify: `backend/src/Daemon.hs` (`startSupervisor` real body — actually the watcher is spawned per rebuild inside `ioSpawnRebuild`; `startSupervisor = pure ()` can stay or re-attach watchers after daemon restart: see step 4)
- Test: `backend/test/Spec.hs` (already covered by `unitOutcome` tests in Task 1)

**Interfaces:**
- Consumes: `unitOutcome`, `rebuildLogTail` effect.
- Produces: daemon-internal only; no interface changes.

**Lib.hs** IO implementation changes:

```haskell
ioSpawnRebuild :: Text -> IO ()
ioSpawnRebuild job = do
  flake <- flakeRef
  log'  <- rebuildLogPath
  let unit = "losos-rebuild-" <> T.unpack job
  res <- try (P.callProcess "systemd-run"
        [ "--unit=" <> unit, "--collect", "--description=losos rebuild"
        , "--property=StandardOutput=append:" <> log'
        , "--property=StandardError=append:" <> log'
        , "nixos-rebuild", "switch", "--flake", flake ]) :: IO (Either SomeException ())
  case res of
    Left e -> …existing flip-to-Failed logic, message "failed to start rebuild: " <> show e …
    Right () -> void (forkIO (watchUnit (T.pack unit) job))

-- | Poll the transient unit until it leaves the active state, then record the
-- outcome. Poll (not JobRemoved signals): one loop, no extra bus wiring, and
-- re-attachable after a daemon restart.
watchUnit :: Text -> Text -> IO ()
watchUnit unit job = loop
  where
    loop = do
      out <- try (P.readProcess "systemctl" ["show", T.unpack unit, "-p", "ActiveState", "-p", "ExecMainStatus", "--value"] "")
                 :: IO (Either SomeException String)
      case out of
        Left _ -> pure ()            -- unit vanished mid-poll; give up quietly
        Right o -> case lines o of
          (state':ec:_) | state' `elem` ["inactive", "failed"] -> finish (fromMaybe 1 (readMaybe ec))
          _ -> threadDelay 2000000 >> loop
    finish code = do
      s <- ioReadState
      case stRebuild s of
        Just rb | rbJob rb == job -> do
          tl <- ioLogTail
          let (st, pct, msg) = unitOutcome code tl
          ioWriteState s { stRebuild = Just rb { rbState = st, rbProgress = pct, rbMessage = msg } }
        _ -> pure ()
```

Also update `cmdStatus` (Task-1 refactored) in the IO path: when the tracked rebuild is `Building`, include the live log tail as `message` so polling shows nixos-rebuild output. Do this inside `cmdStatus` via a new class method? Simplest: add `liveMessage :: Losos m => Rebuild -> m Text` to the class? NO — keep the class unchanged: in `Lib.hs` make `cmdStatus` read `rebuildLogTail` when `rbState == Building` and replace `rbMessage`:

```haskell
Just rb | rbState rb == Building -> do
  tl <- rebuildLogTail
  let msg = if T.null tl then rbMessage rb else tl
  … emit state/progress/message with msg …
```

TestM's `rebuildLogTail` reads `tsLog` — existing plumbing already supports this; add one test: building + tsLog line → status message equals the log line.

**Step 4 (restart resilience):** `Daemon.startSupervisor` — on daemon start, if `state.json` says `Building`, call `ioWatchUnit`/`watchUnit` for the tracked job's unit (`losos-rebuild-<job>`), so a daemon restart (which happens when nixos-rebuild switches to a new lososd!) re-attaches and records the terminal state instead of dangling. Implement as:

```haskell
startSupervisor :: Client -> IO ()
startSupervisor _ = do
  s <- ioReadState   -- via Lib exports; export ioReadState/watchUnit from Lib
  case stRebuild s of
    Just rb | rbState rb == Building ->
      void (forkIO (watchUnit ("losos-rebuild-" <> rbJob rb) (rbJob rb)))
    _ -> pure ()
```

Note for the comment: nixos-rebuild switch restarts lososd (it's a changed system service) mid-rebuild — the watcher thread running inside the old lososd dies with it; this re-attach is what records done/failed afterward. (The rebuild unit itself survives: it doesn't get restarted by the activation.)

- [ ] **Step 1: tests** — Spec.hs: status-with-live-log case; rerun (red if live-message not yet implemented).
- [ ] **Step 2: implement** Lib.hs changes + `startSupervisor`; `cabal build all && cabal test` green.
- [ ] **Step 3: manual smoke (host, non-destructive)** — not possible without a real NixOS root; defer to Task 9 VM test. Instead unit-compile check only.
- [ ] **Step 4: Commit** — `feat(backend): supervise rebuilds as systemd-run transient units`.

---

### Task 4: Loopback admin HTTP API in lososd

**Files:**
- Modify: `backend/src/Daemon.hs` (`startHttp` real body)
- Modify: `backend/losos-ctl.cabal` (library deps += `wai`, `warp`, `http-types`)
- Modify: `backend/schema.json` (add `http` section documenting endpoints)

**Interfaces:**
- Consumes: `cmd*` (Task 1), env overrides pattern (Global Constraints).
- Produces (consumed by Task 5 SPA, Task 6 nginx):
  - Routes (all JSON request/response bodies identical to facade wire):
    - `GET  /api/state` → stateResponse; `GET /api/settings` → settingsResponse; `GET /api/status` → statusResponse
    - `POST /api/change` body `{"mode":"local|mesh"}` → changeResponse
    - `POST /api/apply` body = raw overrides.nix text (`Content-Type: text/plain` or raw) → changeResponse
    - `POST /api/factory-reset` (empty body) → factoryResetResponse
  - Auth: header `Authorization: Bearer <token>`; 401 body `{"error":"unauthorized"}`. Token file path from `LOSOS_ADMIN_TOKEN_FILE` (default `/var/secrets/losos-admin-token`); daemon (root) creates it with a 64-hex-char random token, mode 0600, if absent. Reading `/dev/urandom` 32 bytes, hex-encode via printf-style fold (no new dep).
  - Port: `LOSOS_ADMIN_PORT` (default `8082`), host pinned `127.0.0.1`.

**Daemon.hs** additions:

```haskell
startHttp :: IO ()
startHttp = do
  port <- read . fromMaybe "8082" <$> lookupEnv "LOSOS_ADMIN_PORT"
  tokFile <- fromMaybe "/var/secrets/losos-admin-token" <$> lookupEnv "LOSOS_ADMIN_TOKEN_FILE"
  ensureToken tokFile
  void $ forkIO $ Warp.runSettings
    (Warp.setHost "127.0.0.1" (Warp.setPort port Warp.defaultSettings)) (app tokFile)

app :: FilePath -> Wai.Application
app tokFile req respond = do
  ok <- authorized tokFile req
  if not ok
    then respond (json status401 (A.encode (object ["error" .= ("unauthorized" :: Text)])))
    else case (requestMethod req, pathInfo req) of
      ("GET",  ["api","state"])    -> okJson =<< cmdState
      ("GET",  ["api","settings"]) -> okJson =<< cmdSettings
      ("GET",  ["api","status"])   -> okJson =<< cmdStatus
      ("POST", ["api","change"])   -> withJsonBody req respond $ \v ->
          case v of
            A.Object o -> case KM.lookup "mode" o … parse … pure json
            _ -> badRequest
      ("POST", ["api","apply"])    -> do
          body <- TE.decodeUtf8 . BL.toStrict <$> Wai.strictRequestBody req
          case validateApply body of
            Left err -> respond (json status400 (A.encode (object ["error" .= err])))
            Right code -> okJson =<< cmdApply code
      ("POST", ["api","factory-reset"]) -> okJson =<< cmdFactoryReset
      _ -> respond (json status404 (A.encode (object ["error" .= ("not found" :: Text)])))
```

where `okJson bs = respond (json status200 bs)`, `json st = Wai.responseLBS st [("Content-Type","application/json")]`, `authorized` compares `requestHeaders` bearer to the (trimmed) token file contents read fresh each request.

- [ ] **Step 1: cabal deps** (`wai warp http-types`) + code; `cabal build all` green.
- [ ] **Step 2: host smoke test (unprivileged paths)** — `LOSOS_STATE_DIR=/tmp/losos-test LOSOS_CONFIG=/dev/null LOSOS_OVERRIDES=/dev/null LOSOS_ADMIN_PORT=18082 LOSOS_ADMIN_TOKEN_FILE=/tmp/losos-test/token cabal run lososd &` then: `curl -s localhost:18082/api/state` → 401; `curl -s -H "Authorization: Bearer $(cat /tmp/losos-test/token)" localhost:18082/api/state` → `{"mode":...}`. THEN kill. (Note: with dbus absent in devshell env, runDaemon requires a system bus — on the dev host there IS a system bus, but requesting `org.losos1` fails without policy; so for the smoke, gate the dbus half: if `requestName` fails, log to stderr and keep HTTP running — add that tolerance ONLY when env `LOSOS_NO_DBUS=1`. Implement: `noDbus <- isJust <$> lookupEnv "LOSOS_NO_DBUS"`.)
- [ ] **Step 3: schema.json http section** + commit — `feat(backend): loopback admin JSON API in lososd (Bearer token)`.

---

### Task 5: Admin SPA (`admin-ui/`) — retire nextcloud-app's UI

**Files:**
- Create: `admin-ui/index.html`, `admin-ui/app.js`, `admin-ui/style.css`
- (packaging in Task 8)

**Interfaces:**
- Consumes: Task 4 endpoints (same-origin `/api/...` via nginx), schema.json.
- Produces: static dir packaged as `losos-admin-ui` (Task 8).

Port `nextcloud-app/js/admin.js` logic (read it before writing):
- `generateNix(values)` producing the overrides body with the NEW keys (`losos.nextcloud.apachePort`, no interfacePort, mode values native/container for both services).
- Token flow: on load, `token = sessionStorage.getItem('losos-token')`; fetch `/api/settings`; on 401 render a single password field ("Admin token"); store + retry.
- Sections: Status card (state + rebuild progress bar, polled every 2s while `building`), Settings form (sharing toggle = mode local/mesh radio, nextcloudMode, forgejoMode, hostName, https, gpuEnable, apachePort), buttons: "Apply settings" (POST /api/apply with generated nix, then switch to progress poll), "Factory reset" (confirm() first).
- Plain fetch + minimal DOM; no framework; keep ~150 lines. style.css: clean dark-on-light card layout (this is the "polished" face; spend effort here: css variables, system font stack, accessible labels).

- [ ] **Step 1: write files**; serve locally `cd admin-ui && python3 -m http.server 8080` for a visual check (api calls will fail — expected; verify layout/token prompt).
- [ ] **Step 2: Commit** — `feat(admin-ui): standalone OS settings SPA`.

---

### Task 6: `modules/daemon.nix` — lososd service, D-Bus policy, admin Nginx vhost

**Files:**
- Create: `modules/daemon.nix`
- Modify: `modules/options.nix` (new options — see below)
- Modify: `modules/services.nix` (**delete** the `security.sudo` block; delete backend from environment.systemPackages there — moves to daemon.nix)
- Modify: `modules/configuration.nix` (nothing expected; verify)
- Modify: `flake.nix` (import daemon.nix into the `install` module list)
- Modify: `modules/defaults.nix` (defaults for new options, following existing style — read it first)

**New options** (`options.nix`):

```nix
admin.enable = lib.mkEnableOption "the standalone losos admin endpoint (Nginx vhost + lososd HTTP API)"; # default true via defaults.nix? declare default = true here.
admin.port = lib.mkOption { type = lib.types.port; default = 8081; description = "Public port of the admin endpoint (Nginx)."; };
admin.apiPort = lib.mkOption { type = lib.types.port; default = 8082; description = "Loopback port lososd serves the JSON API on."; };
admin.tokenFile = lib.mkOption { type = lib.types.path; default = "/var/secrets/losos-admin-token"; description = "Bearer token for the admin API; created by lososd if absent. Persisted via /var."; };
```

(`losos.backend.package` description text updated: now provides `losos-ctl` + `lososd`.)

**daemon.nix**:

```nix
{ pkgs, lib, config, ... }:
let
  enabled = config.losos.backend.package != null;
  pkg = config.losos.backend.package;
  dbusPolicy = pkgs.writeTextDir "share/dbus-1/system.d/org.losos1.conf" ''
    <?xml version="1.0"?>
    <!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"
      "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">
    <busconfig>
      <policy user="root">
        <allow own="org.losos1"/>
        <allow send_destination="org.losos1"/>
      </policy>
      <policy group="losos">
        <allow send_destination="org.losos1"/>
      </policy>
      <policy context="default">
        <deny send_destination="org.losos1"/>
      </policy>
    </busconfig>
  '';
in
{
  config = lib.mkIf enabled {
    users.groups.losos = { };

    environment.systemPackages = [ pkg ];  # facade available to root

    services.dbus.packages = [ dbusPolicy ];

    systemd.services.lososd = {
      description = "losos appliance control daemon (D-Bus + admin HTTP API)";
      wantedBy = [ "multi-user.target" ];
      after = [ "dbus.service" ];
      requires = [ "dbus.service" ];
      environment = {
        LOSOS_ADMIN_PORT = toString config.losos.admin.apiPort;
        LOSOS_ADMIN_TOKEN_FILE = toString config.losos.admin.tokenFile;
      };
      serviceConfig = {
        ExecStart = "${pkg}/bin/lososd";
        Restart = "on-failure";
        RestartSec = "2";
        StateDirectory = "losos";   # /var/lib/losos, persisted via impermanence's /var
      };
    };

    # Admin endpoint (its own Nginx vhost)
    services.nginx = lib.mkIf config.losos.admin.enable {
      enable = true;
      recommendedProxySettings = true;
      virtualHosts."losos-admin" = {
        listen = [ { addr = "0.0.0.0"; port = config.losos.admin.port; } ];
        root = config.losos.admin.ui;
        locations."/api/" = {
          proxyPass = "http://127.0.0.1:${toString config.losos.admin.apiPort}";
        };
      };
    };

    networking.firewall.allowedTCPPorts =
      lib.optional config.losos.admin.enable config.losos.admin.port;
  };
}
```

New option `admin.ui` (type package, default = the `losos-admin-ui` package — wire via flake specialArgs or set in daemon.nix through `config.losos.admin.ui` default `pkgs.callPackage ../admin-ui { }`? admin-ui is plain files: `pkgs.runCommand "losos-admin-ui" {} "mkdir $out; cp ${../admin-ui}/* $out/"` — do exactly that inline as option default via `defaultText`; simplest: make it a normal option with default `(pkgs.runCommand ...)` in options.nix using `lib.mkOption { type = lib.types.package; default = pkgs.... }` — options.nix currently takes only `lib`; change its args to `{ lib, pkgs, ... }`.)

- [ ] **Step 1: options + daemon.nix + services.nix sudo deletion + flake import.**
- [ ] **Step 2: eval check** — `nix eval .#nixosConfigurations.install.config.systemd.services.lososd.serviceConfig.ExecStart` resolves; `nix build .#nixosConfigurations.install.config.system.build.toplevel` (may be long; acceptable).
- [ ] **Step 3: Commit** — `feat(modules): lososd systemd service + dbus policy + admin nginx vhost; remove sudo bridge`.

---

### Task 7: nspawn containers + Nginx consolidation

**Files:**
- Create: `modules/nextcloud-common.nix` (shared native Nextcloud stack attrset)
- Rewrite: `modules/containers.nix`
- Modify: `modules/services.nix` (native nextcloud uses common attrset; tahoe web.port → 3457 + nginx proxy vhost on 3456; nginx.enable moves here unconditionally)
- Modify: `modules/options.nix` (rename `aio.apachePort`→`nextcloud.apachePort`; delete `aio.interfacePort`, `aio.datadir`, `containers.user`, `containers.uid`; `nextcloud.mode` enum native/container, default stays "container"; `gpu.enable` description update)
- Modify: `modules/configuration.nix` (delete `containers` user/group; delete any subuid remnants)
- Modify: `modules/defaults.nix`, `modules/overrides.nix` (new keys matching Task 1 `defaultOverridesNix`)
- Modify: `flake.nix` (import nextcloud-common.nix)

**nextcloud-common.nix**:

```nix
{ pkgs, lib, config, self, ... }:
{
  options.lososInternal.nextcloudStack = lib.mkOption {
    type = lib.types.attrs;
    internal = true;
    description = "The native Nextcloud stack shared by host-native mode and the nspawn container. Single source of truth — no drift.";
  };
  config.lososInternal.nextcloudStack = {
    enable = true;
    hostName = config.losos.nextcloud.hostName;
    https = config.losos.nextcloud.https;
    configureRedis = true;
    package = pkgs.nextcloud34;
    datadir = "/var/lib/nextcloud/data";
    database.createLocally = true;
    config = {
      dbtype = "pgsql";
      adminuser = "notshared";
      adminpassFile = config.losos.nextcloud.adminpassFile;
    };
    extraAppsEnable = true;
    extraApps = with pkgs.nextcloud34Packages.apps; {
      inherit deck tasks notes bookmarks calendar contacts maps polls forms tables
              collectives news mail music memories groupfolders files_automatedtagging
              files_linkeditor files_retention previewgenerator checksum notify_push
              dav_push twofactor_webauthn twofactor_admin guests impersonate unroundedcorners;
    };
    appstoreEnable = true;
  };
}
```

(Copy the extraApps list verbatim from current services.nix; note the `losos` app entry is GONE.)

`self` may be null in non-flake contexts (VM test) — guard: `nextcloud-common.nix` doesn't need `self` anymore (losos app dropped), so drop `self` from its args. services.nix keeps `self` for nothing after Task 8; drop then.

**containers.nix** (full rewrite):

```nix
# NixOS Containers (systemd-nspawn) + the Nginx front router.
{
  pkgs, lib, config, ...
}:
let
  nextcloudContainer = config.losos.nextcloud.mode == "container";
  forgejoContainer = config.losos.forgejo.mode == "container";
  containerActive = nextcloudContainer || forgejoContainer;
  apachePort = toString config.losos.nextcloud.apachePort;  # only used in nginx fallback; containers don't port-map
in
{
  containers.nextcloud = lib.mkIf nextcloudContainer {
    autoStart = true;
    privateNetwork = true;
    hostAddress = "10.231.1.1";
    localAddress = "10.231.1.2";
    bindMounts = {
      "/var/lib/nextcloud" = { hostPath = "/var/lib/nextcloud"; isReadOnly = false; };
      "${toString config.losos.nextcloud.adminpassFile}" = { hostPath = toString config.losos.nextcloud.adminpassFile; isReadOnly = true; };
      "/dev/dri" = lib.mkIf config.losos.gpu.enable { hostPath = "/dev/dri"; isReadOnly = false; };
    };
    config = { lib, ... }: {
      system.stateVersion = "26.11";
      services.nextcloud = config.lososInternal.nextcloudStack;
      networking.firewall.allowedTCPPorts = [ 80 ];
    };
  };

  containers.forgejo = lib.mkIf forgejoContainer {
    autoStart = true;
    privateNetwork = true;
    hostAddress = "10.231.2.1";
    localAddress = "10.231.2.2";
    bindMounts."/var/lib/forgejo" = { hostPath = "/var/lib/forgejo"; isReadOnly = false; };
    config = { ... }: {
      system.stateVersion = "26.11";
      services.forgejo = {
        enable = true;
        lfs.enable = true;
        database.type = "postgres";
        stateDir = "/var/lib/forgejo";
        settings.server = {
          HTTP_PORT = 3000;
          ROOT_URL = "http://${config.losos.hostName}.local:8888/";
        };
        actions.ENABLED = true;
      };
    };
  };

  # GPU device access for the nextcloud container's unit.
  systemd.services."container@nextcloud" = lib.mkIf (nextcloudContainer && config.losos.gpu.enable) {
    serviceConfig.DeviceAllow = [ "char-drm rw" ];
  };

  # ── Nginx vhosts for the containers (proxy to container IPs; no host port
  # mappings, so nothing but Nginx binds a public port).
  services.nginx.virtualHosts = lib.mkMerge [
    (lib.mkIf nextcloudContainer {
      "mattbox.local" = {
        locations."/" = {
          proxyPass = "http://10.231.1.2";
          proxyWebsockets = true;
          extraConfig = ''
            client_max_body_size 0;
            proxy_request_buffering off;
            proxy_read_timeout 86400s;
            proxy_send_timeout 86400s;
          '';
        };
      };
    })
    (lib.mkIf forgejoContainer {
      "forgejo" = {
        listen = [ { addr = "0.0.0.0"; port = 8888; } ];
        locations."/" = { proxyPass = "http://10.231.2.2:3000"; proxyWebsockets = true; };
      };
    })
  ];

  networking.firewall.allowedTCPPorts =
    lib.optional nextcloudContainer 80 ++ lib.optional forgejoContainer 8888;
}
```

(Delete: podman options, containers user refs, linger tmpfiles, user units, aioPort binding. Note the old `mattbox.local` host literal — existing vhost hardcodes it; keep parity for now, it's the committed behaviour.)

**services.nix** edits:
- native nextcloud: `services.nextcloud = lib.mkIf (config.losos.nextcloud.mode == "native") config.lososInternal.nextcloudStack;`
- native forgejo unchanged.
- Tahoe behind Nginx: set `services.tahoe.nodes.shared.web.port = 3457;` and add:

```nix
services.nginx = {
  enable = true;
  recommendedProxySettings = true;
  virtualHosts."tahoe" = {
    listen = [ { addr = "0.0.0.0"; port = 3456; } ];
    locations."/" = { proxyPass = "http://127.0.0.1:3457"; proxyWebsockets = true; };
  };
};
```

(firewall keeps 3456 — now owned by Nginx; add a comment "no service binds a public port except Nginx". Comment in tahoe node config: "loopback-reachable only via firewall policy; Nginx owns :3456".)

- [ ] **Step 1: options rename/delete + overrides.nix + defaults.nix sync.**
- [ ] **Step 2: nextcloud-common.nix + services.nix refactor; eval both modes:** `nix eval .#nixosConfigurations.install.config.services.tahoe.nodes.shared.web.port`
- [ ] **Step 3: containers.nix rewrite;** eval: `nix eval .#nixosConfigurations.install.config.containers.nextcloud.localAddress` → `"10.231.1.2"`.
- [ ] **Step 4: build toplevel** again; commit — `feat(modules): nspawn containers replace rootless podman; nginx fronts tahoe`.

---

### Task 8: Retire nextcloud-app; packaging & docs

**Files:**
- Delete: `nextcloud-app/` (whole tree), plus its flake package
- Modify: `flake/packages.nix` (read first: remove `losos-app`; add `losos-admin-ui` via runCommand — or keep admin-ui packaging inside daemon.nix/options.nix default as designed in Task 6; pick ONE place: options.nix default `pkgs.runCommand`)
- Modify: `flake/devshell.nix` (drop PHP/composer bits + `losos-occ`/`ncd` aliases; keep ghc dev shell)
- Modify: `backend/schema.json` title/description (no longer "as invoked by the losos Nextcloud app" — facade + HTTP + D-Bus)
- Modify: `README.md`, `CLAUDE.md` (rewrite bridge/containers sections: daemon + facade + admin endpoint + nspawn; delete sudoers gotcha; add `losos.admin.*` description)

- [ ] **Step 1: deletions + references sweep** — `grep -rn "losos-app\|nextcloud-app\|BackendService" .` after deletion must only hit docs being updated, then zero.
- [ ] **Step 2: devshell/build green** — `nix build .#losos-ctl`.
- [ ] **Step 3: docs** + commit — `refactor!: retire nextcloud plugin; admin endpoint + daemon/facade + nspawn documented`.

---

### Task 9: VM test + final verification

**Files:**
- Read: `tests/` (existing installer VM test harness — reuse its patterns)
- Create: `tests/admin-vm.nix` — `pkgs.testers.nixosTest`:
  - machine imports the same module set as `install` (options, configuration, impermanence? — the existing installer test shows the pattern; reuse its disko-free variant), with `losos.backend.package = self.packages...losos-ctl`… follow how the existing test wires flake outputs.
  - test script (python): `machine.wait_for_unit("lososd.service")`; `machine.succeed("losos-ctl state --json")` contains `"mode"`; `machine.fail("curl -s localhost:8081/api/state")` 401 path via API port: `machine.succeed("curl -s localhost:8082/api/state | grep unauthorized")`; token flow: `token=$(machine.succeed("cat /var/secrets/losos-admin-token"))`; `curl -H "Authorization: Bearer $token" localhost:8081/api/state` 200 via nginx; `systemctl is-active container@nextcloud` active; `curl -s 127.0.0.1:11000`… no — container reachability: `curl -s http://10.231.1.2` nextcloud status page reachable.
- Modify: `flake.nix` (add `checks.x86_64-linux.admin-vm = …` if the flake has a checks pattern; otherwise expose as a package output. Match how the existing tests/ entry is wired.)
- Also update `CLAUDE.md` Build & develop section with the new check command.

- [ ] **Step 1: write test after studying existing tests/ wiring.**
- [ ] **Step 2: run `nix build .#checks.x86_64-linux.admin-vm` (or the exposed attr).** Long build; expected.
- [ ] **Step 3: full green sweep:** `nix build .#losos-ctl`, `nix build .#nixosConfigurations.install.config.system.build.toplevel`, admin-vm check.
- [ ] **Step 4: Commit** — `test: admin endpoint + container VM test`.

---

## Self-review notes

- Spec coverage: daemon+facade (T1–T4), rebuild supervision (T3), admin endpoint + SPA + retirement (T4–T6, T8), nspawn + nginx front-door incl. Tahoe (T7), VM testing (T9). Facade CLI/schema stability asserted in T1 global constraints. Migration alias for `aio` overrides covered in T1 parseSettings.
- Known risk flagged for implementation: `ghcWithPackages` in the devshell must include `dbus`/`warp`; if the devshell derives its package set from the cabal file (read flake/devshell.nix first) it will; otherwise add explicitly.
- Deviation from spec: D-Bus bus/interface names are `org.losos1` / `org.losos.Control1` (cosmetic; spec said `org.nixos.*`).
