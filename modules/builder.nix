# The box's language models: Ollama on loopback, serving two small models.
# `builder.model` writes code: widgets and the owner's own apps for the
# admin page's "Build with AI" tabs, and LosOS Reviewer's reviews.
# `assistant.model` answers everything else, the apps' questions about a
# file or a message (backend/plugins/assistant).
#
# lososd asks it over HTTP (backend/src/builder.rs and the assistant
# plugin), so the owner's description, the code being changed and the
# material shown to the AI never leave the box, and nothing costs anything.
# Ollama listens on 127.0.0.1 only; nginx does not forward it.
#
# The models are not in the store or on the installer medium: two gigabytes
# would make the ISO twice as large. `ollama-model-loader` pulls them once,
# in the background, the first time the box is online, and retries with a
# growing delay until it has them. They land under /var/lib, which is on
# /persist, so they are pulled once per box, not once per boot. Until then
# the tabs and the Ask AI buttons say the model is still downloading.
#
# Ollama runs on the processor. The defaults, Qwen2.5-Coder 1.5B and
# Gemma 3 1B, are sized for the smallest box LosOS supports (4 GiB, two
# cores, LosOS Cloud and LosOS Git running beside them): about 1.5 GiB and
# 1 GiB of memory while they write. Ollama holds one at a time
# (`OLLAMA_MAX_LOADED_MODELS`), because the two together would not fit;
# asking the other one unloads the first, which takes a few seconds, and
# each gives its memory back five minutes after its last use.
{ config, lib, ... }:

let
  cfg = config.losos.builder;
  assistant = config.losos.assistant;
  port = 11434;
  url = "http://127.0.0.1:${toString port}";
in
{
  config = lib.mkIf (cfg.enable || assistant.enable) {
    services.ollama = {
      enable = true;
      host = "127.0.0.1";
      inherit port;
      loadModels = lib.unique (
        lib.optional cfg.enable cfg.model ++ lib.optional assistant.enable assistant.model
      );
      environmentVariables = {
        # One answer at a time, one model in memory: a mini PC shares its
        # memory with LosOS Cloud, the apps and the mesh.
        OLLAMA_NUM_PARALLEL = "1";
        OLLAMA_MAX_LOADED_MODELS = "1";
        OLLAMA_MAX_QUEUE = "4";
      };
    };

    systemd.services.lososd.environment =
      lib.optionalAttrs cfg.enable {
        LOSOS_BUILDER_URL = url;
        LOSOS_BUILDER_MODEL = cfg.model;
        LOSOS_BUILDER_CONTEXT = toString cfg.contextTokens;
      }
      // lib.optionalAttrs assistant.enable {
        LOSOS_ASSISTANT_URL = url;
        LOSOS_ASSISTANT_MODEL = assistant.model;
        LOSOS_ASSISTANT_CONTEXT = toString assistant.contextTokens;
      };
  };
}
