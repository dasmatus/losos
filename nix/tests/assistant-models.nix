# nixos-test-vms config for the box's two language models on the smallest
# box LosOS runs on: 4 GiB of memory and two processor cores, with lososd,
# LosOS Login and LosOS Cloud running beside them.
#
# Questions that are not code (the assistant plugin: Ask AI in LosOS Cloud,
# Summarise in Mail, the other apps' questions) go to `assistant.model`,
# Gemma 3 1B; Build with AI keeps `builder.model`, a coder. This boots the
# real Ollama of modules/builder.nix with the real models, seeded from the
# store because the VM has no network (the blobs are fetched by digest from
# Ollama's registry, so a hash pins each one), and checks that
#
#   * lososd hands the assistant plugin Gemma and the builder the coder, and
#     the model loader would pull both;
#   * a question through /api/assistant is answered by Gemma itself;
#   * asking the coder afterwards unloads Gemma, so the two never hold
#     memory together, and neither runs the box out of memory.
#
# The memory seen while each model answers is written to memory.json in the
# test's output. Under TCG Gemma reads about four seconds a token and
# writes about seven, so one answer takes some twenty minutes: the
# material and the questions are short and the test gets three hours.
{ pkgs }:

let
  inherit (pkgs) lib;
  lososPkgs = import ../packages.nix { inherit pkgs; };

  assistantModel = "gemma3:1b";
  builderModel = "qwen2.5-coder:1.5b";

  blob =
    repo: hex:
    pkgs.fetchurl {
      name = "ollama-blob-${hex}";
      url = "https://registry.ollama.ai/v2/library/${repo}/blobs/sha256:${hex}";
      sha256 = hex;
    };

  # Each model as `ollama pull` leaves it: a manifest naming its blobs.
  models = {
    "gemma3/1b" = {
      repo = "gemma3";
      config = {
        hex = "76a6b777faea7a8fda1d339bfc81ea919ed429c64ed0d7c1b0617bc14afc76b6";
        size = 140;
      };
      runner = "llamacpp";
      layers = [
        {
          type = "model";
          hex = "2f8c3d137f2496dcb4019e17a50f2974b89c1be1a742db3c80cc86f91cafeeef";
          size = 806058400;
        }
        {
          type = "template";
          hex = "e0a42594d802e5d31cdc786deb4823edb8adff66094d49de8fffe976d753e348";
          size = 358;
        }
        {
          type = "license";
          hex = "dd084c7d92a3c1c14cc09ae77153b903fd2024b64a100a0cc8ec9316063d2dbc";
          size = 8432;
        }
        {
          type = "params";
          hex = "3116c52250752e00dd06b16382e952bd33c34fd79fc4fe3a5d2c77cf7de1b14b";
          size = 77;
        }
      ];
    };
    "qwen2.5-coder/1.5b" = {
      repo = "qwen2.5-coder";
      config = {
        hex = "152cb442202b836b5415fe4397169982b74dc3bdbb06b9777a126e0161c740da";
        size = 487;
      };
      runner = null;
      layers = [
        {
          type = "model";
          hex = "29d8c98fa6b098e200069bfb88b9508dc3e85586d20cba59f8dda9a808165104";
          size = 986048576;
        }
        {
          type = "system";
          hex = "66b9ea09bd5b7099cbb4fc820f31b575c0366fa439b08245566692c6784e281e";
          size = 68;
        }
        {
          type = "template";
          hex = "1e65450c30670713aa47fe23e8b9662bdf4065e81cc8e3cbfaa98924fcc0d320";
          size = 1615;
        }
        {
          type = "license";
          hex = "832dd9e00a68dd83b3c3fb9f5588dad7dcf337a0db50f7d9483f310cd292e92e";
          size = 11343;
        }
      ];
    };
  };

  manifest =
    m:
    pkgs.writeText "manifest.json" (
      builtins.toJSON (
        {
          schemaVersion = 2;
          mediaType = "application/vnd.docker.distribution.manifest.v2+json";
          config = {
            mediaType = "application/vnd.docker.container.image.v1+json";
            digest = "sha256:${m.config.hex}";
            inherit (m.config) size;
          };
          layers = map (l: {
            mediaType = "application/vnd.ollama.image.${l.type}";
            digest = "sha256:${l.hex}";
            inherit (l) size;
          }) m.layers;
        }
        // lib.optionalAttrs (m.runner != null) {
          inherit (m) runner;
          format = "gguf";
        }
      )
    );

  modelsDir = "/var/lib/losos-test-models";
  # The models directory, its files linked from the store.
  seed = lib.concatLists (
    lib.mapAttrsToList (
      path: m:
      [ "L+ ${modelsDir}/manifests/registry.ollama.ai/library/${path} - - - - ${manifest m}" ]
      ++ map (b: "L+ ${modelsDir}/blobs/sha256-${b.hex} - - - - ${blob m.repo b.hex}") (
        [ m.config ] ++ m.layers
      )
    ) models
  );

  # What memory looks like while the models answer, every two seconds.
  sampler = pkgs.writeShellScript "memory-sampler" ''
    while true; do
      avail=$(${pkgs.gawk}/bin/awk '/^MemAvailable:/ { print $2 }' /proc/meminfo)
      rss=0
      for p in $(${pkgs.procps}/bin/pgrep -f llama-server || true); do
        r=$(${pkgs.gawk}/bin/awk '/^VmRSS:/ { print $2 }' /proc/$p/status 2>/dev/null || echo 0)
        rss=$((rss + ''${r:-0}))
      done
      echo "$(date +%s) $avail $rss $(${pkgs.procps}/bin/pgrep -fc llama-server || true)" >> /run/memory-samples
      sleep 2
    done
  '';
