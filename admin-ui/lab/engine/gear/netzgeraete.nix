# Netzgeräte Betriebssystem: the root filesystem of the simulator's routers,
# switches and Wi-Fi access points. Static busybox on a read-only ext4 image
# with tmpfs over /tmp, /run and /var, same conventions as the LosOS guest.
# The kernel is the LosOS guest's bzImage; only this image comes from Nix.
#
#   nix-build netzgeraete.nix --arg nixpkgs /path/to/nixpkgs   # -> result/rootfs.bin
{
  nixpkgs ? null,
  pkgs ? (
    if nixpkgs == null then import <nixpkgs> { } else import nixpkgs { system = "x86_64-linux"; }
  ),
}:
let
  busybox = pkgs.pkgsStatic.busybox;
  osName = "Netzgeräte Betriebssystem";
  osVersion = "1.0";

  inittab = pkgs.writeText "inittab" ''
    ::sysinit:/etc/init.d/rcS
    ttyS0::respawn:-/bin/sh
    ::ctrlaltdel:/bin/reboot
  '';

  profile = pkgs.writeText "profile" ''
    export PS1='\h# '
    export PATH=/bin:/usr/bin
  '';

  osRelease = pkgs.writeText "os-release" ''
    NAME="${osName}"
    PRETTY_NAME="${osName} ${osVersion}"
    ID=netzgeraete
    VERSION_ID=${osVersion}
    VERSION="${osVersion}"
  '';

  rcS = pkgs.writeText "rcS" ''
    #!/bin/sh
    # Read-only root; tmpfs over everything that changes.
    mount -t proc proc /proc
    mount -t sysfs sysfs /sys
    mount -t tmpfs tmpfs /tmp
    mount -t tmpfs tmpfs /run
    mount -t tmpfs tmpfs /var
    mkdir -p /var/log /var/run /var/lib/misc /run/netz /tmp/www
    arg() { sed -n "s/.*$1=\([^ ]*\).*/\1/p" /proc/cmdline; }
    HOST=$(arg losos.host); ROLE=$(arg losos.role); IP=$(arg losos.ip)
    GW=$(arg losos.gw); DHCP=$(arg losos.dhcp)
    [ -n "$HOST" ] || HOST=netzgeraet
    [ -n "$ROLE" ] || ROLE=switch
    hostname "$HOST"
    echo "$HOST" > /run/netz/host; echo "$ROLE" > /run/netz/role
    ip link set lo up
    ip link set eth0 up 2>/dev/null
    if [ -n "$IP" ]; then
      ip addr add "$IP" dev eth0
      [ -n "$GW" ] && ip route add default via "$GW"
    fi
    ADDR=''${IP%/*}
    [ -n "$GW" ] && echo "nameserver $GW" > /tmp/resolv.conf || : > /tmp/resolv.conf
    if [ "$ROLE" = router ] && [ -n "$IP" ]; then
      echo 1 > /proc/sys/net/ipv4/ip_forward
      eval "$(ipcalc -m "$IP")"
      if [ -n "$DHCP" ]; then
        FIRST=''${DHCP%-*}; LAST=''${DHCP#*-}
      else
        NET=''${ADDR%.*}; FIRST=$NET.100; LAST=$NET.199
      fi
      cat > /run/netz/udhcpd.conf <<EOF
    interface eth0
    start $FIRST
    end $LAST
    lease_file /var/lib/misc/udhcpd.leases
    pidfile /var/run/udhcpd.pid
    option subnet $NETMASK
    option router $ADDR
    option dns $ADDR
    option lease 86400
    EOF
      # losos.leases=mac@ip,mac@ip: the simulator's own address plan, so the
      # canvas and the router agree on who has which address
      for l in $(arg losos.leases | tr , ' '); do
        echo "static_lease ''${l%@*} ''${l#*@}" >> /run/netz/udhcpd.conf
      done
      udhcpd /run/netz/udhcpd.conf
    fi
    printf '<meta charset="utf-8"><p>${osName}: %s (%s) %s up</p>\n' \
      "$HOST" "$ROLE" "''${IP:-no address}" > /tmp/www/index.html
    httpd -p 80 -h /tmp/www
    /usr/bin/netz-banner
    echo
    echo "Maintenance console. Try: show interfaces | show ip route | show arp"
    echo "                          show dhcp | show version"
    echo
  '';

  banner = pkgs.writeText "netz-banner" ''
    #!/bin/sh
    H=$(cat /run/netz/host); R=$(cat /run/netz/role)
    IP=$(ip -4 -o addr show dev eth0 2>/dev/null | awk '{print $4}')
    b() { printf '\033[44;97m %-58s \033[0m\n' "$1"; }
    b ""
    # printf pads by bytes: the umlaut is two of them, hence 59.
    printf '\033[44;97m %-59s \033[0m\n' "${osName} ${osVersion}"
    b ""
    case "$R" in
      router) b "Router $H is ready" ;;
      switch) b "Switch $H is ready" ;;
      ap)     b "Wi-Fi access point $H is ready" ;;
      *)      b "Device $H ($R) is ready" ;;
    esac
    if [ -n "$IP" ]; then
      b "  Address:  $IP"
      b "  Status:   http://''${IP%/*}"
    else
      b "  No address configured (losos.ip)."
    fi
    b ""
  '';

  show = pkgs.writeText "show" ''
    #!/bin/sh
    R=$(cat /run/netz/role 2>/dev/null)
    case "$1" in
      int*)
        printf '%-10s %-8s %-18s %s\n' Interface State MAC Address
        for d in /sys/class/net/*; do
          n=''${d##*/}
          a=$(ip -4 -o addr show dev "$n" | awk '{printf "%s ", $4}')
          printf '%-10s %-8s %-18s %s\n' "$n" "$(cat $d/operstate)" \
            "$(cat $d/address)" "''${a:--}"
        done ;;
      ip) case "$2" in ro*) ip route show ;; *) echo "usage: show ip route"; exit 2 ;; esac ;;
      arp) arp -n ;;
      dhcp)
        L=/var/lib/misc/udhcpd.leases
        if [ "$R" != router ]; then echo "No DHCP server on a $R."
        elif [ -s $L ]; then dumpleases -f $L
        else awk '/^start/{s=$2} /^end/{e=$2} END{print "No leases yet (pool " s "-" e ")."}' /run/netz/udhcpd.conf; fi ;;
      ver*) cat /etc/os-release; uname -a ;;
      *) echo "usage: show interfaces | ip route | arp | dhcp | version"; exit 2 ;;
    esac
  '';
