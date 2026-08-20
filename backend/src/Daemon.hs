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
import qualified Data.Aeson as A
import qualified Data.Aeson.Key as K
import qualified Data.Aeson.KeyMap as KM
import qualified Data.ByteString as BS
import qualified Data.ByteString.Lazy as BL
import Data.Maybe (fromMaybe, isJust)
import Data.Text (Text)
import qualified Data.Text as T
import qualified Data.Text.Encoding as TE
import qualified Data.Text.IO as TIO
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
import Network.HTTP.Types (Status, status200, status400, status401, status404)
import qualified Network.Wai as Wai
import qualified Network.Wai.Handler.Warp as Warp
import Numeric (showHex)
import System.Directory (createDirectoryIfMissing, doesFileExist)
import System.Environment (lookupEnv)
import System.Exit (die)
import System.FilePath (takeDirectory)
import System.IO (IOMode (ReadMode), hPutStrLn, stderr, withFile)
import System.Posix.Files (setFileMode)
import Text.Read (readMaybe)

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

-- | The loopback HTTP admin API, Bearer-authed. Routes mirror the facade
-- subcommands (/api/<subcommand>); /api/health is deliberately unauthenticated
-- so the public dashboard can show daemon reachability without a token.
-- Listens on 127.0.0.1 only (LOSOS_ADMIN_PORT, default 8082); Nginx proxies
-- /api/* here. The token lives at LOSOS_ADMIN_TOKEN_FILE (default
-- /var/secrets/losos-admin-token), is generated randomly on first start, and
-- is read once here — rotating it means restarting lososd (there is no
-- rotation mechanism; the alternative was a disk read per request).
startHttp :: IO ()
startHttp = do
  port <- maybe 8082 id . (readMaybe =<<) <$> lookupEnv "LOSOS_ADMIN_PORT"
  tokFile <-
    fromMaybe "/var/secrets/losos-admin-token"
      <$> lookupEnv "LOSOS_ADMIN_TOKEN_FILE"
  ensureToken tokFile
  tok <- T.strip <$> TIO.readFile tokFile
  hPutStrLn stderr ("lososd: admin API on 127.0.0.1:" ++ show port)
  void $
    forkIO $
      Warp.runSettings
        (Warp.setHost "127.0.0.1" (Warp.setPort port Warp.defaultSettings))
        (httpApp tok)

-- | Create the admin token file if absent: 32 bytes of /dev/urandom as hex,
-- mode 0600. lososd runs as root; the file lives under persisted /var.
ensureToken :: FilePath -> IO ()
ensureToken path = do
  exists <- doesFileExist path
  unless exists $ do
    createDirectoryIfMissing True (takeDirectory path)
    tok <- newToken
    TIO.writeFile path tok
    setFileMode path 0o600

newToken :: IO Text
newToken = do
  bytes <- withFile "/dev/urandom" ReadMode (`BS.hGet` 32)
  pure (T.concat (map hexByte (BS.unpack bytes)))
  where
    hexByte b =
      let h = showHex b ""
       in T.pack (if length h == 1 then '0' : h else h)

authorized :: Text -> Wai.Request -> Bool
authorized tok req =
  case lookup "Authorization" (Wai.requestHeaders req) of
    Just hdr -> hdr == "Bearer " <> TE.encodeUtf8 tok
    Nothing -> False

jsonResp :: Status -> BL.ByteString -> Wai.Response
jsonResp st = Wai.responseLBS st [("Content-Type", "application/json")]

errJson :: Status -> Text -> Wai.Response
errJson st msg = jsonResp st (A.encode (A.object ["error" A..= msg]))

httpApp :: Text -> Wai.Application
httpApp tok req respond = do
  let authed = authorized tok req
  case (Wai.requestMethod req, Wai.pathInfo req) of
    ("GET", ["api", "health"]) ->
      respond (jsonResp status200 (A.encode (A.object ["ok" A..= True])))
    _ | not authed ->
      respond (errJson status401 "unauthorized")
    ("GET", ["api", "state"]) -> okJson cmdState
    ("GET", ["api", "settings"]) -> okJson cmdSettings
    ("GET", ["api", "status"]) -> okJson cmdStatus
    ("POST", ["api", "change"]) -> do
      body <- Wai.strictRequestBody req
      case A.decode body :: Maybe A.Value of
        Just (A.Object o)
          | Just (A.String m) <- KM.lookup (K.fromText "mode") o ->
              case parseMode m of
                Nothing -> badReq "mode must be 'local' or 'mesh'"
                Just mode -> okJson (cmdChange mode)
        _ -> badReq "body must be JSON: {\"mode\": \"local|mesh\"}"
    ("POST", ["api", "apply"]) -> do
      body <- Wai.strictRequestBody req
      case TE.decodeUtf8' (BL.toStrict body) of
        Left _ -> badReq "body must be UTF-8 Nix code"
        Right txt -> case validateApply txt of
          Left err -> badReq err
          Right code -> okJson (cmdApply code)
    ("POST", ["api", "factory-reset"]) -> okJson cmdFactoryReset
    _ -> respond (errJson status404 "not found")
  where
    okJson io = io >>= respond . jsonResp status200
    badReq = respond . errJson status400

runDaemon :: IO ()
runDaemon = do
  noDbus <- isJust <$> lookupEnv "LOSOS_NO_DBUS"
  unless noDbus startBus
  when noDbus $ hPutStrLn stderr "lososd: LOSOS_NO_DBUS=1 — D-Bus listener disabled"
  startSupervisor
  startHttp
  -- The dbus package's dispatcher threads and warp carry the load; the main
  -- thread just parks.
  forever (threadDelay maxBound)
