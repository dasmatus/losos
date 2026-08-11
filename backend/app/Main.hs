{-# LANGUAGE OverloadedStrings #-}

-- |
-- Module      : Main
-- Description : losos-ctl CLI entrypoint (optparse-applicative).
--
-- Subcommands (see backend/schema.json for the wire formats):
--
--   losos-ctl state           --json   current mode + sharing flag
--   losos-ctl change  --mode <local|mesh>   apply config + trigger rebuild (async)
--   losos-ctl status          --json   rebuild progress (poll)
--   losos-ctl rebuild-done <exitcode> <job>  internal: record terminal rebuild status
--
-- `--json` is accepted as a no-op switch on state/status (the PHP app passes
-- it); output is always JSON regardless, since that's the only consumer.
--
-- The command logic lives in "Lib" and is polymorphic over the 'Losos' effect
-- type class; this entry point runs it in 'IO' (the production interpreter).
module Main (main) where

import Lib
  ( Mode (..),
    cmdApply,
    cmdChange,
    cmdRebuildDone,
    cmdSettings,
    cmdState,
    cmdStatus,
    validateApply,
  )
import qualified Data.Text.IO as TIO
import Data.Text (Text)
import Options.Applicative
  ( Parser,
    ParserInfo,
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
    progDesc,
    str,
    subparser,
    (<**>),
    )
import System.Exit (exitFailure)
import System.IO (hPutStrLn, stderr)

data Command
  = CmdState
  | CmdChange Mode
  | CmdStatus
  | CmdRebuildDone Int Text
  | CmdSettings
  | CmdApply

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

-- | Parse `--mode local|mesh` at parse time, so 'cmdChange' gets a validated
-- 'Mode' and never has to handle an invalid one.
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

commands :: Parser Command
commands =
  subparser $
    mconcat
      [ command "state" (info stateP (progDesc "print current mode + sharing flag")),
        command "change" (info changeP (progDesc "apply a new mode and trigger a rebuild")),
        command "status" (info statusP (progDesc "print rebuild progress")),
        command "settings" (info settingsP (progDesc "print the current losos.* settings from overrides.nix")),
        command "apply" (info applyP (progDesc "apply Nix config from stdin (rewrites overrides.nix + rebuilds)")),
        command "rebuild-done" (info rebuildDoneP (progDesc "record terminal rebuild status (internal)"))
      ]

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

parserInfo :: ParserInfo Command
parserInfo = info (commands <**> helper) (progDesc "losos appliance control backend")