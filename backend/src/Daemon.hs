{-# LANGUAGE OverloadedStrings #-}

-- |
-- Module      : Daemon
-- Description : lososd — the privileged side of the losos control plane.
--
-- lososd is the systemd system service that owns the privileged appliance
-- operations. It fronts the transport-free command core in "Lib" with two
-- listeners:
--
--   1. the system D-Bus: owns the well-known name 'lososBusName' and exports
--      interface 'lososInterface' — one method per CLI subcommand, each
--      carrying a single JSON string in/out;
--   2. a loopback HTTP JSON API (warp), Bearer-authed, that Nginx proxies at
--      \/api/* for the admin UI.
--
-- Authorization for D-Bus is a system-bus policy shipped with the package
-- (root always; group `losos` otherwise). No sudo, no polkit.
--
-- The daemon also supervises rebuilds: the command core's 'ioSpawnRebuild'
-- launches @systemd-run@ transient units and forks the watcher thread that
-- records the terminal state — lososd being the sole writer of state.json.
-- On startup, 'startSupervisor' re-attaches the watcher to an in-flight
-- rebuild (lososd is itself restarted by @nixos-rebuild switch@ mid-rebuild,
-- so without this the state would dangle at "building").
module Daemon
  ( runDaemon,
    lososBusName,
    lososObjectPath,
    lososInterface,
  )
where

import Control.Concurrent (forkIO, threadDelay)
import Control.Monad (forever, unless, void, when)
import qualified Data.ByteString.Lazy as BL
import Data.Maybe (isJust)
import Data.Text (Text)
import qualified Data.Text.Encoding as TE
import DBus
  ( BusName,
    ErrorName,
    InterfaceName,
    ObjectPath,
    busName_,
    errorName_,
    interfaceName_,
    memberName_,
    objectPath_,
    toVariant,
  )
import DBus.Client
  ( Interface (..),
    Reply (..),
    RequestNameReply (..),
    autoMethod,
    connectSystem,
    defaultInterface,
    export,
    nameAllowReplacement,
    nameDoNotQueue,
    nameReplaceExisting,
    requestName,
  )
import Lib
  ( Rebuild (..),
    RebuildState (..),
    State (..),
    cmdApply,
    cmdChange,
    cmdFactoryReset,
    cmdSettings,
    cmdState,
    cmdStatus,
    ioReadState,
    parseMode,
    validateApply,
    watchUnit,
  )
import System.Environment (lookupEnv)
import System.Exit (die)
import System.IO (hPutStrLn, stderr)

-- ──────────────────────────────────────────────────────────────────────────
-- Bus constants (shared with Facade)
-- ──────────────────────────────────────────────────────────────────────────

-- | Well-known system-bus name owned by lososd.
lososBusName :: BusName
lososBusName = busName_ "org.losos1"

-- | The single exported object path.
lososObjectPath :: ObjectPath
lososObjectPath = objectPath_ "/org/losos1"

-- | The control interface ("Control1" — the 1 is the D-Bus convention for a
-- versioned interface name).
lososInterface :: InterfaceName
lososInterface = interfaceName_ "org.losos.Control1"

lososError :: ErrorName
lososError = errorName_ "org.losos1.Error.Failed"

-- ──────────────────────────────────────────────────────────────────────────
-- Method handlers — adapters from the transport-free Lib commands
-- ──────────────────────────────────────────────────────────────────────────

-- | Every method body is a single @s@ carrying the JSON document the
-- corresponding losos-ctl subcommand would print (schema.json).
wrap :: IO BL.ByteString -> IO (Either Reply Text)
wrap io = Right . TE.decodeUtf8 . BL.toStrict <$> io

errReply :: Text -> Either Reply a
errReply msg = Left (ReplyError lososError [toVariant msg])

hState, hStatus, hSettings :: IO (Either Reply Text)
hState = wrap cmdState
hStatus = wrap cmdStatus
hSettings = wrap cmdSettings

hChange :: Text -> IO (Either Reply Text)
hChange modeTxt = case parseMode modeTxt of
  Nothing -> pure (errReply "mode must be 'local' or 'mesh'")
  Just m -> wrap (cmdChange m)

hApply :: Text -> IO (Either Reply Text)
hApply body = case validateApply body of
  Left err -> pure (errReply err)
  Right code -> wrap (cmdApply code)

hFactoryReset :: IO (Either Reply Text)
hFactoryReset = wrap cmdFactoryReset

-- ──────────────────────────────────────────────────────────────────────────
-- Listeners
-- ──────────────────────────────────────────────────────────────────────────

-- | Claim the bus name and export the control interface. Skipped entirely when
-- LOSOS_NO_DBUS=1 (dev/smoke runs on hosts without our system policy).
startBus :: IO ()
startBus = do
  client <- connectSystem
  reply <-
    requestName
      client
      lososBusName
      [nameAllowReplacement, nameReplaceExisting, nameDoNotQueue]
  case reply of
    NamePrimaryOwner -> pure ()
    NameAlreadyOwner -> pure ()
    other ->
      die ("lososd: could not acquire org.losos1 on the system bus (got " ++ show other ++ "); does the dbus policy ship (services.dbus.packages)?")
  export
    client
    lososObjectPath
    defaultInterface
      { interfaceName = lososInterface,
        interfaceMethods =
          [ autoMethod (memberName_ "State") hState,
            autoMethod (memberName_ "Settings") hSettings,
            autoMethod (memberName_ "Change") hChange,
            autoMethod (memberName_ "Apply") hApply,
            autoMethod (memberName_ "FactoryReset") hFactoryReset,
            autoMethod (memberName_ "Status") hStatus
          ]
      }
  hPutStrLn stderr "lososd: org.losos1 exported on the system bus"

-- | Rebuild supervision: re-attach the completion watcher to an in-flight
-- rebuild recorded in state.json. lososd is itself restarted by
-- `nixos-rebuild switch` mid-rebuild (the activation restarts changed
-- services); without this, the rebuild the daemon started would dangle at
-- "building" after the new daemon process comes up.
startSupervisor :: IO ()
startSupervisor = do
  s <- ioReadState
  case stRebuild s of
    Just rb
      | rbState rb == Building -> void (forkIO (watchUnit (rbJob rb)))
      | otherwise -> pure ()
    Nothing -> pure ()

-- | The loopback HTTP admin API. Implemented in Task 4; stubbed until then.
startHttp :: IO ()
startHttp = pure ()

runDaemon :: IO ()
runDaemon = do
  noDbus <- isJust <$> lookupEnv "LOSOS_NO_DBUS"
  unless noDbus startBus
  when noDbus $ hPutStrLn stderr "lososd: LOSOS_NO_DBUS=1 — D-Bus listener disabled"
  startSupervisor
  startHttp
  -- The dbus package's dispatcher threads and warp carry the load; the main
  -- thread just parks.
  void (forkIO (pure ()))
  forever (threadDelay maxBound)
