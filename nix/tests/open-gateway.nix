# An open gateway a box vets for itself, for the VM tests that need an edge
# the box trusts with the market (backend/access/src/gateway/check.rs). Made
# here, at build time, with throwaway keys:
#
#   * a TPM maker: a CA that signs the endorsement key certificate the
#     edge's swtpm carries at the index the TCG profile names, so the box
#     finds the TPM vouched for (`box` puts the CA in its roots);
#   * per gateway URL, the edge key and its owner's passport: an OpenPGP key,
#     the statement naming the URL and the edge key, the detached signature,
#     and the settings file with LosOS's and the owner's payout addresses.
#     The edge boots with both in place, so losos-gateway-measure extends
#     PCR 15 with the settings on the first boot and no claim or reboot is
#     needed.
#
# What the edge measured on the way (PCR 4's programs, PCR 11) depends on
# how the test VM booted, so the published image the box checks it against
# is read off the running edge (`losos-registrar passport image`) and handed
# to the box by `trust(edge, box)` in the test script, which restarts lososd
# so its checks start from that image.
#
#   gw = import ./open-gateway.nix { inherit pkgs lososPkgs; };
#   nodes.edge = { imports = [ (gw.edge "https://edge:8444") ]; … };
#   nodes.box = { imports = [ gw.box ]; … };
#   testScript = gw.script + ''
#     …
#     trust(edge, box)
#   '';
{
  pkgs,
  lososPkgs,
}:

let
  days = "3650";
  # LosOS's 2% and the owner's 2%, on one network.
  losos = {
    base = "0x5050505050505050505050505050505050505050";
  };
  ownerPayout = "0x0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a0a";
  lososJson = pkgs.writeText "losos-test-payouts.json" (builtins.toJSON losos);
  imagesFile = "/run/losos-test/gateway-images.json";

  maker = pkgs.runCommand "losos-test-tpm-maker" { nativeBuildInputs = [ pkgs.openssl ]; } ''
    mkdir -p $out
    openssl req -x509 -newkey rsa:2048 -nodes -days ${days} \
      -subj "/CN=LosOS test TPM maker" \
      -addext basicConstraints=critical,CA:TRUE \
      -addext keyUsage=critical,keyCertSign,cRLSign \
      -keyout $out/ca.key -out $out/ca.pem 2>/dev/null
  '';

  # ring keeps an Ed25519 key as PKCS#8 v2, the public key beside the seed.
  edgeKeygen = pkgs.writeText "losos-test-edge-keygen.py" ''
    import os
    import sys
    from cryptography.hazmat.primitives.asymmetric.ed25519 import Ed25519PrivateKey
    from cryptography.hazmat.primitives.serialization import Encoding, PublicFormat

    seed = os.urandom(32)
    public = Ed25519PrivateKey.from_private_bytes(seed).public_key().public_bytes(
        Encoding.Raw, PublicFormat.Raw
    )
    pkcs8 = (
        bytes.fromhex("3053020101300506032b657004220420")
        + seed
        + bytes.fromhex("a123032100")
        + public
    )
    open(sys.argv[1], "w").write(pkcs8.hex())
    open(sys.argv[2], "w").write(public.hex())
  '';

  passport =
    url:
    pkgs.runCommand "losos-test-gateway-passport"
      {
        nativeBuildInputs = [
          lososPkgs.losos-registrar
          pkgs.gnupg
          (pkgs.python3.withPackages (p: [ p.cryptography ]))
        ];
      }
      ''
        mkdir -p $out
        python3 ${edgeKeygen} $out/edge.key $out/edge.pub
        losos-registrar passport statement --url ${url} \
          --edge-key "$(cat $out/edge.pub)" --payout base=${ownerPayout} \
          --days ${days} > $out/statement.txt
        export GNUPGHOME=$TMPDIR/gnupg
        mkdir -m 0700 $GNUPGHOME
        gpg --batch --pinentry-mode loopback --passphrase "" \
          --quick-gen-key "LosOS test owner <owner@example.test>" ed25519 sign never
        gpg --batch --armor --export > $out/owner.asc
        gpg --batch --pinentry-mode loopback --passphrase "" \
          --armor --detach-sign -o $out/statement.asc $out/statement.txt
        losos-registrar passport settings --statement $out/statement.txt \
          --signature $out/statement.asc --key $out/owner.asc \
          --losos-payouts ${lososJson} > $out/settings.json
      '';

  # Run by the test driver beside swtpm before each boot: the endorsement
  # key's certificate, signed by the test maker, at 0x01c00002. Once: the
  # TPM's state lives as long as the test.
  provisioning = ''
    if ! tpm2_nvreadpublic 0x01c00002 >/dev/null 2>&1; then
      d=$(${pkgs.coreutils}/bin/mktemp -d)
      tpm2_createek -G rsa -u "$d/ek.pub" -c "$d/ek.ctx" >/dev/null
      tpm2_readpublic -c "$d/ek.ctx" -f pem -o "$d/ek.pem" >/dev/null
      tpm2_flushcontext -t
      ${pkgs.openssl}/bin/openssl x509 -new -subj "/CN=LosOS test EK" \
        -force_pubkey "$d/ek.pem" -CA ${maker}/ca.pem -CAkey ${maker}/ca.key \
        -days ${days} -outform DER -out "$d/ek.der"
      tpm2_nvdefine 0x01c00002 -C o -s "$(${pkgs.coreutils}/bin/stat -c %s "$d/ek.der")" \
        -a "ownerwrite|ownerread|authwrite|authread|no_da"
      tpm2_nvwrite 0x01c00002 -C o -i "$d/ek.der"
      ${pkgs.coreutils}/bin/rm -rf "$d"
    fi
  '';
