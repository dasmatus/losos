{-# LANGUAGE OverloadedStrings #-}

{- |
Module      : Main
Description : losos-ctl CLI entrypoint (optparse-applicative).

Subcommands (see backend/schema.json for the wire formats):

  losos-ctl state           --json   current mode + sharing flag
  losos-ctl change  --mode <local|mesh>   apply config + trigger rebuild (async)
  losos-ctl status          --json   rebuild progress (poll)
  losos-ctl settings        --json   current losos.* options from overrides.nix
  losos-ctl apply                    apply Nix code from stdin (rewrites overrides.nix + rebuilds)
  losos-ctl factory-reset             soft factory reset (restore defaults + rebuild)
  losos-ctl rebuild-done <exitcode> <job>  internal: record terminal rebuild status
  losos-ctl install [flags]           the losos auto-installer (was losos-install.sh)

`--json` is accepted as a no-op switch on state/status (the PHP app passes
it); output is always JSON regardless, since that's the only consumer.

The command logic lives in "Lib" (the runtime control backend) and "Installer"
(the auto-installer), both polymorphic over an effect type class; this entry
point runs them in 'IO' (the production interpreter).
-}
module Main (main) where

import Data.Text (Text)
import qualified Data.Text as T
import qualified Data.Text.IO as TIO
import Installer (Options (..), defaultEmitFile, defaultFlakeUrl, defaultFlakeWork, defaultKeyfile, defaultTargetRel, runInstallIO)
import Lib (
    Mode (..),
    cmdApply,
    cmdChange,
    cmdFactoryReset,
    cmdRebuildDone,
    cmdSettings,
    cmdState,
    cmdStatus,
    validateApply,
 )
import Options.Applicative (
    Parser,
    ParserInfo,
    ReadM,
    argument,
    auto,
    command,
    eitherReader,
    execParser,
    flag',
    help,
    helper,
    info,
    long,
    metavar,
    option,
    optional,
    progDesc,
    str,
    subparser,
    switch,
    (<**>),
 )
import System.Environment (lookupEnv)
import System.Exit (exitFailure)
import System.IO (hPutStrLn, stderr)

data Command
    = CmdState
    | CmdChange Mode
    | CmdStatus
    | CmdRebuildDone Int Text
    | CmdSettings
    | CmdApply
    | CmdFactoryReset
    | CmdInstall InstallFlags

-- | The subset of installer flags parsed by optparse. The env-overridable
-- path/url defaults (flake url, work dir, keyfile) are filled in from the
-- environment in 'main', so the CLI surface matches the old @losos-install@
-- script's (@--tpm@, @--drives@, @--no-install@, @--disko-script@,
-- @--emit-target@) bit-for-bit.
data InstallFlags = InstallFlags
    { ifTpm :: Bool
    , ifDrives :: Maybe [Text]
    , ifNoInstall :: Bool
    , ifDiskoScript :: Maybe FilePath
    , ifEmitTarget :: Maybe FilePath
    }

-- | A no-op `--json` switch accepted for contract parity (output is always JSON).
jsonFlag :: Parser ()
jsonFlag = flag' () (long "json" <> help "emit JSON (always on; accepted for parity)")

stateP :: Parser Command
stateP = const CmdState <$> jsonFlag

statusP :: Parser Command
statusP = const CmdStatus <$> jsonFlag

-- | `settings` — current losos.* options parsed from overrides.nix.
settingsP :: Parser Command
settingsP = const CmdSettings <$> jsonFlag

-- | `apply` — read Nix code from stdin, validate, write overrides.nix, rebuild.
-- No options; the payload is the whole stdin.
applyP :: Parser Command
applyP = pure CmdApply

-- | `factory-reset` — soft factory reset. No options; the danger is gated by
-- the locked-down sudoers rule that pins this binary (the PHP admin app
-- confirms in the UI before invoking).
factoryResetP :: Parser Command
factoryResetP = pure CmdFactoryReset

{- | Parse `--mode local|mesh` at parse time, so 'cmdChange' gets a validated
'Mode' and never has to handle an invalid one.
-}
changeP :: Parser Command
changeP =
    CmdChange
        <$> option
            ( eitherReader $ \s -> case s of
                "local" -> Right Local
                "mesh" -> Right Mesh
                _ -> Left "mode must be 'local' or 'mesh'"
            )
            ( long "mode"
                <> metavar "local|mesh"
                <> help "sharing mode: local (private) or mesh (contribute storage)"
            )

