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
-- Description : losos-ctl backend logic, abstracted over a 'Losos' effect class.
--
-- The interesting design choice is the 'Losos' type class: every command
-- ('cmdState', 'cmdChange', 'cmdStatus', 'cmdRebuildDone') is polymorphic in
-- the interpreter. Production runs in 'IO' (real files, setsid-spawned
-- nixos-rebuild); the test suite runs in 'TestM', a pure 'StateT' over a
-- record of fake state, so the state-machine logic can be exercised without a
-- real install or a real rebuild.
--
-- State lives at $LOSOS_STATE_DIR/state.json. The PHP app's BackendService
-- shells out to the three public subcommands (state, change, status); the
-- fourth (rebuild-done) is invoked by the detached rebuild shell once it
-- finishes, to record the terminal rebuild status.
module Lib
  ( -- * Effect type class
    Losos (..),
    jsonLn,
    -- * Interpreters
    TestState (..),
    initTest,
    runTest,
    -- * Commands (generic over 'Losos')
    cmdState,
    cmdChange,
    cmdStatus,
    cmdRebuildDone,
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
  )
where

import Control.Exception (SomeException, try)
import Control.Monad (guard, void)
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
import qualified Data.Text.Encoding as TE
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
import System.IO (Handle, stderr)

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

-- | Encode a JSON value with a trailing newline (the wire format is one JSON
-- object per line).
jsonLn :: A.Value -> BL.ByteString
jsonLn v = A.encode v <> BL.singleton 10

-- ──────────────────────────────────────────────────────────────────────────
-- Settings — the user-tunable losos.* options, as written to overrides.nix
-- ──────────────────────────────────────────────────────────────────────────

-- | The set of losos.* options the admin app can tune. Stored as Nix
-- assignments in modules/overrides.nix; `settings` parses them back out
-- line-based (the same technique `change` uses for sharingMyStorage) so the UI
-- can show current values without evaluating Nix.
data Settings = Settings
  { setSharingMyStorage :: Bool
  , setNextcloudMode :: Text -- "native" | "aio"
  , setForgejoMode :: Text   -- "native" | "container"
  , setHostName :: Text
  , setHttps :: Bool
  , setGpuEnable :: Bool
  , setAioApachePort :: Int
  , setAioInterfacePort :: Int
  }
  deriving (Eq, Show)