in
{
  inherit losos maker passport;

  # The edge side: a TPM vouched for by the test maker, the passport for
  # `url`, and losos.edge.passport.
  edge =
    url:
    let
      p = passport url;
    in
    { config, pkgs, ... }:
    {
      virtualisation.useEFIBoot = true;
      virtualisation.tpm = {
        enable = true;
        inherit provisioning;
      };
      losos.edge.passport.enable = true;
      systemd.tmpfiles.rules = [
        "d /var/lib/losos-registrar 0700 root root - -"
        "C /var/lib/losos-registrar/edge.key 0600 root root - ${p}/edge.key"
        "C /var/lib/losos-gateway/settings.json 0600 root root - ${p}/settings.json"
      ];
      environment.systemPackages = [
        pkgs.tpm2-tools
        config.losos.edge.registrar.package
      ];
    };

  # The box side: the test maker among its TPM roots, LosOS's test payout
  # addresses, and the images `trust` writes.
  box =
    { lib, ... }:
    {
      losos.proxy.gateways = {
        tpmRoots = "${maker}/ca.pem";
        payouts = losos;
      };
      systemd.services.lososd.environment.LOSOS_GATEWAY_IMAGES = lib.mkForce imagesFile;
      systemd.tmpfiles.rules = [ "d /run/losos-test 0755 root root - -" ];
    };

  # Python for the test script: hand the box the image the edge booted, as
  # the published image it checks the edge's quote against.
  script = ''
    import json as _gw_json


    def trust(edge, *boxes):
        edge.wait_for_unit("losos-gateway-measure.service")
        image = edge.succeed("losos-registrar passport image --name 'LosOS test gateway'")
        print(f"{edge.name} booted: {image}")
        doc = _gw_json.dumps([_gw_json.loads(image)])
        for b in boxes:
            b.succeed(f"mkdir -p /run/losos-test && echo '{doc}' > ${imagesFile}")
            # lososd reads its anchors once, at its first scan.
            b.succeed("systemctl restart lososd.service")
            b.wait_until_succeeds("curl -fsS 127.0.0.1:8082/api/health")
  '';
}
