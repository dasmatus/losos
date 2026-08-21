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
-- Module      : Installer
-- Description : losos-install logic, abstracted over an 'Install' effect class.
--
-- This is the Haskell rewrite of @install/losos-install.sh@. It mirrors the
-- design of "Lib" (the losos-ctl control backend): every side effecting
-- operation is a method of the 'Install' type class, so the same 'planInstall'
-- + 'execute' pipeline runs in 'IO' on the installer ISO and in the pure
-- 'TestM' interpreter under the test suite.
--
-- The two fragile bits of the old bash script are replaced structurally:
--
--   * Drive detection used an @awk@ pipeline over @blkid -o export@ (sensitive
--     to record separators and the human-readable SIZE column). Here it is a
--     single @lsblk --json --bytes@ call parsed by @aeson@, and the filtering
--     (fixed disk, non-removable, >1GB, not mounted anywhere in its subtree)
--     is the pure function 'detectCandidates' — unit-testable with fixtures.
--
--   * Invoking @disko@ \/ @nixos-install@ \/ @git@ by interpolating shell
--     strings is replaced by 'System.Process' argument-list calls, so there is
--     no shell quoting to get wrong.
--
-- The flow is split into a pure planner and a thin executor:
--
--   * 'planInstall' turns parsed CLI 'Options' + the detected block devices
--     into @Either Text [InstallAction]@ — an ordered list of effects, or an
--     error message. This is the part that is unit-tested.
--   * 'execute' walks the action list through the 'Install' class; the 'IO'
--     instance does the real work, the 'TestM' instance records it.
--
-- The CLI surface (@--tpm@, @--drives@, @--no-install@, @--disko-script@,
-- @--emit-target@) and the exact @modules/install-target.nix@ format are
-- preserved bit-for-bit, so the VM test (@tests/install.nix@) drives the binary
-- through the same two seams (@--emit-target@, @--disko-script@) unchanged.
module Installer
  ( -- * Effect type class
    Install (..),
    InstallError (..),
    -- * Interpreters
    TestState (..),
    initTestState,
    runTestM,
    runInstallIO,
    writtenFile,
    -- * Planning (pure)
    BlockDev (..),
    Options (..),
    InstallAction (..),
    LsblkOutput (..),
    detectCandidates,
    renderTarget,
    planInstall,
    execute,
    -- * Defaults (env-overridable in the CLI layer)
    defaultFlakeUrl,
    defaultFlakeWork,
    defaultKeyfile,
    defaultTargetRel,
    defaultEmitFile,
    minBytes,
  )
where

import Control.Exception (Exception, SomeException, displayException, throwIO, try)
import Control.Monad (unless, when)
import Control.Monad.State.Strict
  ( MonadState,
    StateT,
    modify',
    runStateT,
  )
import Data.Aeson (FromJSON (..), ToJSON (..), object, (.:), (.:?), (.!=), (.=))
import qualified Data.Aeson as A
import Data.Aeson.Types (typeMismatch)
import qualified Data.ByteString as BS
import qualified Data.ByteString.Lazy as BL
import Data.Functor.Identity (Identity, runIdentity)
import Data.Maybe (catMaybes)
import Data.Text (Text)
import qualified Data.Text as T
import qualified Data.Text.Encoding as TE
import qualified Data.Text.IO as TIO
import GHC.Generics (Generic)
import Lib (atomicWriteWith)
import System.Directory
  ( Permissions (..),
    copyFile,
    createDirectoryIfMissing,
    doesDirectoryExist,
    doesFileExist,
    emptyPermissions,
    removeDirectoryRecursive,
    removePathForcibly,
    setPermissions,
    withCurrentDirectory,
  )
import System.Exit (ExitCode (..), exitFailure)
import System.FilePath ((</>), takeDirectory)
import System.IO (BufferMode (..), hClose, hPutStrLn, hSetBuffering, openBinaryFile, stderr, stdout, IOMode (ReadMode))
import qualified System.Process as P

-- ──────────────────────────────────────────────────────────────────────────
-- Block-device model (lsblk --json)
-- ──────────────────────────────────────────────────────────────────────────

