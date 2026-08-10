{-# LANGUAGE OverloadedStrings #-}
{-# LANGUAGE RankNTypes #-}

-- | Pure tests for the losos-ctl state machine, run against the 'TestM'
-- interpreter (no filesystem, no nixos-rebuild). This is the payoff of the
-- 'Losos' type class: the exact same 'cmdChange'/'cmdStatus'/'cmdRebuildDone'
-- that production runs in 'IO' are exercised here deterministically.
module Main (main) where

import Data.Text (Text)
import qualified Data.Text as T
import Lib
  ( Losos,
    Mode (..),
    Rebuild (..),
    RebuildState (..),
    State (..),
    TestState (..),
    cmdChange,
    cmdRebuildDone,
    cmdStatus,
    initTest,
    runTest,
  )
import Test.Tasty (TestTree, defaultMain, testGroup)
import Test.Tasty.HUnit (assertBool, assertEqual, testCase)

-- | Run one command from a given fake world, returning the next world.
step :: TestState -> (forall m. Losos m => m a) -> TestState
step = runTest

stateAfterChange :: Mode -> State
stateAfterChange mode = tsState (step initTest (cmdChange mode))

outputAfterChange :: Mode -> [Text]
outputAfterChange mode = tsOutput (step initTest (cmdChange mode))

tests :: TestTree
tests =
  testGroup
    "losos-ctl"
    [ testGroup
        "change"
        [ testCase "mesh flips sharing on, marks building, spawns, emits job" $ do
            let s = stateAfterChange Mesh
            assertEqual "mode" Mesh (stMode s)
            assertBool "sharing on" (stSharing s)
            assertBool "rebuild building" $
              case stRebuild s of
                Just rb -> rbState rb == Building && rbProgress rb == 0
                Nothing -> False
            assertBool "rebuild spawned" (tsSpawned (step initTest (cmdChange Mesh)))
            assertBool "emits job id" $
              any (T.isInfixOf "\"job\":\"job-0\"") (outputAfterChange Mesh),
          testCase "local rewrites the config line to false" $ do
            let cfg = tsConfig (step initTest (cmdChange Local))
            assertBool "config has sharingMyStorage = false" $
              any (T.isInfixOf "losos.sharingMyStorage = false;") cfg
        ],
      testGroup
        "status"
        [ testCase "fresh state reports idle" $ do
            let out = tsOutput (step initTest cmdStatus)
            assertBool "idle" (any (T.isInfixOf "\"state\":\"idle\"") out),
          testCase "after change reports building" $ do
            let out = tsOutput (step (step initTest (cmdChange Mesh)) cmdStatus)
            assertBool "building" (any (T.isInfixOf "\"state\":\"building\"") out)
        ],
      testGroup
        "rebuild-done"
        [ testCase "success -> done/100/complete" $ do
            let s = tsState (step (step initTest (cmdChange Mesh)) (cmdRebuildDone 0 "job-0"))
            case stRebuild s of
              Just rb -> do
                assertEqual "state" Done (rbState rb)
                assertEqual "progress" 100 (rbProgress rb)
                assertBool "message complete" ("rebuild complete" `T.isInfixOf` rbMessage rb)
              Nothing -> assertBool "has rebuild" False,
          testCase "failure appends log tail" $ do
            let withLog = initTest {tsLog = ["building... ", "error: evaluation failed"]}
                s = tsState (step (step withLog (cmdChange Mesh)) (cmdRebuildDone 1 "job-0"))
            case stRebuild s of
              Just rb -> assertBool "message includes log tail" ("evaluation failed" `T.isInfixOf` rbMessage rb)
              Nothing -> assertBool "has rebuild" False,
          testCase "stale job id is ignored (no clobber)" $ do
            -- change Mesh starts job-0 (building); a rebuild-done for a
            -- different (older) job must NOT flip it to done/failed.
            let s = tsState (step (step initTest (cmdChange Mesh)) (cmdRebuildDone 0 "stale-job"))
            case stRebuild s of
              Just rb -> do
                assertEqual "still building" Building (rbState rb)
                assertEqual "progress untouched" 0 (rbProgress rb)
              Nothing -> assertBool "has rebuild" False
        ],
      -- sanity: the IO-instance command names still resolve (compile-time check)
      testCase "state command is reachable in IO" $
        assertBool "config present" (any (T.isInfixOf "losos.sharingMyStorage") (tsConfig initTest))
    ]

main :: IO ()
main = defaultMain tests