-- | Mirrors the option defaults in modules/options.nix + the committed
-- modules/overrides.nix, so a missing/corrupt override file still reports the
-- out-of-box values.
defaultSettings :: Settings
defaultSettings =
  Settings
    { setSharingMyStorage = True,
      setNextcloudMode = "aio",
      setForgejoMode = "container",
      setHostName = "mattbox",
      setHttps = False,
      setGpuEnable = True,
      setAioApachePort = 11000,
      setAioInterfacePort = 8000
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
        "aioApachePort" .= setAioApachePort s,
        "aioInterfacePort" .= setAioInterfacePort s
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
      "  losos.nextcloud.mode = \"aio\";",
      "  losos.forgejo.mode = \"container\";",
      "  losos.hostName = \"mattbox\";",
      "  losos.nextcloud.https = false;",
      "  losos.gpu.enable = true;",
      "  losos.aio.apachePort = 11000;",
      "  losos.aio.interfacePort = 8000;",
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
-- `defaultSettings` field-by-field when an assignment is absent.
parseSettings :: Text -> Settings
parseSettings content =
  Settings
    { setSharingMyStorage = readBoolDef (setSharingMyStorage defaultSettings) (lookupNix "sharingMyStorage" content),
      setNextcloudMode = readStrDef (setNextcloudMode defaultSettings) (lookupNix "nextcloud.mode" content),
      setForgejoMode = readStrDef (setForgejoMode defaultSettings) (lookupNix "forgejo.mode" content),
      setHostName = readStrDef (setHostName defaultSettings) (lookupNix "hostName" content),
      setHttps = readBoolDef (setHttps defaultSettings) (lookupNix "nextcloud.https" content),
      setGpuEnable = readBoolDef (setGpuEnable defaultSettings) (lookupNix "gpu.enable" content),
      setAioApachePort = readIntDef (setAioApachePort defaultSettings) (lookupNix "aio.apachePort" content),
      setAioInterfacePort = readIntDef (setAioInterfacePort defaultSettings) (lookupNix "aio.interfacePort" content)
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

-- | The effects a losos-ctl command needs. Splitting these out is what makes
-- the commands testable: 'IO' does the real thing, 'TestM' stubs everything.
class Monad m => Losos m where
  -- | Read the persisted state (defaults to 'defaultState' if absent/corrupt).
  loadState :: m State
  -- | Overwrite the persisted state.
  saveState :: State -> m ()
  -- | Rewrite the `losos.sharingMyStorage = <bool>;` line in the flake config.
  rewriteConfig :: Bool -> m ()
  -- | Overwrite the whole overrides.nix body with the Nix code received from
  -- the admin app (the `apply` command). Atomic on POSIX (temp + rename).
  writeOverrides :: Text -> m ()
  -- | Read the overrides.nix body (defaults to 'defaultOverridesNix' if the
  -- file is missing), so `settings` can parse current values.
  readOverrides :: m Text
  -- | Spawn the detached nixos-rebuild + watcher for the given job id. The job
  -- id is threaded into the watcher's `rebuild-done <ec> <job>` call so a stale
  -- completion (from an older, superseded rebuild) can be detected and ignored
  -- rather than clobbering a newer change's state.
  spawnRebuild :: Text -> m ()
  -- | Last non-empty line of the rebuild log (for failure messages).
  rebuildLogTail :: m Text
  -- | A fresh, unique job id.
  nextJobId :: m Text
  -- | Emit one JSON object (already newline-terminated) to the caller.
  emit :: BL.ByteString -> m ()

-- ──────────────────────────────────────────────────────────────────────────
-- Commands — generic over 'Losos'
-- ──────────────────────────────────────────────────────────────────────────

-- | `state --json`: current mode + sharing flag. Rebuild is intentionally
-- omitted from this response per the contract (use `status` for that).
cmdState :: Losos m => m ()
cmdState = do
  s <- loadState
  emit $
    jsonLn $
      object
        [ "mode" .= modeText (stMode s),
          "sharing" .= stSharing s
        ]

-- | `settings --json`: the user-tunable losos.* options parsed out of
-- overrides.nix, for the admin app's first paint. (Rebuild status is fetched
-- separately via `status`; this is just the config snapshot.)
cmdSettings :: Losos m => m ()
cmdSettings = do
  content <- readOverrides
  let s = parseSettings content
  emit $ jsonLn $ A.toJSON s

-- | Validate the Nix code the admin app wants to apply. Returns Left errmsg
-- on rejection. Kept minimal on purpose — losos-ctl is invoked through a
-- sudoers rule pinned to this binary, and nixos-rebuild itself will reject a
-- syntactically broken file — so here we only guard against the obviously
-- empty / off-target cases that would otherwise silently rewrite the config
-- with garbage.
validateApply :: Text -> Either Text Text
validateApply t
  | T.null (T.strip t) = Left "empty nix config"
  | not (T.isInfixOf "losos." t) = Left "nix config must reference losos.* options"
  | not (T.isInfixOf "{" t) = Left "nix config must be a module body (missing '{')"
  | otherwise = Right t

-- | `apply` (stdin = Nix code from the admin app): validate, overwrite
-- overrides.nix, mark a rebuild as building, spawn a detached nixos-rebuild,
-- emit the job id. The CLI (Main) validates and exits non-zero on Left so the
-- PHP layer raises BackendException; this command receives already-validated
-- code.
cmdApply :: Losos m => Text -> m ()
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
  emit $ jsonLn $ object ["job" .= job]

-- | `change --mode <local|mesh>`: rewrite the flake's sharingMyStorage line,
-- mark a rebuild as building, spawn a detached nixos-rebuild, emit the job id.
-- The mode is pre-validated by the CLI (optparse) so this never sees an
-- invalid one.
cmdChange :: Losos m => Mode -> m ()
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
  emit $ jsonLn $ object ["job" .= job]

-- | `factory-reset`: restore the appliance to its out-of-box configuration.
-- Writes the committed 'defaultOverridesNix' back into modules/overrides.nix
-- (undoing any `apply` the admin app made), resets the persisted state to
-- 'defaultState' (Local, sharing off, idle), and spawns a rebuild so the box
-- reverts to the committed defaults. The destructive variant — wiping the
-- disks and reinstalling from scratch — is the installer ISO, which now
-- auto-runs `losos-install` as root's login shell; booting that medium is the
-- full factory reset. This command is the soft, non-destructive tier: it only
-- touches losos-ctl's own state + overrides.nix and rebuilds.
cmdFactoryReset :: Losos m => m ()
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
  emit $ jsonLn $ object ["job" .= job, "reset" .= (True :: Bool)]

-- | `status --json`: rebuild progress, polled by the PHP app every ~2s.
cmdStatus :: Losos m => m ()
cmdStatus = do
  s <- loadState
  case stRebuild s of
    Nothing ->
      emit $
        jsonLn $
          object
            [ "state" .= Idle,
              "progress" .= (0 :: Int),
              "message" .= ("" :: Text)
            ]
    Just rb ->
      emit $
        jsonLn $
          object
            [ "state" .= rbState rb,
              "progress" .= rbProgress rb,
              "message" .= rbMessage rb
            ]

-- | `rebuild-done <exitcode> <job>`: invoked by the detached rebuild shell
-- when nixos-rebuild finishes, to record success/failure. Internal; not part
-- of the public contract. The `job` argument is the id `change` returned; it
-- must match the currently-tracked rebuild's id, so a stale completion from an
-- older, superseded rebuild is ignored instead of clobbering a newer change's
-- state with the wrong terminal status + log tail.
cmdRebuildDone :: Losos m => Int -> Text -> m ()
cmdRebuildDone code job = do
  s <- loadState
  case stRebuild s of
    -- No rebuild tracked: a late rebuild-done for state that was reset/lost.
    -- Don't fabricate a rebuild record.
    Nothing -> pure ()
    Just rb
      -- Stale completion: the rebuild that finished is not the one currently
      -- tracked (a newer `change` superseded it). Ignore it.
      | rbJob rb /= job -> pure ()
      | otherwise -> do
          let (st, baseMsg) =
                if code == 0
                  then (Done, "rebuild complete")
                  else (Failed, "rebuild failed (exit " <> T.pack (show code) <> ")")
          msg <-
            if code == 0
              then pure baseMsg
              else do
                t <- rebuildLogTail
                pure $ if T.null t then baseMsg else baseMsg <> ": " <> t
          saveState
            s
              { stRebuild =
                  Just
                    rb
                      { rbState = st,
                        rbProgress = if code == 0 then 100 else rbProgress rb,
                        rbMessage = msg
                      }
              }

-- ──────────────────────────────────────────────────────────────────────────
-- IO interpreter — the real backend
-- ──────────────────────────────────────────────────────────────────────────

-- | Run a losos-ctl command in 'IO' (the production interpreter).
instance Losos IO where
  loadState = ioReadState
  saveState = ioWriteState
  rewriteConfig = ioRewriteConfig
  writeOverrides = ioWriteOverrides
  readOverrides = ioReadOverrides
  spawnRebuild = ioSpawnRebuild
  rebuildLogTail = ioLogTail
  nextJobId = ioNewJobId
  emit = BL.putStr

-- All paths env-overridable so the binary is testable without a real install:
stateDir :: IO FilePath
stateDir = fromMaybe "/var/lib/losos" <$> lookupEnv "LOSOS_STATE_DIR"

stateFile :: IO FilePath
stateFile = (</> "state.json") <$> stateDir

rebuildLogPath :: IO FilePath
rebuildLogPath = (</> "rebuild.log") <$> stateDir

configFilePath :: IO FilePath
configFilePath = fromMaybe "/etc/nixos/defaults.nix" <$> lookupEnv "LOSOS_CONFIG"

-- | The overrides.nix the admin app rewrites via `apply` (and `settings`
-- parses). Defaults to the committed module under the persisted flake; override
-- with LOSOS_OVERRIDES for testing off-box.
overridesFilePath :: IO FilePath
overridesFilePath = fromMaybe "/etc/nixos/modules/overrides.nix" <$> lookupEnv "LOSOS_OVERRIDES"

flakeRef :: IO String
flakeRef = fromMaybe "/etc/nixos#install" <$> lookupEnv "LOSOS_FLAKE"

ctlBin :: IO String
ctlBin = fromMaybe "/run/current-system/sw/bin/losos-ctl" <$> lookupEnv "LOSOS_CTL_BIN"

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
        -- corrupt state -> treat as fresh rather than crashing the web UI
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

-- | Spawn the rebuild detached: `setsid -f sh -c 'nixos-rebuild … > log 2>&1;
-- ec=$?; <ctl> rebuild-done $ec <job>'`. setsid gives the child a new session
-- so it survives the sudo/PHP process tree ending; -f forks so setsid returns
-- immediately. The shell runs nixos-rebuild, then re-enters this binary with
-- the exit code AND the job id to record the terminal status — no second daemon
-- needed. The job id lets `cmdRebuildDone` ignore stale completions.
ioSpawnRebuild :: Text -> IO ()
ioSpawnRebuild job = do
  flake <- flakeRef
  log' <- rebuildLogPath
  bin <- ctlBin
  let cmd =
        unwords
          [ "nixos-rebuild",
            "switch",
            "--flake",
            shellQuote flake,
            ">",
            shellQuote log',
            "2>&1",
            ";",
            "ec=$?",
            ";",
            shellQuote bin,
            "rebuild-done",
            "$ec",
            shellQuote (T.unpack job)
          ]
  -- create_group detaches the child process group from ours; setsid -f makes
  -- it a session leader so it outlives us. We don't wait for it.
  let cp =
        (P.proc "setsid" ["-f", "sh", "-c", cmd])
          { P.create_group = True,
            P.std_in = P.NoStream,
            P.std_out = P.NoStream,
            P.std_err = P.NoStream
          }
  res <-
    try (P.createProcess cp) ::
      IO (Either SomeException (Maybe Handle, Maybe Handle, Maybe Handle, P.ProcessHandle))
  case res of
    -- Spawn failed (e.g. setsid missing): don't leave state.json stuck in
    -- Building forever — flip the tracked rebuild to Failed so the UI shows
    -- it instead of spinning on "building" with no watcher ever arriving.
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
    Right _ -> pure ()

-- | Minimal POSIX shell quoting: wrap in single quotes, escape embedded ones.
shellQuote :: String -> String
shellQuote s = "'" ++ concatMap (\c -> if c == '\'' then "'\\''" else [c]) s ++ "'"

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
-- simulated rebuild log, a job counter, whether a rebuild was spawned, and the
-- JSON the commands emitted.
data TestState = TestState
  { tsState :: State,
    tsConfig :: [Text],
    tsLog :: [Text],
    tsJobCounter :: Int,
    tsSpawned :: Bool,
    tsOutput :: [Text]
  }

-- | A convenient starting world: the default overrides.nix body + default state.
initTest :: TestState
initTest =
  TestState
    { tsState = defaultState,
      tsConfig = T.lines defaultOverridesNix,
      tsLog = [],
      tsJobCounter = 0,
      tsSpawned = False,
      tsOutput = []
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
  emit bs = modify' (\st -> st {tsOutput = tsOutput st <> [TE.decodeUtf8 (BL.toStrict bs)]})

-- | Run a command in the pure interpreter, returning the final fake world.
runTest :: TestState -> (forall m. Losos m => m a) -> TestState
runTest initSt m = runIdentity (snd <$> runStateT (unTestM m) initSt)