-- | One node of the @lsblk --json@ tree. We keep the children so a disk can be
-- excluded when any partition in its subtree is mounted (the old script's
-- @findmnt@+@PKNAME@ dance, collapsed into a tree walk).
data BlockDev = BlockDev
  { bdName :: Text,
    bdSize :: Integer,
    bdRm :: Int,
    bdType :: Text,
    bdMountpoints :: [Text],
    bdChildren :: [BlockDev]
  }
  deriving (Eq, Show, Generic)

instance FromJSON BlockDev where
  parseJSON = A.withObject "BlockDev" $ \o ->
    BlockDev
      <$> o .: "name"
      <*> o .: "size"
      <*> (o .:? "rm" >>= parseRm)
      <*> o .:? "type" .!= ""
      <*> parseMountpoints o
      <*> (maybe [] id <$> (o .:? "children"))
    where
      -- lsblk emits "mountpoints": [] (empty) or ["<path>"] or [null] (a
      -- partition with no fs). Older lsblk used the singular "mountpoint"
      -- string; we fall back to it if the plural column is absent. Nulls in
      -- the array mean "no mountpoint here" and are dropped.
      parseMountpoints o = do
        m <- o .:? "mountpoints"
        case m of
          Just xs -> pure (catMaybes xs)
          Nothing -> maybe [] (: []) <$> (o .:? "mountpoint")
      -- The @rm@ (removable) column is emitted as a JSON boolean (@true@ /
      -- @false@) by util-linux >= 2.39, and as an integer (@0@ / @1@) by
      -- older lsblk. The internal model keeps an Int (0 = fixed, 1 =
      -- removable, matching the bash script's @RM==0@ test), so accept
      -- both shapes and normalize. Field absent -> 0 (non-removable).
      parseRm Nothing = pure 0
      parseRm (Just (A.Bool b)) = pure (if b then 1 else 0)
      parseRm (Just (A.Number n)) = pure (if n == 0 then 0 else 1)
      parseRm (Just v) = typeMismatch "rm (Bool or Number)" v

instance ToJSON BlockDev where
  toJSON b =
    object
      [ "name" .= bdName b,
        "size" .= bdSize b,
        "rm" .= bdRm b,
        "type" .= bdType b,
        "mountpoints" .= bdMountpoints b,
        "children" .= bdChildren b
      ]

newtype LsblkOutput = LsblkOutput {lsBlockdevices :: [BlockDev]}

instance FromJSON LsblkOutput where
  parseJSON = A.withObject "LsblkOutput" $ \o -> LsblkOutput <$> o .: "blockdevices"

-- ──────────────────────────────────────────────────────────────────────────
-- Pure planning
-- ──────────────────────────────────────────────────────────────────────────

-- | Minimum disk size to be considered a candidate (1 GB, matching the old
-- script's @size+0 > 1000000000@).
minBytes :: Integer
minBytes = 1000000000

-- | True if this device or any descendant carries a mountpoint — i.e. the disk
-- is in use and must not be repartitioned.
hasMountpoint :: BlockDev -> Bool
hasMountpoint b =
  not (null (bdMountpoints b)) || any hasMountpoint (bdChildren b)

-- | The pure drive filter: whole disks, non-removable, larger than 'minBytes',
-- and not mounted anywhere in their subtree. Names are returned with the
-- @\/dev\/@ prefix the installer writes into @install-target.nix@ (lsblk names
-- are bare, e.g. @sdb@; @blkid@ DEVNAME was already @\/dev\/@-prefixed, so this
-- preserves the old wire format the VM test asserts on).
detectCandidates :: [BlockDev] -> [Text]
detectCandidates = map (("/dev/" <>) . bdName) . filter isCandidate
  where
    isCandidate b =
      bdType b == "disk"
        && bdRm b == 0
        && bdSize b > minBytes
        && not (hasMountpoint b)

-- | Render @modules/install-target.nix@. The format is fixed — the VM test
-- asserts on @losos.targetDrives@ and the @\/dev\/@-prefixed drive names — so
-- this is exact. ASCII comment (the old script's em-dash/non-breaking-hyphen
-- header isn't asserted on and only invites encoding surprises).
renderTarget :: [Text] -> Bool -> Text
renderTarget drives tpm =
  T.unlines
    [ "# Generated by losos-install -- do not hand-edit; re-run losos-install",
      "# to change the target drives or TPM mode. This file is imported by",
      "# flake.nix only when it exists (builtins.pathExists), so it stays out",
      "# of the admin app's modules/overrides.nix (which losos-ctl apply",
      "# rewrites wholesale) and out of any published flake. The installer",
      "# commits it into the local /persist/etc/nixos git repo so the default",
      "# git+file:///etc/nixos auto-upgrade keeps seeing it.",
      "{ ... }:",
      "",
      "{",
      "  losos.targetDrives = [ "
        <> T.intercalate " " (map quote drives)
        <> " ];",
      "  losos.tpm.enable = " <> (if tpm then "true" else "false") <> ";",
      "}"
    ]
  where
    quote d = "\"" <> d <> "\""

-- | CLI options. The drive override, the two test seams (@--disko-script@,
-- @--emit-target@), and the TPM switch are parsed by optparse in the CLI
-- layer; the path/url fields are env-overridable defaults filled in there too.
data Options = Options
  { optTpm :: Bool,
    optDrives :: Maybe [Text],
    optNoInstall :: Bool,
    optDiskoScript :: Maybe FilePath,
    optEmitTarget :: Maybe FilePath,
    optFlakeUrl :: Text,
    optFlakeWork :: FilePath,
    optKeyfile :: FilePath,
    optTargetRel :: FilePath,
    optEmitFile :: FilePath
  }
  deriving (Eq, Show)

-- | A single side effect the installer performs. 'planInstall' produces the
-- list; 'execute' walks it. Keeping this as data (rather than calling IO
-- directly from the planner) is what lets the pure 'TestM' interpreter replay
-- the plan and the unit tests assert on its shape.
data InstallAction
  = ALog Text
  | AWriteTarget FilePath Text
  | AEnsureKeyfile FilePath
  | ARunDiskoScript FilePath
  | ACloneFlake Text FilePath
  | ARunDisko FilePath
  | ABindMount FilePath FilePath
  | ARunNixosInstall FilePath
  | ALayFlake FilePath FilePath
  | ACopyKeyfile FilePath FilePath
  deriving (Eq, Show)

-- | Turn options + detected devices into an ordered action list, or an error.
-- Mirrors the bash script's branch order exactly:
--
--   1. resolve drives (override > auto-detect; empty -> Left)
--   2. log the target drives
--   3. @--emit-target@: write the rendered target to FILE and stop
--   4. (@--tpm@? skip keyfile) ensure the keyfile exists (non-TPM only)
--   5. @--disko-script@: write target to the emit file, run the script, stop
--   6. otherwise: clone flake, drop in the target, run disko, bind
--      \/mnt\/persist\/nix over \/mnt\/nix (the installed system's \/nix is an
--      impermanence bind from \/persist\/nix, so the store must be written
--      through the same bind — otherwise nixos-install fills the \/mnt tmpfs
--      and the first boot finds an empty store); @--no-install@ stops here,
--      else (non-TPM) stage the keyfile at \/mnt\/etc\/keys — the chrooted
--      bootloader install resolves the boot.initrd.secrets source inside
--      \/mnt when it appends the initrd secret — and at \/mnt\/persist\/etc\/keys
--      (the path later rebuilds on the running box read), then nixos-install
--      + lay the flake
planInstall :: Options -> [BlockDev] -> Either Text [InstallAction]
planInstall opts bds = do
  drives <- resolveDrives
  let target = renderTarget drives (optTpm opts)
      logDrives = ALog ("losos-install: target drives: " <> T.intercalate " " drives)
      keyfile = optKeyfile opts
      work = optFlakeWork opts
      targetRel = optTargetRel opts
      emitFile = optEmitFile opts
  case optEmitTarget opts of
    Just f -> Right [logDrives, AWriteTarget f target, ALog ("losos-install: wrote " <> T.pack f)]
    Nothing -> case optDiskoScript opts of
      Just script ->
        Right $
          [logDrives]
            ++ ensureKeyfileIf keyfile (optTpm opts)
            ++ [ AWriteTarget emitFile target,
                 ARunDiskoScript script,
                 ALog "losos-install: format+mount complete (test mode, no install)."
               ]
      Nothing ->
        Right $
          [logDrives]
            ++ ensureKeyfileIf keyfile (optTpm opts)
            ++ [ ACloneFlake (optFlakeUrl opts) work,
                 AWriteTarget (work </> targetRel) target,
                 ARunDisko work
               ]
            ++ installTail keyfile (optTpm opts) (optNoInstall opts) work
  where
    resolveDrives =
      case optDrives opts of
        Just ds
          | not (null ds) -> Right ds
          | otherwise -> Left "--drives given but empty"
        Nothing ->
          let cs = detectCandidates bds
           in if null cs
                then Left "no candidate fixed disks found (whole disks, non-removable, >1GB, not mounted); pass --drives to override"
                else Right cs

ensureKeyfileIf :: FilePath -> Bool -> [InstallAction]
ensureKeyfileIf keyfile tpm
  | tpm = []
  | otherwise = [AEnsureKeyfile keyfile]

installTail :: FilePath -> Bool -> Bool -> FilePath -> [InstallAction]
installTail keyfile tpm noInstall work
  | noInstall =
      bindNix
        ++ [ALog "losos-install: --no-install set; stopping after disko. /mnt is ready."]
  | otherwise =
      bindNix
        ++ ( if tpm
               then []
               else
                 [ ACopyKeyfile keyfile "/mnt/etc/keys/persist-keyfile",
                   ACopyKeyfile keyfile "/mnt/persist/etc/keys/persist-keyfile"
                 ]
           )
        ++ [ ARunNixosInstall work,
             ALayFlake work "/mnt/persist/etc/nixos"
           ]
        ++ doneLogs
  where
    -- The installed system's /nix is an impermanence bind from /persist/nix;
    -- disko mounts only its own devices, so without this bind the store
    -- would land on the throwaway /mnt tmpfs.
    bindNix = [ABindMount "/mnt/persist/nix" "/mnt/nix"]
    -- Post-install guidance, carried over from the bash installer. The TPM
    -- hint is load-bearing: nothing else (README included) tells the user to
    -- enroll the TPM, and until they do, every boot of the headless box
    -- stops at a LUKS passphrase prompt.
    doneLogs =
      ALog "losos-install: done. Remove the install medium and reboot into the installed system."
        : ( if tpm
              then
                [ ALog "losos-install: TPM mode: the passphrase typed at format time is asked on every boot until TPM2 is enrolled. Boot the installed system, then run:",
                  ALog "losos-install:   systemd-cryptenroll --tpm2-device=auto --tpm2-pcrs=0+7 /dev/persist-vg/persist"
                ]
              else []
          )

-- ──────────────────────────────────────────────────────────────────────────
-- The 'Install' effect type class
-- ──────────────────────────────────────────────────────────────────────────

-- | The effects the executor needs. 'IO' does the real thing; 'TestM' records
-- the plan's behaviour for the test suite. There is deliberately no
-- @readBlockDevices@ method: drive enumeration is an 'IO' pre-step feeding the
-- pure planner, not something the pure interpreter should fake through state.
class Monad m => Install m where
  writeFileAtomic :: FilePath -> Text -> m ()
  ensureKeyfile :: FilePath -> m ()
  runDiskoScript :: FilePath -> m ()
  cloneFlake :: Text -> FilePath -> m ()
  runDisko :: FilePath -> m ()
  bindMount :: FilePath -> FilePath -> m ()
  runNixosInstall :: FilePath -> m ()
  layFlake :: FilePath -> FilePath -> m ()
  copyKeyfile :: FilePath -> FilePath -> m ()
  logInfo :: Text -> m ()

-- | Walk an action list through the 'Install' class.
execute :: Install m => [InstallAction] -> m ()
execute = mapM_ step
  where
    step = \case
      ALog t -> logInfo t
      AWriteTarget p t -> writeFileAtomic p t
      AEnsureKeyfile p -> ensureKeyfile p
      ARunDiskoScript p -> runDiskoScript p
      ACloneFlake u d -> cloneFlake u d
      ARunDisko d -> runDisko d
      ABindMount s d -> bindMount s d
      ARunNixosInstall d -> runNixosInstall d
      ALayFlake s d -> layFlake s d
      ACopyKeyfile s d -> copyKeyfile s d

-- ──────────────────────────────────────────────────────────────────────────
-- IO interpreter — the real installer
-- ──────────────────────────────────────────────────────────────────────────

-- | A typed error for install failures (lsblk missing, disko non-zero, etc.).
-- The CLI top level catches this and turns it into a stderr message + non-zero
-- exit, so a failure never leaves a half-installed box without an explanation.
data InstallError = InstallError Text
  deriving (Show)

instance Exception InstallError

instance Install IO where
  writeFileAtomic = ioWriteFileAtomic
  ensureKeyfile = ioEnsureKeyfile
  runDiskoScript = ioRunProcPath
  cloneFlake = ioCloneFlake
  runDisko = ioRunDisko
  bindMount = ioBindMount
  runNixosInstall = ioRunNixosInstall
  layFlake = ioLayFlake
  copyKeyfile = ioCopyKeyfile
  logInfo = TIO.putStrLn

-- | Run an external command, capturing stderr for the error message. Throws
-- 'InstallError' on non-zero exit (the CLI catches it). Argument-list invocation
-- — no shell, no quoting.
ioRunProc :: String -> [String] -> IO ()
ioRunProc exe args = do
  (ec, _out, err) <- P.readProcessWithExitCode exe args ""
  case ec of
    ExitSuccess -> pure ()
    ExitFailure n ->
      throwIO (InstallError (T.pack (exe ++ " " ++ unwords args ++ " failed (exit " ++ show n ++ "):\n" <> err)))

-- | Run a long-lived external command with inherited stdio. disko and
-- nixos-install stream progress for many minutes (and cryptsetup prompts on
-- the terminal in TPM mode) — capturing their output would leave the console
-- frozen for the whole install. On failure only the exit code goes into the
-- error; the process output already went to the terminal.
ioRunProcInherit :: String -> [String] -> IO ()
ioRunProcInherit exe args = do
  (_, _, _, ph) <- P.createProcess (P.proc exe args)
  ec <- P.waitForProcess ph
  case ec of
    ExitSuccess -> pure ()
    ExitFailure n ->
      throwIO (InstallError (T.pack (exe ++ " " ++ unwords args ++ " failed (exit " ++ show n ++ "); see the output above")))

-- | Run a prebuilt diskoScript by path (it is an executable nix-built script).
ioRunProcPath :: FilePath -> IO ()
ioRunProcPath p = ioRunProcInherit p []

-- | disko's combined destroy mode asks "Type 'yes' to continue" on stdin
-- unless --yes-wipe-all-disks is passed. This is the by-design unattended
-- destructive installer, so pass it — without the flag every unattended run
-- reads EOF at the prompt and aborts.
ioRunDisko :: FilePath -> IO ()
ioRunDisko work =
  ioRunProcInherit
    "disko"
    ["--mode", "destroy,format,mount", "--yes-wipe-all-disks", "--flake", work ++ "#install"]

ioRunNixosInstall :: FilePath -> IO ()
ioRunNixosInstall work = ioRunProcInherit "nixos-install" ["--flake", work ++ "#install", "--no-root-passwd"]

-- | Clone the flake into a fresh work dir, then drop the clone's @.git@:
-- nix's git fetcher exposes only *tracked* files to flake evaluation, so the
-- install-target.nix written into the clone afterwards would be invisible to
-- @disko --flake@ \/ @nixos-install --flake@ (flake.nix's builtins.pathExists
-- guard reads false in the store copy) and the build would silently fall
-- back to the default targetDrives \/ TPM mode — i.e. format the wrong disk.
-- Without @.git@ the work dir is a plain path flake and every file in it is
-- visible. ('ioLayFlake' git-inits the \/persist\/etc\/nixos copy from
-- scratch, so nothing downstream needs the clone's git metadata.)
ioCloneFlake :: Text -> FilePath -> IO ()
ioCloneFlake url work = do
  exists <- doesDirectoryExist work
  when exists $ removePathForcibly work
  ioRunProc "git" ["clone", "--depth", "1", T.unpack url, work]
  removePathForcibly (work </> ".git")

-- | Bind-mount @src@ (under the mounted \/mnt\/persist) onto @dst@ (under
-- the \/mnt tmpfs), creating both directories first. Used for \/mnt\/nix:
-- the running system's \/nix is an impermanence bind from \/persist\/nix, so
-- nixos-install must write the store through the same bind or it lands on
-- the throwaway tmpfs and the first boot finds an empty store.
ioBindMount :: FilePath -> FilePath -> IO ()
ioBindMount src dst = do
  createDirectoryIfMissing True src
  createDirectoryIfMissing True dst
  ioRunProc "mount" ["--bind", src, dst]

ioLayFlake :: FilePath -> FilePath -> IO ()
ioLayFlake work dest = do
  createDirectoryIfMissing True (takeDirectory dest)
  exists <- doesDirectoryExist dest
  when exists $ removeDirectoryRecursive dest
  ioRunProc "cp" ["-a", work, dest]
  ioRunProc "chmod" ["-R", "u+w", dest]
  withCurrentDirectory dest $ do
    ioRunProc "git" ["init", "-q"]
    ioRunProc "git" ["add", "-A"]
    ioRunProc "git" ["-c", "user.email=losos@local", "-c", "user.name=losos-install", "commit", "-qm", "losos install"]

-- | Atomic file write for install-target.nix: ensure the parent dir, then
-- Lib.atomicWriteWith (temp sibling + rename).
ioWriteFileAtomic :: FilePath -> Text -> IO ()
ioWriteFileAtomic path text = do
  createDirectoryIfMissing True (takeDirectory path)
  atomicWriteWith TIO.writeFile path text

-- | Owner-only permission sets shared by the keyfile generate and copy paths
-- (0700 dir / 0600 file in spirit; System.Directory has no group/other bits
-- to grant, so empty + owner flags is exactly that).
ownerOnlyDir, ownerOnlyFile :: Permissions
ownerOnlyDir = emptyPermissions {readable = True, writable = True, executable = True}
ownerOnlyFile = emptyPermissions {readable = True, writable = True}

-- | Generate a fresh LUKS keyfile at @path@ unless one already exists (the VM
-- test pre-creates one, so this is a no-op there). 4096 random bytes from
-- @\/dev\/urandom@ — read in Haskell rather than shelling out to @dd@. The
-- parent dir is forced to 0700 and the keyfile to 0600.
ioEnsureKeyfile :: FilePath -> IO ()
ioEnsureKeyfile path = do
  exists <- doesFileExist path
  unless exists $ do
    TIO.putStrLn ("losos-install: generating " <> T.pack path)
    let dir = takeDirectory path
    createDirectoryIfMissing True dir
    setPermissions dir ownerOnlyDir
    h <- openBinaryFile "/dev/urandom" ReadMode
    bytes <- BS.hGet h 4096
    hClose h
    BS.writeFile path bytes
    setPermissions path ownerOnlyFile

-- | Copy the generated keyfile to an install-time destination. Two copies are
-- staged before nixos-install runs: \/mnt\/etc\/keys\/persist-keyfile — the
-- chrooted bootloader install resolves the boot.initrd.secrets source inside
-- \/mnt when append-initrd-secrets bakes the key into the ESP initrd, and
-- fails hard if it is missing — and \/mnt\/persist\/etc\/keys\/persist-keyfile,
-- the bind source every later rebuild on the running box reads.
ioCopyKeyfile :: FilePath -> FilePath -> IO ()
ioCopyKeyfile src dest = do
  let dir = takeDirectory dest
  createDirectoryIfMissing True dir
  setPermissions dir ownerOnlyDir
  copyFile src dest
  setPermissions dest ownerOnlyFile

-- | Enumerate block devices via @lsblk --json --bytes@. Fails with
-- 'InstallError' if lsblk is missing or its output is unparseable.
readBlockDevices :: IO [BlockDev]
readBlockDevices = do
  (ec, out, err) <-
    P.readProcessWithExitCode
      "lsblk"
      ["--json", "--bytes", "-o", "NAME,SIZE,RM,TYPE,MOUNTPOINTS,PKNAME"]
      ""
  case ec of
    ExitFailure n -> throwIO (InstallError (T.pack ("lsblk failed (exit " ++ show n ++ "): " ++ err)))
    ExitSuccess -> case A.eitherDecode (BL.fromStrict (TE.encodeUtf8 (T.pack out))) of
      Left e -> throwIO (InstallError (T.pack ("lsblk parse error: " ++ e ++ " (output: " ++ take 200 out ++ ")")))
      Right parsed -> pure (lsBlockdevices parsed)

-- | The CLI entry: read devices (only when auto-detecting), plan, execute,
-- surface errors as stderr + non-zero exit. Drives the @losos-ctl install@
-- subcommand.
runInstallIO :: Options -> IO ()
runInstallIO opts = do
  -- Line-buffer our own stdout: when it is a pipe (the VM test backdoor, any
  -- scripted caller) GHC block-buffers it, and the installer's progress lines
  -- would otherwise surface only at exit — after the output of the child
  -- processes they announce.
  hSetBuffering stdout LineBuffering
  bds <- case optDrives opts of
    Just _ -> pure []
    Nothing -> readBlockDevices
  case planInstall opts bds of
    Left err -> dieErr ("losos-install: " <> err)
    Right acts -> do
      res <- try (execute acts) :: IO (Either SomeException ())
      case res of
        Left e -> dieErr ("losos-install: " <> T.pack (displayException e))
        Right _ -> pure ()
  where
    dieErr msg = do
      hPutStrLn stderr (T.unpack msg)
      exitFailure

-- ──────────────────────────────────────────────────────────────────────────
-- Defaults (env-overridable from the CLI layer)
-- ──────────────────────────────────────────────────────────────────────────

defaultFlakeUrl :: Text
defaultFlakeUrl = "https://codeberg.org/dasmatus/losos.git"

defaultFlakeWork :: FilePath
defaultFlakeWork = "/tmp/losos-flake"

defaultKeyfile :: FilePath
defaultKeyfile = "/etc/keys/persist-keyfile"

defaultTargetRel :: FilePath
defaultTargetRel = "modules/install-target.nix"

defaultEmitFile :: FilePath
defaultEmitFile = "/tmp/losos-install-target.nix"

-- ──────────────────────────────────────────────────────────────────────────
-- Test interpreter — pure, for the test suite
-- ──────────────────────────────────────────────────────────────────────────

-- | Fake world for tests: every file the plan wrote (path -> content), and
-- flags recording which side effects fired. The planner is the primary test
-- surface; this lets a test also replay a plan end-to-end and assert that,
-- e.g., @--emit-target@ wrote exactly one file and never ran disko.
data TestState = TestState
  { tsFiles :: [(FilePath, Text)],
    tsDiskoRan :: Bool,
    tsDiskoScriptRan :: Maybe FilePath,
    tsBindMounts :: [(FilePath, FilePath)],
    tsNixosInstallRan :: Bool,
    tsCloneRan :: Bool,
    tsLayFlakeRan :: Bool,
    tsKeyfileEnsured :: Maybe FilePath,
    tsKeyfileCopied :: Bool,
    tsKeyfileCopyDest :: Maybe FilePath,
    tsLog :: [Text]
  }
  deriving (Eq, Show)

initTestState :: TestState
initTestState =
  TestState
    { tsFiles = [],
      tsDiskoRan = False,
      tsDiskoScriptRan = Nothing,
      tsBindMounts = [],
      tsNixosInstallRan = False,
      tsCloneRan = False,
      tsLayFlakeRan = False,
      tsKeyfileEnsured = Nothing,
      tsKeyfileCopied = False,
      tsKeyfileCopyDest = Nothing,
      tsLog = []
    }

newtype TestM a = TestM {unTestM :: StateT TestState Identity a}
  deriving newtype (Functor, Applicative, Monad, MonadState TestState)

instance Install TestM where
  writeFileAtomic p t = modify' (\s -> s {tsFiles = (p, t) : tsFiles s})
  ensureKeyfile p = modify' (\s -> s {tsKeyfileEnsured = Just p})
  runDiskoScript p = modify' (\s -> s {tsDiskoScriptRan = Just p, tsDiskoRan = True})
  cloneFlake _ _ = modify' (\s -> s {tsCloneRan = True})
  runDisko _ = modify' (\s -> s {tsDiskoRan = True})
  bindMount src dst = modify' (\s -> s {tsBindMounts = tsBindMounts s ++ [(src, dst)]})
  runNixosInstall _ = modify' (\s -> s {tsNixosInstallRan = True})
  layFlake _ _ = modify' (\s -> s {tsLayFlakeRan = True})
  copyKeyfile src d = modify' (\s -> s {tsKeyfileCopied = True, tsKeyfileCopyDest = Just d, tsFiles = (d, "key:" <> T.pack src) : tsFiles s})
  logInfo t = modify' (\s -> s {tsLog = tsLog s ++ [t]})

-- | Run a plan in the pure interpreter, returning the final fake world.
runTestM :: TestState -> TestM a -> TestState
runTestM st m = runIdentity (snd <$> runStateT (unTestM m) st)

-- | Look up the content a plan wrote to @path@ (last write wins).
writtenFile :: TestState -> FilePath -> Maybe Text
writtenFile st path = lookup path (tsFiles st)