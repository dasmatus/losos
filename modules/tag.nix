# LosOS Tag: the assistant people tag in LosOS Chat, and the model it and
# the rest of the box's AI share.
#
# `tag.enable` turns the tag on: the ask queue and the rooms' listener in
# lososd (`backend/src/tag.rs`), fed by Ollama on loopback through the
# same service `modules/builder.nix` runs. The tag's asks run on the ask's
# harness (backend/src/tag.rs `harness`): when the box has T3 Code
# (`tag.t3Bin`) that is the default, else OpenCode (`tag.opencodeBin`),
# else the box's own model over Ollama. The harnesses carry their own
# provider settings — T3 Code has no Ollama provider — and the built-in
# path is the one that answers when neither harness is configured.
#
# `tag.model` is the box's built-in model, and everything on the box's AI
# shares one: the assistant's Ask AI, the builder and the tag ask the
# same `modules/builder.nix` service (`LOSOS_ASSISTANT_MODEL`,
# `LOSOS_BUILDER_MODEL` and `LOSOS_TAG_MODEL` name the same tag by
# default). The Bonsai 27B default — a 1-bit Q1_0 recording of
# Qwen3.6-27B, 4.4 GB, 262,144 tokens of context, Apache License 2.0,
# Ollama 0.32.5 or newer (https://ollama.com/MichelRosselli/bonsai-27b) —
# is bigger than the smallest box's memory; the queue asks for a spillover
# between 50 and 75 percent of the layers onto the slower side when the
# box is tight, and keeps every ask on the mesh's own lenders
# (modules/inference-lend.nix) unless a turn actually runs.
{
  config,
  lib,
  pkgs,
  ...
}:

let
  cfg = config.losos.tag;
  inherit (cfg) model;
in
{
  config = lib.mkIf cfg.enable {
    systemd.services.lososd.environment = {
      # The tag's own flag: set only when the box carries it.
      LOSOS_TAG = "1";
      # The built-in model's tag on Ollama.
      LOSOS_TAG_MODEL = model;
      # Everything the tag reads is the person's: the web search's own
      # URL and the harnesses' binaries ride the environment alone, so
      # a search goes to the box's own SearXNG or nowhere, and only the
      # module's own paths ever spawn.
      # The spillover pair rides the environment always: the queue
      # reads its module's say, and falls back to its own constants
      # when the words carry nothing.
      LOSOS_TAG_SPILL_START = toString cfg.spilloverStart;
      LOSOS_TAG_SPILL_PRESSURED = toString cfg.spilloverPressured;
    }
    // lib.optionalAttrs (cfg.searxng != "") {
      LOSOS_TAG_SEARXNG_URL = cfg.searxng;
    }
    // lib.optionalAttrs (cfg.meshUrl != "") {
      # The network's own endpoint: in a mesh, the llama.cpp head the
      # edge runs for a model group, which is how a small box asks a
      # model no box holds alone.
      LOSOS_TAG_MESH_URL = cfg.meshUrl;
      LOSOS_TAG_MESH_MODEL = cfg.meshModel;
    }
    // lib.optionalAttrs (cfg.t3Bin != "") {
      LOSOS_TAG_T3_BIN = cfg.t3Bin;
    }
    // lib.optionalAttrs (cfg.opencodeBin != "") {
      LOSOS_TAG_OPENCODE_BIN = cfg.opencodeBin;
    }
    // lib.optionalAttrs (cfg.browser && cfg.enable) {
      # The ask's one reader. `midori` puts Playwright's WebKit shell in
      # place of Chromium — the lighter renderer — and `tag.midoriBin`
      # hands the wrapper the Midori binary itself, when the owner put
      # one in the store (nixpkgs does not carry it). Whatever the
      # engine, the ask gets one public page's text and nothing else.
      LOSOS_TAG_BROWSE_BIN = lib.getExe (
        pkgs.writers.writePython3Bin
          ("losos-tag-browse" + lib.optionalString (cfg.browserEngine == "midori") "-midori")
          {
            libraries = [ pkgs.python3Packages.playwright ];
            flakeIgnore = [ "E501" ];
          }
          (
            if cfg.midoriBin != "" then
              # The Midori adapter: its own process, one page, text alone.
              # Chromium-fork headless does not need a GPU or a JS console.
              ''
                import subprocess
                import sys

                url = sys.argv[1]
                out = subprocess.run(
                    [
                        "${cfg.midoriBin}",
                        "--headless",
                        "--disable-gpu",
                        "--no-first-run",
                        "--no-remote",
                        "--disable-features=Autofill,Translate",
                        "--dump-dom",
                        url,
                    ],
                    capture_output=True,
                    timeout=60,
                )
                sys.stdout.write(out.stdout.decode(errors="replace")[:20_000])
              ''
            else if cfg.browserEngine == "midori" then
              # The WebKit adapter: Playwright's own build, found through
              # the driver's browser directory, with no sign-ins, no
              # frames and one page.
              ''
                import os
                import sys
                os.environ.setdefault(
                    "PLAYWRIGHT_BROWSERS_PATH",
                    "${pkgs.playwright-driver.browsers}",
                )
                from playwright.sync_api import sync_playwright

                url = sys.argv[1]
                with sync_playwright() as p:
                    # WebKitGTK-class engine, the light renderer beside
                    # the box's own services.
                    browser = p.webkit.launch(headless=True)
                    page = browser.new_page()
                    page.goto(url, timeout=45_000)
                    print(page.inner_text("body")[:20_000])
                    browser.close()
              ''
            else
              ''
                import sys
                from playwright.sync_api import sync_playwright

                url = sys.argv[1]
                with sync_playwright() as p:
                    browser = p.chromium.launch(
                        executable_path="${pkgs.chromium}/bin/chromium",
                        headless=True,
                    )
                    page = browser.new_page()
                    page.goto(url, timeout=45_000)
                    print(page.inner_text("body")[:20_000])
                    browser.close()
              ''
          )
      );
    };
  };
}
