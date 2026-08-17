{-# LANGUAGE DeriveAnyClass #-}
{-# LANGUAGE DeriveGeneric #-}
{-# LANGUAGE DerivingStrategies #-}
{-# LANGUAGE DerivingVia #-}
{-# LANGUAGE FlexibleInstances #-}
{-# LANGUAGE GeneralizedNewtypeDeriving #-}
{-# LANGUAGE LambdaCase #-}
{-# LANGUAGE MultiParamTypeClasses #-}
{-# LANGUAGE OverloadedStrings #-}
{-# LANGUAGE RankNTypes #-}

-- |
-- Module      : Lib
-- Description : losos appliance control core, abstracted over a 'Losos' effect class.
--
-- The interesting design choice is the 'Losos' type class: every command
-- ('cmdState', 'cmdChange', 'cmdStatus', …) is polymorphic in the
-- interpreter. Production runs in 'IO' (real files, systemd-run rebuilds) from
-- the lososd daemon; the test suite runs in 'TestM', a pure 'StateT' over a
-- record of fake state, so the state-machine logic can be exercised without a
-- real install or a real rebuild.
--
-- Commands /return/ their JSON response (one object, no trailing newline —
-- the caller adds framing). Two call sites consume them:
--
--   * lososd ("Daemon"), exposing them over the system D-Bus and a loopback
--     HTTP JSON API;
--   * the losos-ctl facade ("Facade"), which calls the daemon and prints.
--
-- State lives at $LOSOS_STATE_DIR/state.json. The daemon is the sole writer;
-- rebuild completion is recorded by a watcher thread polling the transient
-- systemd unit (see 'ioSpawnRebuild'), so there is no rebuild-done subcommand
-- and no second writer.
module Lib
  ( -- * Effect type class
    Losos (..),
    -- * Interpreters
    TestState (..),
    initTest,
    runTest,
    -- * Commands (generic over 'Losos')
    cmdState,
    cmdChange,
    cmdStatus,
    cmdSettings,
    cmdApply,
    cmdFactoryReset,
    validateApply,
    -- * Types
    Mode (..),
    modeText,
    parseMode,
    RebuildState (..),
    Rebuild (..),
    State (..),
    Settings (..),
    defaultSettings,
    parseSettings,
    defaultOverridesNix,
    unitOutcome,
    -- * IO helpers (used by the daemon's supervisor)
    ioReadState,
    ioWriteState,
    ioLogTail,
    ioSpawnRebuild,
    watchUnit,
    rebuildLogPath,
  )
where

import Control.Applicative ((<|>))
import Control.Concurrent (forkIO, threadDelay)
import Control.Exception (SomeException, try)
import Control.Monad (void)
import Control.Monad.State.Strict
  ( MonadState,
    StateT,
    gets,
    modify',
    runStateT,
  )
import Data.Aeson (FromJSON (..), ToJSON (..), object, (.:), (.:?), (.!=), (.=))
import qualified Data.Aeson as A
import qualified Data.ByteString.Lazy as BL
import Data.Functor.Identity (Identity, runIdentity)
import Data.List (find)
import Data.Maybe (fromMaybe)
import Data.Text (Text)
import Text.Read (readMaybe)
import qualified Data.Text as T
import qualified Data.Text.IO as TIO
import Data.Time.Clock (getCurrentTime)
import Data.Time.Format (defaultTimeLocale, formatTime)
import GHC.Generics (Generic)
import System.Directory
  ( createDirectoryIfMissing,
    doesFileExist,
    renamePath,
  )
import System.Environment (lookupEnv)
import System.FilePath ((</>))
import qualified System.Process as P
import System.Posix.Process (getProcessID)
import System.IO (stderr)

-- ──────────────────────────────────────────────────────────────────────────
-- Domain types
-- ──────────────────────────────────────────────────────────────────────────

-- | Sharing mode. "local" == private Nextcloud (sharing off); "mesh" ==
-- contributing storage to the Tahoe grid (sharing on). One toggle, one source
-- of truth (losos.sharingMyStorage).
data Mode = Local | Mesh
  deriving (Eq, Show)

modeText :: Mode -> Text
modeText Local = "local"
modeText Mesh = "mesh"

parseMode :: Text -> Maybe Mode
parseMode "local" = Just Local
parseMode "mesh" = Just Mesh
parseMode _ = Nothing

-- | Rebuild lifecycle.
data RebuildState = Idle | Building | Done | Failed
  deriving (Eq, Show)

rebuildStateText :: RebuildState -> Text
rebuildStateText Idle = "idle"
rebuildStateText Building = "building"
rebuildStateText Done = "done"
rebuildStateText Failed = "failed"

instance ToJSON RebuildState where
  toJSON = toJSON . rebuildStateText

instance FromJSON RebuildState where
  parseJSON = A.withText "RebuildState" $ \t -> case t of
    "idle" -> pure Idle
    "building" -> pure Building
    "done" -> pure Done
    "failed" -> pure Failed
    _ -> fail $ "unknown rebuild state: " ++ T.unpack t

-- | A rebuild in progress or recently finished.
data Rebuild = Rebuild
  { rbJob :: Text,
    rbState :: RebuildState,
    rbProgress :: Int,
    rbMessage :: Text
  }
  deriving (Eq, Show, Generic)

instance ToJSON Rebuild where
  toJSON (Rebuild job st pct msg) =
    object ["job" .= job, "state" .= st, "progress" .= pct, "message" .= msg]

instance FromJSON Rebuild where
  parseJSON = A.withObject "Rebuild" $ \o ->
    Rebuild
      <$> o .: "job"
      <*> o .: "state"
      <*> o .:? "progress" .!= 0
      <*> o .:? "message" .!= ""

-- | The full persisted state. `state` returns only mode+sharing (per the
-- contract); `status` returns the rebuild field. Both come from here.
data State = State
  { stMode :: Mode,
    stSharing :: Bool,
    stRebuild :: Maybe Rebuild
  }
  deriving (Eq, Show)

instance ToJSON State where
  toJSON (State mode sharing rebuild) =
    object
      [ "mode" .= modeText mode,
        "sharing" .= sharing,
        "rebuild" .= rebuild
      ]

instance FromJSON State where
  parseJSON = A.withObject "State" $ \o -> do
    modeTxt <- o .: "mode"
    sharing <- o .: "sharing"
    rebuild <- o .:? "rebuild"
    let mode = fromMaybe Local (parseMode modeTxt)
    pure $ State mode sharing rebuild

-- | Default state for a fresh install (no state file yet): Local only, idle.
defaultState :: State
defaultState = State Local False Nothing

-- | Map a finished rebuild unit's exit code + log tail to the terminal
-- rebuild record fields. Pure so the watcher thread's policy is testable.
unitOutcome :: Int -> Text -> (RebuildState, Int, Text)
unitOutcome code logTail
  | code == 0 = (Done, 100, "rebuild complete")
  | otherwise =
      ( Failed,
        0,
        let base = "rebuild failed (exit " <> T.pack (show code) <> ")"
         in if T.null logTail then base else base <> ": " <> logTail
      )

-- ──────────────────────────────────────────────────────────────────────────
-- Settings — the user-tunable losos.* options, as written to overrides.nix
-- ──────────────────────────────────────────────────────────────────────────

-- | The set of losos.* options the admin UI can tune. Stored as Nix
-- assignments in modules/overrides.nix; `settings` parses them back out
-- line-based (the same technique `change` uses for sharingMyStorage) so the UI
-- can show current values without evaluating Nix.
data Settings = Settings
  { setSharingMyStorage :: Bool
  , setNextcloudMode :: Text -- "native" | "container"
  , setForgejoMode :: Text   -- "native" | "container"
  , setHostName :: Text
  , setHttps :: Bool
  , setGpuEnable :: Bool
  , setApachePort :: Int
  , setCfdEnable :: Bool
  }
  deriving (Eq, Show)

-- | Mirrors the option defaults in modules/options.nix + the committed
-- modules/overrides.nix, so a missing/corrupt override file still reports the
-- out-of-box values.
defaultSettings :: Settings
defaultSettings =
  Settings
    { setSharingMyStorage = True,
      setNextcloudMode = "container",
      setForgejoMode = "container",
      setHostName = "mattbox",
      setHttps = False,
      setGpuEnable = True,
      setApachePort = 11000,
      setCfdEnable = False
    }

instance ToJSON Settings where
  toJSON s =
    object
      [ "sharingMyStorage" .= setSharingMyStorage s,
        "nextcloudMode" .= setNextcloudMode s,
        "forgejoMode" .= setForgejoMode s,
        "hostName" .= setHostName s,
        "https" .= setHttps s,
        "gpuEnable" .= setGpuEnable s,
        "apachePort" .= setApachePort s,
        "cfdEnable" .= setCfdEnable s
      ]

-- | The default overrides.nix body, returned by the IO reader when the file is
-- missing so `settings` always reports sensible values. Mirrors
-- modules/overrides.nix exactly.
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
      "  losos.cfd.enable = false;",
      "}"
    ]

-- | Extract the raw value text of `losos.<key> = <value>;` from a Nix module
-- body (line-based). Nothing if no such assignment line exists. The first
-- non-comment line containing both `losos.<key>` and `=` wins.
lookupNix :: Text -> Text -> Maybe Text
lookupNix key content = do
  let needle = "losos." <> key
  line <-
    find
      ( \l ->
          needle `T.isInfixOf` l
            && "=" `T.isInfixOf` l
            && not (T.isPrefixOf "#" (T.strip l))
      )
      (T.lines content)
  let afterEq = T.drop 1 (snd (T.breakOn "=" line))
      val = T.strip (T.dropWhileEnd (== ';') (T.strip afterEq))
  pure val

-- | Parse the full Settings from an overrides.nix body, falling back to
-- `defaultSettings` field-by-field when an assignment is absent. Legacy
-- overrides written by the pre-nspawn admin app are honoured on the read
-- side: `losos.aio.apachePort` feeds the port when `nextcloud.apachePort` is
-- absent, and `nextcloud.mode = "aio"` reads back as "container".
parseSettings :: Text -> Settings
parseSettings content =
  Settings
    { setSharingMyStorage = readBoolDef (setSharingMyStorage defaultSettings) (lookupNix "sharingMyStorage" content),
      setNextcloudMode = readModeDef (setNextcloudMode defaultSettings) (lookupNix "nextcloud.mode" content),
      setForgejoMode = readStrDef (setForgejoMode defaultSettings) (lookupNix "forgejo.mode" content),
      setHostName = readStrDef (setHostName defaultSettings) (lookupNix "hostName" content),
      setHttps = readBoolDef (setHttps defaultSettings) (lookupNix "nextcloud.https" content),
      setGpuEnable = readBoolDef (setGpuEnable defaultSettings) (lookupNix "gpu.enable" content),
      setApachePort =
        readIntDef
          (setApachePort defaultSettings)
          ( lookupNix "nextcloud.apachePort" content
              <|> lookupNix "aio.apachePort" content
          ),
      setCfdEnable = readBoolDef (setCfdEnable defaultSettings) (lookupNix "cfd.enable" content)
    }

readBoolDef :: Bool -> Maybe Text -> Bool
readBoolDef d = \case
  Nothing -> d
  Just v -> case T.strip v of
    "true" -> True
    "false" -> False
    _ -> d

readStrDef :: Text -> Maybe Text -> Text
readStrDef d = \case
  Nothing -> d
  Just v -> stripQuotes (T.strip v)

-- | Like 'readStrDef', but maps the retired "aio" mode value to "container".
readModeDef :: Text -> Maybe Text -> Text
readModeDef d = \case
  Nothing -> d
  Just v ->
    let s = stripQuotes (T.strip v)
     in if s == "aio" then "container" else s

readIntDef :: Int -> Maybe Text -> Int
readIntDef d = \case
  Nothing -> d
  Just v -> case readMaybe (T.unpack (T.strip v)) of
    Just n -> n
    Nothing -> d

stripQuotes :: Text -> Text
stripQuotes s =
  let s' = T.strip s
   in if T.length s' >= 2 && T.head s' == '"' && T.last s' == '"'
        then T.init (T.tail s')
        else s'

-- ──────────────────────────────────────────────────────────────────────────
-- The 'Losos' effect type class
-- ──────────────────────────────────────────────────────────────────────────

-- | The effects a losos command needs. Splitting these out is what makes
-- the commands testable: 'IO' does the real thing (inside lososd), 'TestM'
-- stubs everything.
class Monad m => Losos m where
  -- | Read the persisted state (defaults to 'defaultState' if absent/corrupt).
  loadState :: m State
  -- | Overwrite the persisted state.
  saveState :: State -> m ()
  -- | Rewrite the `losos.sharingMyStorage = <bool>;` line in the flake config.
  rewriteConfig :: Bool -> m ()
  -- | Overwrite the whole overrides.nix body with the Nix code received from
  -- the admin UI (the `apply` command). Atomic on POSIX (temp + rename).
  writeOverrides :: Text -> m ()
  -- | Read the overrides.nix body (defaults to 'defaultOverridesNix' if the
  -- file is missing), so `settings` can parse current values.
  readOverrides :: m Text
  -- | Start the nixos-rebuild for the given job id and arrange for its
  -- completion to be recorded (the IO implementation launches a transient
  -- systemd unit and spawns the watcher thread). The job id lets the watcher
  -- ignore a stale completion from an older, superseded rebuild.
  spawnRebuild :: Text -> m ()
  -- | Last non-empty line of the rebuild log (for failure messages; also the
  -- live progress line while a rebuild is building).
  rebuildLogTail :: m Text
  -- | A fresh, unique job id.
  nextJobId :: m Text

-- ──────────────────────────────────────────────────────────────────────────
-- Commands — generic over 'Losos'
-- ──────────────────────────────────────────────────────────────────────────

-- | `state --json`: current mode + sharing flag. Rebuild is intentionally
-- omitted from this response per the contract (use `status` for that).
cmdState :: Losos m => m BL.ByteString
cmdState = do
  s <- loadState
  pure $
    A.encode $
      object
        [ "mode" .= modeText (stMode s),
          "sharing" .= stSharing s
        ]

-- | `settings --json`: the user-tunable losos.* options parsed out of
-- overrides.nix, for the admin UI's first paint. (Rebuild status is fetched
-- separately via `status`; this is just the config snapshot.)
cmdSettings :: Losos m => m BL.ByteString
cmdSettings = do
  content <- readOverrides
  pure $ A.encode $ A.toJSON $ parseSettings content

-- | Validate the Nix code the admin UI wants to apply. Returns Left errmsg
-- on rejection. Kept minimal on purpose — callers reach the daemon only
-- through D-Bus group ACL or the Bearer-authed loopback API, and
-- nixos-rebuild itself rejects a syntactically broken file — so here we only
-- guard against the obviously empty / off-target cases that would otherwise
-- silently rewrite the config with garbage.
validateApply :: Text -> Either Text Text
validateApply t
  | T.null (T.strip t) = Left "empty nix config"
  | not (T.isInfixOf "losos." t) = Left "nix config must reference losos.* options"
  | not (T.isInfixOf "{" t) = Left "nix config must be a module body (missing '{')"
  | otherwise = Right t

-- | `apply` (payload = Nix code from the admin UI): validate (the daemon/CLI
-- layer), overwrite overrides.nix, mark a rebuild as building, start a
-- supervised nixos-rebuild, return the job document. This command receives
-- already-validated code.
cmdApply :: Losos m => Text -> m BL.ByteString
cmdApply nixCode = do
  writeOverrides nixCode
  job <- nextJobId
  s0 <- loadState
  let rb =
        Rebuild
          { rbJob = job,
            rbState = Building,
            rbProgress = 0,
            rbMessage = "rebuild started"
          }
  saveState s0 {stRebuild = Just rb}
  spawnRebuild job
  pure $ A.encode $ object ["job" .= job]

-- | `change --mode <local|mesh>`: rewrite the flake's sharingMyStorage line,
-- mark a rebuild as building, start a supervised nixos-rebuild, return the
-- job document. The mode is pre-validated by the caller so this never sees an
-- invalid one.
cmdChange :: Losos m => Mode -> m BL.ByteString
cmdChange mode = do
  let sharing = mode == Mesh
  rewriteConfig sharing
  job <- nextJobId
  s0 <- loadState
  let rb = Rebuild
        { rbJob = job,
          rbState = Building,
          rbProgress = 0,
          rbMessage = "rebuild started"
        }
  saveState s0 {stMode = mode, stSharing = sharing, stRebuild = Just rb}
  spawnRebuild job
  pure $ A.encode $ object ["job" .= job]

-- | `factory-reset`: restore the appliance to its out-of-box configuration.
-- Writes the committed 'defaultOverridesNix' back into modules/overrides.nix
-- (undoing any `apply` the admin UI made), resets the persisted state to
-- 'defaultState' (Local, sharing off, idle), and starts a rebuild so the box
-- reverts to the committed defaults. The destructive variant — wiping the
-- disks and reinstalling from scratch — is the installer ISO, which auto-runs
-- `losos-install` as root's login shell; booting that medium is the full
-- factory reset. This command is the soft, non-destructive tier: it only
-- touches lososd's own state + overrides.nix and rebuilds.
cmdFactoryReset :: Losos m => m BL.ByteString
cmdFactoryReset = do
  writeOverrides defaultOverridesNix
  job <- nextJobId
  saveState
    defaultState
      { stRebuild =
          Just
            Rebuild
              { rbJob = job,
                rbState = Building,
                rbProgress = 0,
                rbMessage = "factory reset: rebuild started"
              }
      }
  spawnRebuild job
  pure $ A.encode $ object ["job" .= job, "reset" .= (True :: Bool)]

-- | `status --json`: rebuild progress, polled by the admin UI every ~2s.
-- While a rebuild is Building the message is the *live* last line of the
-- rebuild log (the facade monitors systemd's journal-flushed output), falling
-- back to the static "rebuild started" line before nixos-rebuild writes
-- anything.
cmdStatus :: Losos m => m BL.ByteString
cmdStatus = do
  s <- loadState
  case stRebuild s of
    Nothing ->
      pure $
        A.encode $
          object
            [ "state" .= Idle,
              "progress" .= (0 :: Int),
              "message" .= ("" :: Text)
            ]
    Just rb
      | rbState rb == Building -> do
          tl <- rebuildLogTail
          let msg = if T.null tl then rbMessage rb else tl
          pure $
            A.encode $
              object
                [ "state" .= Building,
                  "progress" .= rbProgress rb,
                  "message" .= msg
                ]
      | otherwise ->
          pure $
            A.encode $
              object
                [ "state" .= rbState rb,
                  "progress" .= rbProgress rb,
                  "message" .= rbMessage rb
                ]

-- ──────────────────────────────────────────────────────────────────────────
-- IO interpreter — the real backend (used by lososd)
-- ──────────────────────────────────────────────────────────────────────────

-- | Run a losos command in 'IO' (the production interpreter).
instance Losos IO where
  loadState = ioReadState
  saveState = ioWriteState
  rewriteConfig = ioRewriteConfig
  writeOverrides = ioWriteOverrides
  readOverrides = ioReadOverrides
  spawnRebuild = ioSpawnRebuild
  rebuildLogTail = ioLogTail
  nextJobId = ioNewJobId

-- All paths env-overridable so the binary is testable without a real install:
stateDir :: IO FilePath
stateDir = fromMaybe "/var/lib/losos" <$> lookupEnv "LOSOS_STATE_DIR"

stateFile :: IO FilePath
stateFile = (</> "state.json") <$> stateDir

rebuildLogPath :: IO FilePath
rebuildLogPath = (</> "rebuild.log") <$> stateDir

configFilePath :: IO FilePath
configFilePath = fromMaybe "/etc/nixos/defaults.nix" <$> lookupEnv "LOSOS_CONFIG"

-- | The overrides.nix the admin UI rewrites via `apply` (and `settings`
-- parses). Defaults to the committed module under the persisted flake; override
-- with LOSOS_OVERRIDES for testing off-box.
overridesFilePath :: IO FilePath
overridesFilePath = fromMaybe "/etc/nixos/modules/overrides.nix" <$> lookupEnv "LOSOS_OVERRIDES"

flakeRef :: IO String
flakeRef = fromMaybe "/etc/nixos#install" <$> lookupEnv "LOSOS_FLAKE"

ioReadState :: IO State
ioReadState = do
  path <- stateFile
  exists <- doesFileExist path
  if not exists
    then pure defaultState
    else do
      bytes <- BL.readFile path
      case A.decode bytes :: Maybe State of
        Just s -> pure s
        -- corrupt state -> treat as fresh rather than crashing the admin UI
        Nothing -> pure defaultState

ioWriteState :: State -> IO ()
ioWriteState s = do
  dir <- stateDir
  createDirectoryIfMissing True dir
  path <- stateFile
  -- Atomic on POSIX: write a temp sibling then rename. A bare truncate-then-
  -- write (BL.writeFile) could leave state.json half-written on power loss or
  -- SIGKILL mid-save; ioReadState would then decode the partial JSON as
  -- Nothing and silently fall back to defaultState (Local, sharing off) — a
  -- mode mismatch vs. the real flake config. rename() is atomic.
  let tmp = path <> ".tmp"
  BL.writeFile tmp (A.encode s)
  renamePath tmp path

-- | Replace the `losos.sharingMyStorage = <bool>;` line in the config file.
-- Line-based (no regex dep): the first line containing `losos.sharingMyStorage`
-- is rewritten wholesale, preserving the 2-space indent defaults.nix uses. If
-- no such line exists, inject one before the final closing brace.
ioRewriteConfig :: Bool -> IO ()
ioRewriteConfig sharing = do
  path <- configFilePath
  exists <- doesFileExist path
  if not exists
    then TIO.hPutStr stderr ("warning: config " <> T.pack path <> " missing; not rewriting\n")
    else do
      contents <- TIO.readFile path
      TIO.writeFile path (T.unlines (injectLine sharing (T.lines contents)))

injectLine :: Bool -> [Text] -> [Text]
injectLine sharing ls =
  let newLine = "  losos.sharingMyStorage = " <> boolText sharing <> ";"
      -- Match the assignment line, not a comment that merely mentions the
      -- option name: require an `=` on the line. (A pure comment would
      -- otherwise be rewritten and the real assignment left dangling.)
      isAssign l = "losos.sharingMyStorage" `T.isInfixOf` l && "=" `T.isInfixOf` l
      (before, rest) = break isAssign ls
  in case rest of
       (_ : after) -> before ++ [newLine] ++ after
       [] -> case reverse ls of
         (last_ : rev) | "}" `T.isInfixOf` last_ -> reverse rev ++ [newLine, last_]
         _ -> ls ++ [newLine]
  where
    boolText True = "true"
    boolText False = "false"

-- | Overwrite overrides.nix atomically (temp sibling + rename), like
-- ioWriteState. A bare truncate-then-write could leave the file half-written
-- on power loss, and the next eval would fail mid-rebuild.
ioWriteOverrides :: Text -> IO ()
ioWriteOverrides nixCode = do
  path <- overridesFilePath
  let tmp = path <> ".tmp"
  TIO.writeFile tmp nixCode
  renamePath tmp path

-- | Read overrides.nix. A missing file (fresh box, or LOSOS_OVERRIDES pointing
-- nowhere) yields the default body so `settings` still reports out-of-box
-- values instead of crashing the admin UI.
ioReadOverrides :: IO Text
ioReadOverrides = do
  path <- overridesFilePath
  exists <- doesFileExist path
  if not exists then pure defaultOverridesNix else TIO.readFile path

ioNewJobId :: IO Text
ioNewJobId = do
  now <- getCurrentTime
  pid <- getProcessID
  let stamp = T.pack $ formatTime defaultTimeLocale "%Y%m%d%H%M%S" now
  pure $ stamp <> "-" <> T.pack (show pid)

-- | The systemd unit name for a rebuild job. Unit names allow digits, '-'
-- and alphanumerics, which the timestamp-pid job ids satisfy.
rebuildUnit :: Text -> String
rebuildUnit job = "losos-rebuild-" <> T.unpack job

-- | Start the rebuild as a transient systemd unit and fork the watcher thread
-- that records its terminal state. Compared to the old setsid+sh hack: the
-- rebuild is a real unit (journal, cgroup, `systemctl status`), and lososd is
-- the sole writer of state.json. No `--collect`: we must be able to read
-- ExecMainStatus after the unit exits; dead units vanish at reboot anyway
-- (the appliance root is tmpfs).
ioSpawnRebuild :: Text -> IO ()
ioSpawnRebuild job = do
  flake <- flakeRef
  log' <- rebuildLogPath
  res <-
    try
      ( P.callProcess
          "systemd-run"
          [ "--unit=" <> rebuildUnit job,
            "--description=losos rebuild " <> T.unpack job,
            "--property=StandardOutput=append:" <> log',
            "--property=StandardError=append:" <> log',
            "nixos-rebuild",
            "switch",
            "--flake",
            flake
          ]
      ) ::
      IO (Either SomeException ())
  case res of
    -- Spawn failed (e.g. systemd-run unavailable): don't leave state.json
    -- stuck in Building forever — flip the tracked rebuild to Failed so the
    -- UI shows it instead of spinning on "building" with no watcher arriving.
    Left e -> do
      s <- ioReadState
      case stRebuild s of
        Just rb ->
          ioWriteState
            s
              { stRebuild =
                  Just
                    rb
                      { rbState = Failed,
                        rbProgress = 0,
                        rbMessage = "failed to start rebuild: " <> T.pack (show e)
                      }
              }
        Nothing -> pure ()
    Right () -> void (forkIO (watchUnit job))

-- | One poll of the rebuild unit.
data PollResult = PollWait | PollUnknown | PollDone Int

-- | Ask systemd for the unit's state. `PollUnknown` covers "unit missing or
-- not started yet" (a brief race right after systemd-run returns) and
-- "vanished entirely" (reboot mid-rebuild) — the watcher waits through a
-- grace window for the former and gives up on the latter.
pollUnit :: Text -> IO PollResult
pollUnit job = do
  res <-
    try
      ( P.readProcess
          "systemctl"
          [ "show",
            rebuildUnit job,
            "--value",
            "-p",
            "ActiveState",
            "-p",
            "ExecMainStatus"
          ]
          ""
      ) ::
      IO (Either SomeException String)
  pure $ case res of
    Left _ -> PollWait -- systemctl hiccup: keep waiting
    Right o -> case lines o of
      (st' : ec : _)
        | st' `elem` ["active", "activating", "deactivating", "reloading"] -> PollWait
        | st' == "failed" -> PollDone (fromMaybe 1 (readMaybe ec))
        | st' == "inactive" ->
            case readMaybe ec of
              Just c -> PollDone c
              Nothing -> PollUnknown
      _ -> PollUnknown

-- | Poll the unit until it finishes, then write the terminal rebuild record —
-- but only if @job@ is still the currently-tracked rebuild (a completion for
-- an older, superseded job is ignored, same rule as the old rebuild-done).
watchUnit :: Text -> IO ()
watchUnit job = go (0 :: Int)
  where
    go unknowns = do
      r <- pollUnit job
      case r of
        PollWait -> threadDelay 2000000 >> go 0
        PollUnknown
          | unknowns < 15 -> threadDelay 2000000 >> go (unknowns + 1)
          | otherwise ->
              finish Failed 0 "rebuild unit vanished (reboot or manual stop mid-rebuild) — system state unknown; apply again to retry"
        PollDone c -> do
          tl <- ioLogTail
          let (st, pct, msg) = unitOutcome c tl
          finish st pct msg
    finish st pct msg = do
      s <- ioReadState
      case stRebuild s of
        Just rb
          | rbJob rb == job ->
              ioWriteState
                s
                  { stRebuild =
                      Just rb {rbState = st, rbProgress = pct, rbMessage = msg}
                  }
        _ -> pure ()

ioLogTail :: IO Text
ioLogTail = do
  path <- rebuildLogPath
  exists <- doesFileExist path
  if not exists
    then pure ""
    else do
      contents <- TIO.readFile path
      let nonEmpty = filter (not . T.null) (map T.strip (T.lines contents))
      pure $ case nonEmpty of
        [] -> ""
        ls -> T.take 240 (last ls)

-- ──────────────────────────────────────────────────────────────────────────
-- Test interpreter — pure, for the test suite
-- ──────────────────────────────────────────────────────────────────────────

-- | Fake world for tests: the current state, the simulated config lines, the
-- simulated rebuild log, a job counter, and whether a rebuild was spawned.
data TestState = TestState
  { tsState :: State,
    tsConfig :: [Text],
    tsLog :: [Text],
    tsJobCounter :: Int,
    tsSpawned :: Bool
  }

-- | A convenient starting world: the default overrides.nix body + default state.
initTest :: TestState
initTest =
  TestState
    { tsState = defaultState,
      tsConfig = T.lines defaultOverridesNix,
      tsLog = [],
      tsJobCounter = 0,
      tsSpawned = False
    }

newtype TestM a = TestM {unTestM :: StateT TestState Identity a}
  deriving newtype (Functor, Applicative, Monad, MonadState TestState)

instance Losos TestM where
  loadState = gets tsState
  saveState s = modify' (\st -> st {tsState = s})
  rewriteConfig sharing = modify' (\st -> st {tsConfig = injectLine sharing (tsConfig st)})
  writeOverrides t = modify' (\st -> st {tsConfig = T.lines t})
  readOverrides = gets (T.unlines . tsConfig)
  spawnRebuild _ = modify' (\st -> st {tsSpawned = True})
  rebuildLogTail = gets (\st -> case filter (not . T.null) (map T.strip (tsLog st)) of
      [] -> ""
      ls -> T.take 240 (last ls))
  nextJobId = do
    n <- gets tsJobCounter
    modify' (\st -> st {tsJobCounter = n + 1})
    pure $ "job-" <> T.pack (show n)

-- | Run a command in the pure interpreter, returning its JSON response and
-- the final fake world.
runTest :: TestState -> (forall m. Losos m => m a) -> (a, TestState)
runTest initSt m = runIdentity (runStateT (unTestM m) initSt)