in
pkgs.runCommand "netzgeraete-rootfs"
  {
    nativeBuildInputs = [
      pkgs.e2fsprogs
      pkgs.fakeroot
      pkgs.binutils-unwrapped
    ];
    SOURCE_DATE_EPOCH = 1;
  }
  ''
    r=$PWD/root
    mkdir -p $r/bin $r/usr/bin $r/etc/init.d \
      $r/proc $r/sys $r/dev $r/tmp $r/run $r/var $r/root $r/mnt
    install -m755 ${busybox}/bin/busybox $r/bin/busybox
    strip $r/bin/busybox
    for i in $($r/bin/busybox --list); do
      [ "$i" = busybox ] || ln -s busybox $r/bin/$i
    done
    ln -s bin $r/sbin
    ln -s ../bin $r/usr/sbin
    install -m644 ${inittab} $r/etc/inittab
    install -m644 ${profile} $r/etc/profile
    install -m644 ${osRelease} $r/etc/os-release
    install -m755 ${rcS} $r/etc/init.d/rcS
    install -m755 ${banner} $r/usr/bin/netz-banner
    install -m755 ${show} $r/usr/bin/show
    echo 'root:x:0:0:root:/root:/bin/sh' > $r/etc/passwd
    echo 'root:x:0:' > $r/etc/group
    ln -s /tmp/resolv.conf $r/etc/resolv.conf
    chmod 1777 $r/tmp
    mkdir -p $out
    fakeroot sh -c "
      chown -R 0:0 $r
      mknod -m 666 $r/dev/null c 1 3
      mknod -m 600 $r/dev/console c 5 1
      mke2fs -q -t ext4 -O ^has_journal -L netzgeraete \
        -U 6e65747a-6765-7261-6574-650000000001 \
        -E hash_seed=6e65747a-6765-7261-6574-650000000001 \
        -d $r $out/rootfs.bin 4M
    "
    e2fsck -fn $out/rootfs.bin
  ''
