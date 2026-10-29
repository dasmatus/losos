# Unlock /persist only on the box's own network (losos.tpm.tang).
#
# With the TPM alone, the chip opens the volume wherever the box is switched
# on: a thief who carries the whole box away carries the key with it. With one
# or more Tang servers listed, the installer seals the disk key with Clevis to
# the TPM2 AND one of those servers (`sss`, threshold 2, the servers under a
# nested threshold of 1), keeps the result at /etc/keys/persist.jwe, and does
# not enrol a systemd TPM2 token in the LUKS header. Every boot the initrd
# brings the wired network up, asks a server, and opens the volume only with
# both answers. The server never sees the key (Tang's exchange is blinded);
# all it learns is that the box booted.
#
# Off the network, or once the servers have forgotten the box, the Clevis unit
# fails and systemd-cryptsetup asks at the console. The answer is the box's
# recovery code (backend/src/recovery.rs), which losos-unlock-recovery below
# adds as a LUKS passphrase as soon as lososd has minted it, and swaps when it
# changes.
#
# The JWE goes into the initrd on the ESP. That is fine: it is ciphertext only
# the TPM of this machine together with a Tang server can open.
#
# The box's edge can be one of the servers (`edge = true`, from
# `losos-install --tang-edge`; edge/registrar/src/tang.rs). Its key is this
# box's alone, and it is required on its own: the disk needs the TPM2, the
# edge, and one home server when any are listed. Once the box has a recovery
# code, losos-tang-edge-owner below tells the edge the hash of a secret made
# from it; the owner of a stolen box types the code on the edge's /tang/lost
# page, the browser makes the same secret, and the edge answers for the box
# no more. Neither the code nor the disk key ever reaches the edge.
{
  pkgs,
  lib,
  config,
  ...
}:

let
  tang = config.losos.tpm.tang;
  on = tang != [ ];
  device = "/dev/persist-vg/persist";
  keyfile = "/etc/keys/persist-keyfile";
  jwe = "/etc/keys/persist.jwe";
  codeFile = "/var/secrets/losos-recovery-code";
  stateDir = "/var/lib/losos-unlock";
  edges = builtins.filter (t: t.edge) tang;
  # The installer's {"url", "claim"}: the edge key's address and the secret
  # that names this box's owner to it.
  edgeFile = "/etc/keys/tang-edge.json";
  https = builtins.any (t: lib.hasPrefix "https://" t.url) tang;
  caBundle = "${pkgs.cacert}/etc/ssl/certs/ca-bundle.crt";