in

pkgs.testers.nixosTest {
  name = "losos-assistant-models";
  # Two answers on emulated cores; see the note at the top.
  globalTimeout = 10800;

  nodes.box = _: {
    imports = [
      ../../modules/options.nix
      ../../modules/containers.nix
      ../../modules/tls.nix
      ../../modules/daemon.nix
      ../../modules/identity.nix
      ../../modules/cloud.nix
      ../../modules/builder.nix
    ];
    networking.hostName = "mattbox";
    losos = {
      hostName = "mattbox";
      backend.package = lososPkgs.losos-ctl;
      backend.plugins = lososPkgs.losos-plugins;
      # No browser here: an empty page in place of the admin UI.
      admin.ui = pkgs.runCommand "no-admin-ui" { } "mkdir $out; touch $out/index.html";
      cloud.enable = true;
      forgejo.enable = false;
      # The coder sized for a 4 GiB box, beside the assistant's Gemma.
      builder.model = builderModel;
      builder.contextTokens = 12288;
    };

    services.ollama.modelsDir = modelsDir;
    systemd.tmpfiles.rules = [
      "d ${modelsDir} 0755 root root -"
      "d ${modelsDir}/blobs 0755 root root -"
    ]
    ++ seed;
    # No network to pull from: the models are seeded above.
    systemd.services.ollama-model-loader.wantedBy = lib.mkForce [ ];
    systemd.services.ollama.environment.OLLAMA_NOPRUNE = "1";

    systemd.services.memory-sampler.serviceConfig.ExecStart = sampler;

    environment.systemPackages = [
      pkgs.curl
      pkgs.jq
    ];
    virtualisation = {
      memorySize = 4096;
      cores = 2;
      diskSize = 4096;
      # AVX2 for llama.cpp; TCG emulates it.
      qemu.options = [ "-cpu max" ];
    };
  };

  testScript =
    { nodes, ... }:
    ''
      import json
      import time

      ASSISTANT = "${assistantModel}"
      BUILDER = "${builderModel}"
      LOAD = ${builtins.toJSON nodes.box.services.ollama.loadModels}
      PASSWORD = "Correct-Horse-Battery-9"
      MAIL = (
          "From: Eva\n"
          "Subject: Lake trip\n\n"
          "Hi Matus, we meet at nine on Saturday at the boathouse. "
          "Please bring the paddles. Eva"
      )

      box.wait_for_unit("lososd.service")
      box.wait_for_unit("ollama.service")
      box.wait_for_open_port(11434)
      box.wait_for_open_port(8082)

      def api(path, token, method="GET", data=None):
          args = f"-X {method} http://127.0.0.1:8082{path} -H 'Authorization: Bearer {token}'"
          if data is not None:
              args += f" -H 'Content-Type: application/json' -d '{json.dumps(data)}'"
          return json.loads(box.succeed(f"curl -sf {args}"))

      def ollama_ps():
          return [m["name"] for m in json.loads(box.succeed("curl -sf 127.0.0.1:11434/api/ps"))["models"]]

      def samples(since):
          rows = [l.split() for l in box.succeed("cat /run/memory-samples").splitlines()]
          return [(int(a), int(r), int(n)) for t, a, r, n in rows if int(t) >= since]

      with subtest("the assistant gets Gemma and the builder the coder"):
          env = box.succeed("systemctl show lososd -p Environment --value")
          assert f"LOSOS_ASSISTANT_MODEL={ASSISTANT}" in env, env
          assert f"LOSOS_BUILDER_MODEL={BUILDER}" in env, env
          assert "LOSOS_ASSISTANT_URL=http://127.0.0.1:11434" in env, env
          assert sorted(LOAD) == sorted([ASSISTANT, BUILDER]), LOAD
          env = box.succeed("systemctl show ollama -p Environment --value")
          assert "OLLAMA_MAX_LOADED_MODELS=1" in env, env
          listed = [m["name"] for m in json.loads(box.succeed("curl -sf 127.0.0.1:11434/api/tags"))["models"]]
          assert sorted(listed) == sorted([ASSISTANT, BUILDER]), listed
          # Nothing about the models leaves the box.
          assert "OLLAMA_NO_CLOUD=1" in env, env
          box.succeed("journalctl -u ollama | grep -q 'Ollama cloud disabled: true'")
          box.fail("journalctl -u ollama | grep -q 'lookup ollama.com'")

      with subtest("claim the box"):
          out = box.succeed(
              "curl -sf -X POST http://127.0.0.1:8082/api/setup/claim -H 'Host: mattbox.local'"
              f" -H 'Content-Type: application/json' -d '{json.dumps({"name": "matus", "password": PASSWORD})}'"
          )
          token = json.loads(out)["token"]
          box.wait_for_unit("losos-cloud.service")
          info = api("/api/assistant", token)
          assert info == {"available": True, "ready": True, "model": ASSISTANT}, info

      box.systemctl("start memory-sampler.service")

      with subtest("Gemma answers a question about a message"):
          start = int(time.time())
          ask = api("/api/assistant/asks", token, method="POST", data={
              "question": "In one sentence: when and where do they meet?",
              "sources": [{"title": "Lake trip", "text": MAIL}],
              "lang": "en",
          })
          deadline = time.time() + 3600
          while (state := api(f"/api/assistant/asks/{ask['id']}", token))["state"] not in ("done", "failed"):
              assert time.time() < deadline, state
              time.sleep(10)
          print("Gemma:", state)
          assert state["state"] == "done", state
          assert len(state["answer"].strip()) > 10, state
          assert ollama_ps() == [ASSISTANT], ollama_ps()
          gemma = samples(start)
          box.succeed("journalctl -u ollama | grep -q 'gemma3'")

      with subtest("the coder unloads Gemma, so the two never share memory"):
          start = int(time.time())
          body = {
              "model": BUILDER,
              "stream": False,
              "messages": [{"role": "user", "content": "Write an HTML comment that says hello."}],
              "options": {"num_ctx": 12288, "num_predict": 12},
          }
          box.succeed(
              f"curl -sf --max-time 3600 127.0.0.1:11434/api/chat -d '{json.dumps(body)}' > /tmp/coder.json"
          )
          answer = json.loads(box.succeed("cat /tmp/coder.json"))
          print("Coder:", answer)
          assert answer["message"]["content"].strip(), answer
          assert ollama_ps() == [BUILDER], ollama_ps()
          coder = samples(start)

      with subtest("both fit in 4 GiB, one at a time"):
          box.systemctl("stop memory-sampler.service")
          rows = gemma + coder
          assert max(n for _, _, n in rows) <= 1, "two models were in memory at once"
          box.fail("journalctl -k | grep -iE 'out of memory|oom-kill'")
          used = {
              name: {
                  "peakRssMiB": max(r for _, r, _ in rows) // 1024,
                  "leastAvailableMiB": min(a for a, _, _ in rows) // 1024,
              }
              for name, rows in ((ASSISTANT, gemma), (BUILDER, coder))
          }
          report = {"memoryMiB": 4096, "cores": 2, "models": used}
          print(json.dumps(report, indent=2))
          for name, m in used.items():
              assert m["leastAvailableMiB"] >= 256, (name, m)
          box.succeed(f"echo '{json.dumps(report)}' > /tmp/memory.json")
          box.copy_from_machine("/tmp/memory.json")
    '';
}