rebuildDoneP :: Parser Command
rebuildDoneP =
    CmdRebuildDone
        <$> argument auto (metavar "EXITCODE" <> help "nixos-rebuild exit code")
        <*> argument str (metavar "JOB" <> help "job id returned by `change`")

-- | Comma-separated drive list: @--drives /dev/sdb,/dev/sdc@.
drivesReader :: ReadM [Text]
drivesReader = eitherReader $ \s ->
    let ds = filter (not . T.null) (map T.strip (T.splitOn "," (T.pack s)))
     in if null ds then Left "no drives given" else Right ds

installP :: Parser Command
installP =
    CmdInstall
        <$> ( InstallFlags
                <$> switch (long "tpm" <> help "use TPM2 (interactive passphrase at format time)")
                <*> optional
                    ( option
                        drivesReader
                        ( long "drives"
                            <> metavar "A,/dev/b,..."
                            <> help "override drive auto-detection"
                        )
                    )
                <*> switch (long "no-install" <> help "stop after disko (format+mount), skip nixos-install")
                <*> optional
                    ( option
                        str
                        ( long "disko-script"
                            <> metavar "PATH"
                            <> help "run a prebuilt diskoScript instead of `disko --flake`"
                        )
                    )
                <*> optional
                    ( option
                        str
                        ( long "emit-target"
                            <> metavar "FILE"
                            <> help "write install-target.nix to FILE and exit"
                        )
                    )
            )

commands :: Parser Command
commands =
    subparser $
        mconcat
            [ command "state" (info stateP (progDesc "print current mode + sharing flag"))
            , command "change" (info changeP (progDesc "apply a new mode and trigger a rebuild"))
            , command "status" (info statusP (progDesc "print rebuild progress"))
            , command "settings" (info settingsP (progDesc "print the current losos.* settings from overrides.nix"))
            , command "apply" (info applyP (progDesc "apply Nix config from stdin (rewrites overrides.nix + rebuilds)"))
            , command "factory-reset" (info factoryResetP (progDesc "soft factory reset: restore defaults + rebuild (destructive reset = boot the installer ISO)"))
            , command "rebuild-done" (info rebuildDoneP (progDesc "record terminal rebuild status (internal)"))
            , command "install" (info installP (progDesc "the losos auto-installer (was losos-install.sh)"))
            ]

-- | Read an env var with a fallback (the old script's ${VAR:-default}).
envDef :: String -> Text -> IO Text
envDef name def = maybe def T.pack <$> lookupEnv name

envDefPath :: String -> FilePath -> IO FilePath
envDefPath name def = maybe def id <$> lookupEnv name

-- | Turn the parsed CLI flags into a full 'Installer.Options', filling the
-- env-overridable path/url defaults the way the old @losos-install@ did.
mkInstallOptions :: InstallFlags -> IO Options
mkInstallOptions flags = do
    flakeUrl <- envDef "LOSOS_FLAKE_URL" defaultFlakeUrl
    flakeWork <- envDefPath "LOSOS_FLAKE_WORK" defaultFlakeWork
    keyfile <- envDefPath "LOSOS_KEYFILE" defaultKeyfile
    pure
        Options
            { optTpm = ifTpm flags
            , optDrives = ifDrives flags
            , optNoInstall = ifNoInstall flags
            , optDiskoScript = ifDiskoScript flags
            , optEmitTarget = ifEmitTarget flags
            , optFlakeUrl = flakeUrl
            , optFlakeWork = flakeWork
            , optKeyfile = keyfile
            , optTargetRel = defaultTargetRel
            , optEmitFile = defaultEmitFile
            }

main :: IO ()
main = do
    cmd <- execParser parserInfo
    case cmd of
        CmdState -> cmdState
        CmdChange m -> cmdChange m
        CmdStatus -> cmdStatus
        CmdRebuildDone c j -> cmdRebuildDone c j
        CmdSettings -> cmdSettings
        CmdApply -> do
            input <- TIO.getContents
            case validateApply input of
                Left err -> do
                    hPutStrLn stderr ("losos-ctl apply: " <> T.unpack err)
                    exitFailure
                Right code -> cmdApply code
        CmdFactoryReset -> cmdFactoryReset
        CmdInstall flags -> do
            opts <- mkInstallOptions flags
            runInstallIO opts

parserInfo :: ParserInfo Command
parserInfo = info (commands <**> helper) (progDesc "losos appliance control backend + auto-installer")