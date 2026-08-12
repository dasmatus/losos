{-# LANGUAGE OverloadedStrings #-}
{-# LANGUAGE RankNTypes #-}

-- | Pure tests for the losos state machine, run against the 'TestM'
-- interpreter (no filesystem, no nixos-rebuild). This is the payoff of the
-- 'Losos' type class: the exact same commands the daemon runs in 'IO' are
-- exercised here deterministically. Commands return their JSON response;
-- helpers below decode/assert on the returned bytes.
module Main (main) where

import qualified Data.ByteString.Lazy as BL
import Data.Text (Text)
import qualified Data.Text as T
import qualified Data.Text.Encoding as TE
import Installer
  ( BlockDev (..),
    InstallAction (..),
    Options (..),
    defaultEmitFile,
    defaultFlakeUrl,
    defaultFlakeWork,
    defaultKeyfile,
    defaultTargetRel,
    detectCandidates,
    execute,
    initTestState,
    planInstall,
    renderTarget,
    runTestM,
    tsFiles,
    tsDiskoRan,
    tsDiskoScriptRan,
    tsLayFlakeRan,
    tsNixosInstallRan,
    writtenFile,
  )
import Lib
  ( Losos,
    Mode (..),
    Rebuild (..),
    RebuildState (..),
    State (..),
    TestState (..),
    cmdApply,
    cmdChange,
    cmdFactoryReset,
    cmdSettings,
    cmdStatus,
    initTest,
    runTest,
    unitOutcome,
  )
import Test.Tasty (TestTree, defaultMain, testGroup)
import Test.Tasty.HUnit (assertBool, assertEqual, testCase)

-- │ Run one command from a given fake world, returning the next world.
step :: TestState -> (forall m. Losos m => m a) -> TestState
step st m = snd (runTest st m)

-- │ The JSON a command returned, as Text.
out :: TestState -> (forall m. Losos m => m BL.ByteString) -> Text
out st m = TE.decodeUtf8 (BL.toStrict (fst (runTest st m)))

stateAfterChange :: Mode -> State
stateAfterChange mode = tsState (step initTest (cmdChange mode))

-- ── Installer test helpers ────────────────────────────────────────────────
-- A canonical Options record matching the production defaults; individual
-- cases override the field they exercise.
baseOpts :: Options
baseOpts =
  Options
    { optTpm = False,
      optDrives = Nothing,
      optNoInstall = False,
      optDiskoScript = Nothing,
      optEmitTarget = Nothing,
      optFlakeUrl = defaultFlakeUrl,
      optFlakeWork = defaultFlakeWork,
      optKeyfile = defaultKeyfile,
      optTargetRel = defaultTargetRel,
      optEmitFile = defaultEmitFile
    }

isLog :: InstallAction -> Bool
isLog (ALog _) = True
isLog _ = False

isWrite :: InstallAction -> Bool
isWrite (AWriteTarget _ _) = True
isWrite _ = False

isEnsureKeyfile :: InstallAction -> Bool
isEnsureKeyfile (AEnsureKeyfile _) = True
isEnsureKeyfile _ = False

isDiskoScript :: InstallAction -> Bool
isDiskoScript (ARunDiskoScript _) = True
isDiskoScript _ = False

isClone :: InstallAction -> Bool
isClone (ACloneFlake _ _) = True
isClone _ = False

isDisko :: InstallAction -> Bool
isDisko (ARunDisko _) = True
isDisko _ = False

isNixosInstall :: InstallAction -> Bool
isNixosInstall (ARunNixosInstall _) = True
isNixosInstall _ = False

isLay :: InstallAction -> Bool
isLay (ALayFlake _ _) = True
isLay _ = False

isCopyKeyfile :: InstallAction -> Bool
isCopyKeyfile (ACopyKeyfile _ _) = True
isCopyKeyfile _ = False

tests :: TestTree
tests =
  testGroup
    "losos-ctl"
    [ testGroup
        "change"
        [ testCase "mesh flips sharing on, marks building, spawns, returns job" $ do
            let s = stateAfterChange Mesh
            assertEqual "mode" Mesh (stMode s)
            assertBool "sharing on" (stSharing s)
            assertBool "rebuild building" $
              case stRebuild s of
                Just rb -> rbState rb == Building && rbProgress rb == 0
                Nothing -> False
            assertBool "rebuild spawned" (tsSpawned (step initTest (cmdChange Mesh)))
            assertBool "returns job id" $
              "\"job\":\"job-0\"" `T.isInfixOf` out initTest (cmdChange Mesh),
          testCase "local rewrites the config line to false" $ do
            let cfg = tsConfig (step initTest (cmdChange Local))
            assertBool "config has sharingMyStorage = false" $
              any (T.isInfixOf "losos.sharingMyStorage = false;") cfg
        ],
      testGroup
        "status"
        [ testCase "fresh state reports idle" $ do
            assertBool "idle" ("\"state\":\"idle\"" `T.isInfixOf` out initTest cmdStatus),
          testCase "after change reports building" $ do
            let s = out (step initTest (cmdChange Mesh)) cmdStatus
            assertBool "building" ("\"state\":\"building\"" `T.isInfixOf` s),
          testCase "while building, message is the live rebuild log tail" $ do
            let withLog = initTest {tsLog = ["copying path '/nix/store/abc-bash'"]}
                s = out (step withLog (cmdChange Mesh)) cmdStatus
            assertBool "live tail in message" ("copying path" `T.isInfixOf` s)
        ],
      testGroup
        "unitOutcome"
        [ testCase "success -> done/100/complete" $ do
            let (st, pct, msg) = unitOutcome 0 ""
            assertEqual "state" Done st
            assertEqual "progress" 100 pct
            assertBool "message complete" ("rebuild complete" `T.isInfixOf` msg),
          testCase "failure appends log tail" $ do
            let (st, pct, msg) = unitOutcome 1 "error: evaluation failed"
            assertEqual "state" Failed st
            assertEqual "progress reset" 0 pct
            assertBool "message includes log tail" ("evaluation failed" `T.isInfixOf` msg),
          testCase "failure without log keeps bare message" $ do
            let (st, _, msg) = unitOutcome 2 ""
            assertEqual "state" Failed st
            assertEqual "message" "rebuild failed (exit 2)" msg
        ],
      -- sanity: the default config the tests start from has the sharing line
      testCase "default overrides include the sharing line" $
        assertBool "config present" (any (T.isInfixOf "losos.sharingMyStorage") (tsConfig initTest)),
      testGroup
        "settings"
        [ testCase "reports defaults from a fresh overrides file" $ do
            let s = out initTest cmdSettings
            assertBool "sharing true" ("\"sharingMyStorage\":true" `T.isInfixOf` s)
            assertBool "nextcloudMode container" ("\"nextcloudMode\":\"container\"" `T.isInfixOf` s)
            assertBool "forgejoMode container" ("\"forgejoMode\":\"container\"" `T.isInfixOf` s)
            assertBool "hostName mattbox" ("\"hostName\":\"mattbox\"" `T.isInfixOf` s)
            assertBool "apachePort 11000" ("\"apachePort\":11000" `T.isInfixOf` s)
            assertBool "no legacy keys leak" $
              not ("aioApachePort" `T.isInfixOf` s || "aioInterfacePort" `T.isInfixOf` s),
          testCase "legacy aio.apachePort feeds apachePort" $ do
            let code =
                  T.unlines
                    [ "{ ... }:",
                      "{",
                      "  losos.aio.apachePort = 12345;",
                      "}"
                    ]
                s = out (step initTest (cmdApply code)) cmdSettings
            assertBool "apachePort 12345" ("\"apachePort\":12345" `T.isInfixOf` s),
          testCase "legacy mode \"aio\" reads back as container" $ do
            let code =
                  T.unlines
                    [ "{ ... }:",
                      "{",
                      "  losos.nextcloud.mode = \"aio\";",
                      "}"
                    ]
                s = out (step initTest (cmdApply code)) cmdSettings
            assertBool "nextcloudMode container" ("\"nextcloudMode\":\"container\"" `T.isInfixOf` s)
        ],
      testGroup
        "apply"
        [ testCase "writes the nix code, marks building, spawns, returns job" $ do
            let code =
                  T.unlines
                    [ "{ ... }:",
                      "",
                      "{",
                      "  losos.sharingMyStorage = false;",
                      "  losos.hostName = \"box2\";",
                      "}"
                    ]
                st = step initTest (cmdApply code)
            assertBool "config rewritten to the applied lines" (any (T.isInfixOf "losos.hostName = \"box2\";") (tsConfig st))
            assertBool "old sharing line gone" (not (any (T.isInfixOf "losos.sharingMyStorage = true;") (tsConfig st)))
            assertBool "rebuild spawned" (tsSpawned st)
            assertBool "returns job" ("\"job\":\"job-0\"" `T.isInfixOf` out initTest (cmdApply code)),
          testCase "settings reflects applied values after apply" $ do
            let code =
                  T.unlines
                    [ "{ ... }:",
                      "{",
                      "  losos.sharingMyStorage = false;",
                      "  losos.hostName = \"box2\";",
                      "  losos.nextcloud.mode = \"native\";",
                      "  losos.nextcloud.apachePort = 12345;",
                      "}"
                    ]
                s = out (step initTest (cmdApply code)) cmdSettings
            assertBool "hostName box2" ("\"hostName\":\"box2\"" `T.isInfixOf` s)
            assertBool "nextcloudMode native" ("\"nextcloudMode\":\"native\"" `T.isInfixOf` s)
            assertBool "apachePort 12345" ("\"apachePort\":12345" `T.isInfixOf` s)
        ],
      testGroup
        "factory-reset"
        [ testCase "restores overrides, resets state, spawns, acks reset" $ do
            -- apply a non-default overrides first so reset has something to undo
            let code =
                  T.unlines
                    [ "{ ... }:",
                      "{",
                      "  losos.sharingMyStorage = false;",
                      "  losos.hostName = \"box2\";",
                      "}"
                    ]
                afterApply = step initTest (cmdApply code)
                st = step afterApply cmdFactoryReset
            -- overrides restored to the committed default body (hostName mattbox
            -- reappears; the applied box2 line is gone)
            assertBool "overrides restored to defaults" $
              any (T.isInfixOf "losos.hostName = \"mattbox\";") (tsConfig st)
            assertBool "applied hostName gone" $
              not (any (T.isInfixOf "losos.hostName = \"box2\";") (tsConfig st))
            -- state reset to defaultState (Local, sharing off)
            assertEqual "mode reset to Local" Local (stMode (tsState st))
            assertBool "sharing off" (not (stSharing (tsState st)))
            -- a rebuild was spawned
            assertBool "rebuild spawned" (tsSpawned st)
            -- the ack carries reset:true and a job id
            let r = out afterApply cmdFactoryReset
            assertBool "acks reset:true" ("\"reset\":true" `T.isInfixOf` r)
            assertBool "returns job id" ("\"job\":\"job-1\"" `T.isInfixOf` r)
        ],
      testGroup
        "installer"
        [ -- ── detectCandidates (pure drive filter) ──────────────────────────
          let vda = disk "vda" (8 * gi) [part "vda1" ["/"]]
              vdb = disk "vdb" (2 * gi) []
              vdc = disk "vdc" (2 * gi) []
              vdd = disk "vdd" (2 * gi) []
              -- distractors: removable, sub-1GB, and a loop device (not a disk)
              sdaRemovable = (disk "sda" (2 * gi) []) {bdRm = 1}
              sdbTiny = (disk "sdb" (500 * mi) []) {bdSize = 500 * mi}
              loop0 = (disk "loop0" (4 * gi) []) {bdType = "loop"}
              gi = 1024 * 1024 * 1024 :: Integer
              mi = 1024 * 1024 :: Integer
              disk n s kids = BlockDev n s 0 "disk" [] kids
              part n mps = BlockDev n (1 * gi) 0 "part" mps []
           in testGroup
                "detectCandidates"
                [ testCase "finds the three empty target disks" $ do
                    let cs = detectCandidates [vda, vdb, vdc, vdd]
                    assertEqual "three candidates" ["/dev/vdb", "/dev/vdc", "/dev/vdd"] cs,
                  testCase "excludes the mounted VM root (vda)" $
                    assertBool "vda excluded" (not ("/dev/vda" `elem` detectCandidates [vda, vdb, vdc, vdd])),
                  testCase "excludes removable, tiny, and non-disk devices" $
                    assertBool "only the three fixed disks" $
                      detectCandidates [vda, vdb, vdc, vdd, sdaRemovable, sdbTiny, loop0]
                        == ["/dev/vdb", "/dev/vdc", "/dev/vdd"]
                ],
          -- ── renderTarget (pure Nix rendering) ──────────────────────────────
          testGroup
            "renderTarget"
            [ testCase "emits targetDrives + tpm.enable=false (keyfile path)" $ do
                let t = renderTarget ["/dev/vdb", "/dev/vdc", "/dev/vdd"] False
                assertBool "has targetDrives assignment" (T.isInfixOf "losos.targetDrives" t)
                assertBool "lists drives" (T.isInfixOf "[ \"/dev/vdb\" \"/dev/vdc\" \"/dev/vdd\" ]" t)
                assertBool "tpm false" (T.isInfixOf "losos.tpm.enable = false;" t)
                assertBool "module body" (T.isInfixOf "{ ... }:" t),
              testCase "emits tpm.enable=true (TPM path)" $
                let t = renderTarget ["/dev/nvme0n1"] True
                 in assertBool "tpm true" (T.isInfixOf "losos.tpm.enable = true;" t)
            ],
          -- ── planInstall (the flow / action list) ───────────────────────────
          let bds = [disk "vda" (8 * gi) [part "vda1" ["/"]], disk "vdb" (2 * gi) [], disk "vdc" (2 * gi) [], disk "vdd" (2 * gi) []]
              gi = 1024 * 1024 * 1024 :: Integer
              disk n s kids = BlockDev n s 0 "disk" [] kids
              part n mps = BlockDev n (1 * gi) 0 "part" mps []
              opts = baseOpts
           in testGroup
                "planInstall"
                [ testCase "--emit-target writes the target and stops before disko" $ do
                    let o = opts {optEmitTarget = Just "/tmp/detected.nix"}
                        Right acts = planInstall o bds
                    assertBool "writes exactly the emit file" (any isWrite acts)
                    assertBool "writes no disko" (not (any isDisko acts))
                    assertBool "writes no disko-script" (not (any isDiskoScript acts))
                    assertBool "writes no clone" (not (any isClone acts))
                    assertBool "writes no nixos-install" (not (any isNixosInstall acts)),
                  testCase "--disko-script (keyfile path) ensures keyfile + runs script" $ do
                    let o = opts {optDiskoScript = Just "/etc/losos/disko-script"}
                        Right acts = planInstall o bds
                    assertBool "ensures keyfile" (any isEnsureKeyfile acts)
                    assertBool "runs disko-script" (any isDiskoScript acts)
                    assertBool "no clone" (not (any isClone acts))
                    assertBool "no nixos-install" (not (any isNixosInstall acts)),
                  testCase "--disko-script --tpm skips the keyfile" $ do
                    let o = (opts {optDiskoScript = Just "/etc/losos/disko-script"}) {optTpm = True}
                        Right acts = planInstall o bds
                    assertBool "no ensure-keyfile under TPM" (not (any isEnsureKeyfile acts)),
                  testCase "full path (keyfile): clone, disko, install, lay, copy keyfile" $ do
                    let Right acts = planInstall opts bds
                    assertBool "logs target drives" (any isLog acts)
                    assertBool "ensures keyfile" (any isEnsureKeyfile acts)
                    assertBool "clones flake" (any isClone acts)
                    assertBool "runs disko" (any isDisko acts)
                    assertBool "runs nixos-install" (any isNixosInstall acts)
                    assertBool "lays flake" (any isLay acts)
                    assertBool "copies keyfile" (any isCopyKeyfile acts),
                  testCase "full path --no-install stops after disko" $ do
                    let o = opts {optNoInstall = True}
                        Right acts = planInstall o bds
                    assertBool "runs disko" (any isDisko acts)
                    assertBool "no nixos-install" (not (any isNixosInstall acts))
                    assertBool "no lay" (not (any isLay acts))
                    assertBool "no copy keyfile" (not (any isCopyKeyfile acts)),
                  testCase "full path --tpm: no keyfile ensure, no keyfile copy" $ do
                    let o = opts {optTpm = True}
                        Right acts = planInstall o bds
                    assertBool "no ensure-keyfile" (not (any isEnsureKeyfile acts))
                    assertBool "no copy keyfile" (not (any isCopyKeyfile acts)),
                  testCase "no candidate disks is a Left (error)" $ do
                    let mounted = [disk "vda" (8 * gi) [part "vda1" ["/"]]]
                     in assertBool "no disks -> Left" (null (detectCandidates mounted))
                    let r = planInstall opts [disk "vda" (8 * gi) [part "vda1" ["/"]]]
                     in case r of
                          Left _ -> assertBool "Left" True
                          Right _ -> assertBool "expected Left" False,
                  testCase "--drives override bypasses detection" $ do
                    let o = opts {optDrives = Just ["/dev/nvme0n1"]}
                        Right acts = planInstall o bds
                        t = case [t' | AWriteTarget _ t' <- acts] of
                          (x : _) -> x
                          _ -> ""
                    assertBool "override drive in rendered target" (T.isInfixOf "/dev/nvme0n1" t),
                  testCase "execute (TestM) --emit-target writes the file, runs no disko" $ do
                    let o = opts {optEmitTarget = Just "/tmp/detected.nix"}
                        Right acts = planInstall o bds
                        st = runTestM initTestState (execute acts)
                    assertBool "wrote the emit file" (writtenFile st "/tmp/detected.nix" /= Nothing)
                    assertBool "no disko ran" (not (tsDiskoRan st)),
                  testCase "execute (TestM) --disko-script runs the script, no install" $ do
                    let o = opts {optDiskoScript = Just "/etc/losos/disko-script"}
                        Right acts = planInstall o bds
                        st = runTestM initTestState (execute acts)
                    assertEqual "ran the script" (Just "/etc/losos/disko-script") (tsDiskoScriptRan st)
                    assertBool "no nixos-install" (not (tsNixosInstallRan st)),
                  testCase "execute (TestM) full path runs every stage" $ do
                    let Right acts = planInstall opts bds
                        st = runTestM initTestState (execute acts)
                    assertBool "disko ran" (tsDiskoRan st)
                    assertBool "nixos-install ran" (tsNixosInstallRan st)
                    assertBool "lay ran" (tsLayFlakeRan st)
                ]
        ]
    ]

main :: IO ()
main = defaultMain tests
