# The nginx guard every admin route of the front vhost carries: LAN sources
# only, never the box itself, never the mesh's pod network. Why each line is
# there is written above `lanOnly` in modules/containers.nix.
''
  if ($remote_addr = $server_addr) {
    return 403;
  }
  deny 10.42.0.0/16;
  allow 10.0.0.0/8;
  allow 172.16.0.0/12;
  allow 192.168.0.0/16;
  deny all;
''
