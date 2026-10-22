# The box's language models: Ollama on loopback, serving two small models.
# `builder.model` writes widgets for the admin page's "Build with AI" tab;
# `assistant.model` answers the apps' questions about a file or a message
# (backend/plugins/assistant): code to one, everything else to the other.
#
# lososd asks it over HTTP (backend/src/builder.rs and the assistant
# plugin), so the owner's description, the widget being changed and the
# material shown to the AI never leave the box, and nothing costs anything.
# Ollama listens on 127.0.0.1 only; nginx does not forward it.
#
# The models are not in the store or on the installer medium: they would
# double the ISO. `ollama-model-loader` pulls them once, in the background,
# the first time the box is online, and retries with a growing delay until
# it has them. They land under /var/lib, which is on /persist, so they are
# pulled once per box, not once per boot. Until then the tab and the Ask AI
# buttons say the model is still downloading.
#
# Ollama runs on the processor and holds one model at a time
# (`OLLAMA_MAX_LOADED_MODELS`): on a 4 GiB box the two together would not
# fit beside LosOS Cloud and the apps. Asking the other one unloads the
# first, which takes a few seconds, and each gives its memory back five
# minutes after its last use.
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