in
{
  config = lib.mkIf on {
    assertions = [
      {
        assertion = config.losos.tpm.enable;
        message = "losos.tpm.tang needs losos.tpm.enable: the disk key is sealed to the TPM2 AND a Tang server.";
      }
      {
        assertion = builtins.length edges <= 1;
        message = "losos.tpm.tang: one entry at most is the box's edge.";
      }
    ];

    boot.initrd.clevis = {
      enable = true;
      useTang = true;
      devices.persist.secretFile = jwe;
    };

    # The wired network in stage 1, so the Clevis unit can reach a server.
    # DHCP on every Ethernet port, and the box's name kept out of the request:
    # whoever runs the DHCP server learns a MAC address and nothing else.
    # NetworkManager runs stage 2 (configuration.nix), so networkd lets go of
    # the addresses at switch-root rather than handing them over.
    boot.initrd.network.enable = true;
    boot.initrd.network.flushBeforeStage2 = true;
    boot.initrd.systemd.network.networks."10-losos-unlock" = {
      matchConfig.Type = "ether";
      networkConfig.DHCP = "ipv4";
      dhcpV4Config.SendHostname = false;
    };
    # One port with a cable is enough to go on; a second, empty port would
    # otherwise hold the boot for the whole timeout.
    boot.initrd.systemd.network.wait-online = {
      anyInterface = true;
      timeout = 30;
    };
    # nixpkgs' unit asks once. The first answer can come before the port the
    # server is on has its address (another port was online first), so keep
    # asking for half a minute, each try cut off after 20 s, before
    # systemd-cryptsetup falls back to the console prompt: the prompt comes
    # within a minute either way. An empty key file is what that fallback
    # expects: it is what a failed `clevis decrypt` leaves, as in nixpkgs'
    # own test.
    # An edge is reached over https: the CA bundle in stage 1 for curl. The
    # initrd's clock is the firmware's, which is right on a box whose clock
    # battery is alive; Tang itself would not need TLS, the edge's router does.
    boot.initrd.systemd.storePaths = lib.mkIf https [ caBundle ];
    boot.initrd.systemd.services.cryptsetup-clevis-persist.environment = lib.mkIf https {
      CURL_CA_BUNDLE = caBundle;
    };
    boot.initrd.systemd.services.cryptsetup-clevis-persist.script = lib.mkForce ''
      mkdir -p /clevis-persist
      mount -t ramfs none /clevis-persist
      umask 277
      until=$(( $(date +%s) + 30 ))
      try=1
      while :; do
        if timeout 20 clevis decrypt < /etc/clevis/persist.jwe > /clevis-persist/decrypted; then
          exit 0
        fi
        [ "$(date +%s)" -lt "$until" ] || break
        echo "no Tang server answered (try $try), asking again"
        try=$(( try + 1 ))
        sleep 2
      done
      : > /clevis-persist/decrypted
      echo "no Tang server answered: the disk asks for the recovery code"
      exit 1
    '';
    # Drivers for the Ethernet ports repurposed mini-PCs carry, USB adapters
    # included, and the virtio one for VMs. The rest of the initrd has only
    # storage drivers (boot.nix).
    boot.initrd.availableKernelModules = [
      "virtio_net"
      "e1000"
      "e1000e"
      "igb"
      "igc"
      "r8169"
      "tg3"
      "alx"
      "atl1c"
      "r8152"
      "ax88179_178a"
      "cdc_ether"
    ];

    # The recovery code as a LUKS passphrase, so a box that cannot reach a
    # server can still be opened by its owner at the console. Authenticated
    # with the keyfile inside /persist; the code is read from its file and
    # never put on a command line. The slot it went into is remembered, so a
    # new code (after an erase or a restore) replaces the old one rather than
    # adding a second.
    systemd.services.losos-unlock-recovery = {
      description = "Add the recovery code as the disk's console passphrase";
      after = [ "lososd.service" ];
      unitConfig.ConditionPathExists = [
        codeFile
        keyfile
      ];
      path = [
        pkgs.cryptsetup
        pkgs.coreutils
        pkgs.gnugrep
      ];
      serviceConfig = {
        Type = "oneshot";
        StateDirectory = "losos-unlock";
        StateDirectoryMode = "0700";
        UMask = "0077";
      };
      script = ''
        code=$(tr -d '\n' < ${codeFile})
        [ -n "$code" ] || exit 0
        if printf '%s' "$code" | cryptsetup open --test-passphrase --key-file=- ${device} 2>/dev/null; then
          exit 0
        fi
        old=""
        [ -s ${stateDir}/slot ] && old=$(cat ${stateDir}/slot)
        # The free slot luksAddKey will take is the first one; read it back
        # from the header afterwards by testing the code against each slot.
        printf '%s' "$code" | cryptsetup luksAddKey --key-file=${keyfile} --new-keyfile=- ${device}
        for slot in $(seq 0 31); do
          if printf '%s' "$code" | cryptsetup open --test-passphrase --key-slot "$slot" --key-file=- ${device} 2>/dev/null; then
            if [ -n "$old" ] && [ "$old" != "$slot" ]; then
              cryptsetup luksKillSlot --key-file=${keyfile} ${device} "$old" || true
            fi
            printf '%s\n' "$slot" > ${stateDir}/slot
            echo "recovery code added as the console passphrase (slot $slot)"
            exit 0
          fi
        done
        echo "added the recovery code but could not find its slot" >&2
        exit 1
      '';
    };
    # The edge learns the hash of the owner's lost-box secret, made from the
    # recovery code exactly as the edge's /tang/lost page makes it in the
    # owner's browser (edge/registrar/src/tang.rs, `lost_secret`). The claim
    # secret the installer left in /persist proves this is the box the key
    # was made for. Sent again whenever the code changes, and retried while
    # the edge does not answer.
    systemd.services.losos-tang-edge-owner = lib.mkIf (edges != [ ]) {
      description = "Tell the edge how this box's owner can report it stolen";
      after = [
        "lososd.service"
        "network-online.target"
      ];
      wants = [ "network-online.target" ];
      unitConfig.ConditionPathExists = [
        codeFile
        edgeFile
      ];
      path = [
        pkgs.curl
        pkgs.coreutils
        pkgs.jq
      ];
      serviceConfig = {
        Type = "oneshot";
        StateDirectory = "losos-unlock";
        StateDirectoryMode = "0700";
        UMask = "0077";
        Restart = "on-failure";
        RestartSec = "5min";
      };
      script = ''
        code=$(tr -d '[:space:]' < ${codeFile} | tr 'A-Z' 'a-z')
        [ -n "$code" ] || exit 0
        secret=$(printf 'losos-tang-lost\n%s' "$code" | sha256sum | cut -d' ' -f1)
        lost=$(printf '%s' "$secret" | sha256sum | cut -d' ' -f1)
        if [ "$(cat ${stateDir}/edge-owner 2>/dev/null)" = "$lost" ]; then
          exit 0
        fi
        url=$(jq -er .url ${edgeFile})
        case "$url" in
          http://*/tang/* | https://*/tang/*) ;;
          *) echo "${edgeFile} names no edge key: $url" >&2; exit 1 ;;
        esac
        jq -c --arg lost "$lost" '{claim: .claim, lost: $lost}' ${edgeFile} \
          | curl -sS --fail-with-body --connect-timeout 10 --max-time 30 \
              -H 'Content-Type: application/json' --data-binary @- "$url/owner"
        printf '%s\n' "$lost" > ${stateDir}/edge-owner
        echo "the edge knows how this box's owner reports it stolen"
      '';
    };
    systemd.paths.losos-tang-edge-owner = lib.mkIf (edges != [ ]) {
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathExists = codeFile;
        PathChanged = codeFile;
      };
    };
    systemd.timers.losos-tang-edge-owner = lib.mkIf (edges != [ ]) {
      wantedBy = [ "timers.target" ];
      timerConfig = {
        OnBootSec = "10min";
        OnUnitActiveSec = "1d";
      };
    };

    systemd.paths.losos-unlock-recovery = {
      wantedBy = [ "multi-user.target" ];
      pathConfig = {
        PathExists = codeFile;
        PathChanged = codeFile;
      };
    };
  };
}
