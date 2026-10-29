# The box under Network in Windows' File Explorer, and each person's LosOS
# Cloud files as a network share there, in Finder and in Linux file
# managers (losos.cloud.windows.enable).
#
#   WS-Discovery (wsdd)   how Windows 10 and 11 find computers on the LAN,
#                         since they dropped SMB1 and its browse lists
#   mDNS `_smb._tcp`      how Finder and Linux file managers find them; it
#                         takes the place of the WebDAV entry
#                         (modules/cloud.nix), so the box is listed once
#   LLMNR                 how Windows turns the bare name it found into an
#                         address
#   Samba, SMB2 and up    the share: `\\<box>\<name>`, signed in to with the
#                         LosOS name and password
#
# Samba runs in user space. The kernel's SMB server stays blocked
# (modules/hardening.nix): a bug in it is a bug in the kernel.
#
# The shares are each person's `<cloud>/<login>/files`, the folder LosOS
# Cloud serves on every other door, and Samba touches every file as the
# cloud's own user. LosOS Cloud watches those folders (`losos-cloud serve
# --watch`, backend/src/cloud/watch.rs) and gives a file written from
# Windows its id and ETag as it lands, so the web app, the sync apps and
# live editing see it at once.
#
# The sign-in is NTLM, which can only be checked against the NT hash of a
# password. lososd makes one from the LosOS password when it sees it in the
# clear and writes the files Samba reads from the people file
# (backend/src/windows.rs): the password file, the shares, the name map and
# the Unix accounts Samba needs, as systemd userdb records on /run. A person
# whose password lososd has not seen since the share came on types it once
# on their Account page. Off, the NT hashes are deleted.
#
# LAN only: Samba takes the private address ranges and nothing else, not
# the box itself and not the mesh's pods; the edge's tunnel never carries
# it. nix/tests/windows.nix boots it.
{
  config,
  lib,
  ...
}:

let
  on = config.losos.cloud.windows.enable && config.lososInternal.appSignIn.cloud;
  dir = "/var/lib/losos-windows";
  samba = config.services.samba.package;
in
{
  config = lib.mkMerge [
    (lib.mkIf on {
      services.samba = {
        enable = true;
        # No NetBIOS: Windows 10 and 11 browse with WS-Discovery, and nmbd's
        # broadcasts are the old SMB1 world.
        nmbd.enable = false;
        winbindd.enable = false;
        settings.global = {
          "server string" = "LosOS";
          "workgroup" = "WORKGROUP";
          "security" = "user";
          "passdb backend" = "smbpasswd:${dir}/smbpasswd";
          "username map" = "${dir}/users.map";
          # The shares, written by lososd; the file ends back in [global].
          "include" = "${dir}/shares.conf";
          "map to guest" = "Never";
          "restrict anonymous" = 2;
          # Only shares the person may open are listed to them.
          "access based share enum" = "yes";

          "server min protocol" = "SMB2_10";
          "server smb encrypt" = "desired";
          "disable netbios" = "yes";
          "smb ports" = 445;
          "multicast dns register" = "no";

          # The LAN, never loopback (anything on the box) and never the
          # mesh's pod network: the same rule as the admin pages'
          # (modules/lan-only.nix).
          # IPv6 link-local and unique local addresses are the LAN too, and
          # a Mac tries the box's link-local address first: smbd takes the
          # connection on IPv6 before this list is read, so a refusal here
          # is a dropped connection the Mac does not retry over IPv4.
          # EXCEPT takes everything after it, so it comes last.
          "hosts allow" = "fe80::/10 fc00::/7 172.16.0.0/12 192.168.0.0/16 10. EXCEPT 10.42.";
          "hosts deny" = "ALL";

          # Each share touches its files as LosOS Cloud's own user; that is
          # set per share (backend/src/windows.rs), because here it would
          # also be IPC$'s, and the share list would then see every person
          # as that user.
          "unix extensions" = "no";

          "load printers" = "no";
          "printing" = "bsd";
          "printcap name" = "/dev/null";
          "disable spoolss" = "yes";

          "logging" = "systemd";
          "log level" = 1;
        };
      };

      services.samba-wsdd = {
        enable = true;
        openFirewall = true;
        workgroup = "WORKGROUP";
        # wsdd names the box in capitals by default. GNOME Files merges a
        # WS-Discovery entry into the mDNS one below only when the two
        # names match exactly, so both say the hostname as it is.
        extraOptions = [
          "--shortlog"
          "--preserve-case"
        ];
      };

      # The name WS-Discovery hands Windows is bare (`losos`), and Windows
      # asks for it with LLMNR. modules/mdns.nix keeps LLMNR off otherwise.
      services.resolved.settings.Resolve.LLMNR = lib.mkForce true;
      networking.firewall.allowedTCPPorts = [
        445
        5355
      ];
      networking.firewall.allowedUDPPorts = [ 5355 ];

      # One entry for the box in Finder and Linux file managers: the share,
      # in place of LosOS Cloud's WebDAV entry under the same name.
      losos.mdns.services.losos-cloud = lib.mkForce ''
        Name=%H
        Type=_smb._tcp
        Port=445
      '';

      systemd.services.samba-smbd = {
        after = [ "lososd.service" ];
        serviceConfig = {
          # Samba opens the cloud's files and nothing in the two data
          # homes.
          ProtectHome = true;
          PrivateTmp = true;
          PrivateDevices = true;
          ProtectClock = true;
          ProtectHostname = true;
          ProtectKernelLogs = true;
          ProtectKernelModules = true;
          ProtectKernelTunables = true;
          ProtectControlGroups = true;
          RestrictRealtime = true;
          RestrictSUIDSGID = true;
          LockPersonality = true;
          SystemCallArchitectures = "native";
        };
      };

      systemd.tmpfiles.rules = [ "d ${dir} 0700 root root -" ];

      systemd.services.lososd = {
        environment = {
          LOSOS_WINDOWS_DIR = dir;
          LOSOS_WINDOWS_SMBCONTROL = "${samba}/bin/smbcontrol";
          LOSOS_CLOUD_ROOT = "/var/lib/losos-cloud";
        };
        after = [ "losos-cloud.service" ];
      };
    })

    # Off: the NT hashes go, and the accounts with them.
    (lib.mkIf (!on) {
      systemd.tmpfiles.rules = [
        "R ${dir} - - - - -"
        "r /run/userdb/losos-win-*.user - - - - -"
        "r /run/userdb/losos-win-*.group - - - - -"
        "r /run/userdb/59???.user - - - - -"
        "r /run/userdb/59???.group - - - - -"
      ];
    })
  ];
}
