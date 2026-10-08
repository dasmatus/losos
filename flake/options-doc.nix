# The `losos.*` option tree as a document the admin UI can draw.
#
# The Advanced pane (admin-ui/app/src/screens/settings/pane-advanced.tsx)
# shows every option this appliance declares, with a typed editor for the
# ones a running box can take through modules/overrides.nix. It is generated
# from the declarations, not written by hand, so an option added to
# modules/options.nix appears on the pane without anyone remembering to list
# it — and so the pane cannot describe an option differently from the module
# that declares it.
#
# Two consumers, one function:
#   * modules/config-repo.nix evaluates it against the box's own
#     configuration and writes /etc/losos/options.json, which lososd serves
#     at GET /api/options with the current overrides.nix assignments joined
#     in (backend/src/options.rs). The document therefore describes the
#     modules the box actually runs, and `current` is what the last rebuild
#     merged.
#   * flake.nix evaluates it against the published `install` configuration,
#     pure, for the `losos-options-doc` check and for the browser test
#     (tests/admin-ui.nix), which renders the real document and fails on any
#     row the pane cannot draw.
#
# `classify` is the gate the second consumer exists for: an option whose type
# it does not know is a `throw`, so a declaration with a new type turns the
# eval job red instead of landing on the pane as an empty row. Add the type
# here AND an editor in the pane (lib/option-value.ts) to let it through.
#
# What is left out, and why:
#   * losos.edge.* — the VPS side of the master proxy (modules/edge.nix is
#     imported by the `nixosModules.edge` output, never by `install`).
#     Nothing on the appliance reads them, so a switch for one on this pane
#     would flip nothing and say it had.
#   * the installer's three (targetDrives, tpm.enable, bios) are *shown* but
#     not editable: losos-install writes them into modules/install-target.nix
#     at normal priority, so a line for one in overrides.nix would make the
#     next rebuild fail with "conflicting definition values". They say so.
#   * package, path and attribute-set options are shown read-only. A package
#     is chosen by the flake, not typed into a form.
{
  lib,
  options,
  config,
}:

