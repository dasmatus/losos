-- |
-- Module      : Main (lososd)
-- Description : Entrypoint for the lososd systemd system service.
module Main (main) where

import Daemon (runDaemon)

main :: IO ()
main = runDaemon
