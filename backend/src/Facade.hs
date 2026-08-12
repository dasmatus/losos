{-# LANGUAGE OverloadedStrings #-}

-- |
-- Module      : Facade
-- Description : The client half of the losos-ctl facade: relay one CLI
--               subcommand to lososd over the system D-Bus and return the JSON
--               document the daemon produced.
--
-- The daemon answers every method with a single @s@ (string) holding the JSON
-- document from schema.json; a 'MethodError' reply maps to 'BackendFailure'
-- (losos-ctl's Main catches it, prints to stderr, exits non-zero — the same
-- failure shape the old sudo bridge had).
module Facade
  ( callBackend,
    BackendFailure (..),
  )
where

import Control.Exception (Exception, throwIO)
import qualified Data.ByteString.Lazy as BL
import Data.Text (Text)
import qualified Data.Text as T
import qualified Data.Text.Encoding as TE
import Data.Typeable (Typeable)
import DBus
  ( MethodCall (..),
    MethodError,
    MethodReturn (..),
    Variant,
    fromVariant,
    memberName_,
    methodCall,
    methodErrorMessage,
  )
import DBus.Client (call, connectSystem)
import Daemon (lososBusName, lososInterface, lososObjectPath)

-- | Raised when lososd is unreachable or answers with an error reply.
newtype BackendFailure = BackendFailure String
  deriving (Show, Typeable)

instance Exception BackendFailure

-- | Call method @member@ of the lososd control interface with @args@ and
-- return the reply's JSON body. Throws 'BackendFailure' on any failure.
callBackend :: Text -> [Variant] -> IO BL.ByteString
callBackend member args = do
  client <- connectSystem
  let mc =
        (methodCall lososObjectPath lososInterface (memberName_ (T.unpack member)))
          { methodCallDestination = Just lososBusName,
            methodCallBody = args
          }
  res <- call client mc
  case res of
    Left err -> throwIO (BackendFailure (renderMethodError member err))
    Right ret -> case methodReturnBody ret of
      (v : _)
        | Just t <- fromVariant v ->
            pure (BL.fromStrict (TE.encodeUtf8 (t :: Text)))
      _ -> throwIO (BackendFailure "lososd returned a malformed reply (expected a single JSON string)")

renderMethodError :: Text -> MethodError -> String
renderMethodError member err =
  "losos-ctl "
    ++ T.unpack member
    ++ ": lososd call failed — "
    ++ methodErrorMessage err