let
  # Prefixes left out of the document entirely, with the reason (kept here so
  # the omission is a decision rather than an accident of the walk).
  excluded = {
    edge = "declared for the edge VPS (nixosModules.edge); nothing on the appliance reads it";
  };

  # Options another file on the box sets at normal priority: a line in
  # overrides.nix would collide with it. `fixed` names who owns the value.
  fixed = {
    "targetDrives" = "installer";
    "tpm.enable" = "installer";
    "bios" = "installer";
    # The live medium's two: the installed system never consults them.
    "installer.autorun" = "installer";
    "installer.package" = "installer";
    # The medium's Secure Boot shape is decided when the ISO is built
    # (modules/secure-boot.nix); the installed system never reads these.
    "secureBoot.enable" = "installer";
    "secureBoot.certFile" = "installer";
  };

  # Options whose wrong value leaves the box unreachable, unbootable, or
  # locked out of the thing that would fix it. The pane asks before it
  # changes one. Each entry is a prefix: "cluster" covers cluster.*.
  dangerous = [
    "admin"
    "backend"
    "cluster"
    "hardening.enable"
    "hostName"
    "keyring"
    "nextcloud.mode"
    "forgejo.mode"
    "shared.fscrypt"
    "storage"
    "tls"
    "upgradeFlakeUri"
    "proxy.tokenFile"
    "proxy.bootstrapTokenFile"
    "cache"
  ];

  isDangerous = name: lib.any (p: name == p || lib.hasPrefix "${p}." name) dangerous;

  # Options that take effect only while another (a bool) is on. The pane
  # greys the row and says so while that one is off in the draft. The value
  # still saves; it just does nothing until the gate opens. Each pair is
  # enforced in the modules, not here (lososInternal.federation in
  # modules/options.nix for the two below).
  needs = {
    "forgejo.federation.enable" = "sharingMyStorage";
    "nextcloud.federation.enable" = "sharingMyStorage";
  };

  # The two integer bounds a NixOS int type carries only in its description.
  # `ints.between 50 100` says "integer between 50 and 100 (both inclusive)".
  betweenBounds =
    desc:
    let
      m = builtins.match "integer between ([0-9]+) and ([0-9]+).*" desc;
    in
    if m == null then
      null
    else
      {
        min = lib.toInt (builtins.elemAt m 0);
        max = lib.toInt (builtins.elemAt m 1);
      };

  # The editor kind for a type, or a throw for one this file has not met.
  #
  # The result always carries `kind`; `int` adds `min`/`max` when the type
  # bounds them, `enum` adds `values`, `nullable` wraps an inner kind, and
  # `opaque` carries `reason` — the type's own description — for the
  # read-only row. `form` is a hint for the text editor: "path" for the
  # secret-path options (a file on the box, typed as a string).
  classify =
    name: type:
    let
      n = type.name;
    in
    if n == "bool" then
      { kind = "bool"; }
    else if n == "str" then
      { kind = "str"; }
    else if lib.hasPrefix "strMatching" n then
      {
        kind = "str";
        pattern = type.functor.payload.pattern;
      }
    else if n == "int" then
      { kind = "int"; }
    else if n == "positiveInt" then
      {
        kind = "int";
        min = 1;
      }
    else if n == "unsignedInt" then
      {
        kind = "int";
        min = 0;
      }
    else if n == "unsignedInt16" then
      {
        kind = "int";
        min = 0;
        max = 65535;
      }
    else if n == "intBetween" then
      { kind = "int"; } // (betweenBounds type.description)
    else if n == "float" then
      { kind = "float"; }
    else if n == "enum" then
      {
        kind = "enum";
        values = type.functor.payload.values;
      }
    else if n == "listOf" then
      (
        let
          inner = classify name type.nestedTypes.elemType;
        in
        if inner.kind == "str" then
          { kind = "list"; }
        else
          {
            kind = "opaque";
            reason = type.description;
          }
      )
    else if n == "nullOr" then
      (
        let
          inner = classify name type.nestedTypes.elemType;
        in
        if inner.kind == "opaque" then
          inner // { reason = type.description; }
        else
          {
            kind = "nullable";
            inherit inner;
          }
      )
    else if n == "coercedTo" then
      # options.nix's `secretPath`: a string that names a file on the box.
      (classify name type.nestedTypes.finalType) // { form = "path"; }
    else if
      lib.elem n [
        "package"
        "path"
        "attrs"
        "attrsOf"
        "submodule"
      ]
    then
      {
        kind = "opaque";
        reason = type.description;
      }
    else
      throw "flake/options-doc.nix: losos.${name} has type `${n}` (${type.description}), which the Advanced pane has no editor for. Add it to `classify` here and to admin-ui/app/src/lib/option-value.ts.";

  # A value as JSON the pane can show. Packages become their name, anything
  # else that `builtins.toJSON` could not carry becomes its pretty-printed
  # Nix, and the simple shapes travel as themselves.
  plain =
    v:
    if lib.isDerivation v then
      { package = v.name or "package"; }
    else if builtins.isAttrs v then
      { nix = lib.generators.toPretty { multiline = false; } v; }
    else if builtins.isFunction v then
      { nix = "<function>"; }
    else
      v;

  # `defaultText` when the declaration gives one (a literalExpression), else
  # the default pretty-printed as Nix, else nothing.
  defaultText =
    opt:
    if opt ? defaultText then
      (if builtins.isAttrs opt.defaultText then opt.defaultText.text else toString opt.defaultText)
    else if opt ? default then
      lib.generators.toPretty { multiline = false; } (plain opt.default)
    else
      null;

  # The nixpkgs lib.types description, with the one bit of markup it may
  # carry stripped, so it reads as a sentence on the row.
  describeType = type: type.description;

  leaf =
    path: opt:
    let
      name = lib.concatStringsSep "." path;
      editor = classify name opt.type;
      group = if builtins.length path > 1 then builtins.head path else "general";
    in
    {
      inherit name group editor;
      nixType = describeType opt.type;
      description = opt.description or "";
      default = if opt ? default then plain opt.default else null;
      defaultText = defaultText opt;
      current = plain (lib.getAttrFromPath path config.losos);
      danger = isDangerous name;
      fixed = fixed.${name} or null;
      readOnly = editor.kind == "opaque" || (fixed.${name} or null) != null;
      # `visible = false`: still in the document, so /api/apply and a push to
      # LosOS Git accept it, but the pane does not draw a row for it.
      hidden = (opt.visible or true) == false;
    }
    // lib.optionalAttrs (needs ? ${name}) { needs = needs.${name}; };

  walk =
    path: v:
    if lib.isOption v then
      [ (leaf path v) ]
    else
      lib.concatLists (
        lib.mapAttrsToList (k: x: if path == [ ] && excluded ? ${k} then [ ] else walk (path ++ [ k ]) x) v
      );
in
{
  # Bumped when a consumer would misread an older document. lososd checks it.
  version = 1;
  options = walk [ ] options.losos;
  # So a reader of the document knows what is not in it.
  inherit excluded;
}
