# The builder's model: Ollama on loopback, serving one small model that
# writes widgets and the owner's own apps for the admin page's "Build with
# AI" tabs.
#
# lososd asks it over HTTP (backend/src/builder.rs), so the owner's
# description and the widget or app being changed never leave the box, and
# a build costs nothing. Ollama listens on 127.0.0.1 only; nginx does not
# forward it.
#
# The model is not in the store or on the installer medium: a gigabyte
# would make the ISO half as large again. `ollama-model-loader` pulls it
# once, in the background, the first time the box is online, and retries
# with a growing delay until it has it. It lands under /var/lib, which is on
# /persist, so it is pulled once per box, not once per boot. Until then the
# tabs say the model is still downloading.
#
# Ollama runs on the processor. The default, Qwen2.5-Coder 1.5B, is sized
# for the smallest box LosOS supports (4 GiB, two cores, LosOS cloud and
# LosOS Git running beside it): it takes about 1.5 GiB of memory while it
# writes and gives it back five minutes after the last build.
{ config, lib, ... }:

let
  cfg = config.losos.builder;
  port = 11434;
in
{
  config = lib.mkIf cfg.enable {
    services.ollama = {
      enable = true;
      host = "127.0.0.1";
      inherit port;
      loadModels = [ cfg.model ];
      environmentVariables = {
        # One build at a time, one model in memory: a mini PC shares its
        # memory with LosOS Cloud, the apps and the mesh.
        OLLAMA_NUM_PARALLEL = "1";
        OLLAMA_MAX_LOADED_MODELS = "1";
        OLLAMA_MAX_QUEUE = "4";
      };
    };

    systemd.services.lososd.environment = {
      LOSOS_BUILDER_URL = "http://127.0.0.1:${toString port}";
      LOSOS_BUILDER_MODEL = cfg.model;
      LOSOS_BUILDER_CONTEXT = toString cfg.contextTokens;
    };
  };
}
