# Box names that wallets find

A box's name on an official edge pays the box's wallet once its owner binds
it (handbook, [Master proxy → Paid by name](https://losos.dasmat.us/in-depth/master-proxy)).
The edge serves the record from its own zone, signed by Knot. A buyer's
wallet finds it only when public DNS can validate that zone from the root:
the parent has to delegate the zone to the edge and publish the zone's DS
record. Until then the Network pane says, beside each name, that wallets do
not find it or do not trust it yet.

The zone below is a domain of its own, registered at Namecheap, so the DS
goes in through the registrar's DNSSEC page. A subdomain of a domain whose
DNS host takes no DS record for a subdomain cannot be validated.

## 1. The edge

On the edge's NixOS configuration, with `example.net` standing for the
domain:

```nix
losos.edge = {
  publicDomain = "example.net";
  dns = {
    enable = true;
    zone = "example.net"; # every tenant hostname is <box>.example.net
    nameservers = [ "ns1.example.net" "ns2.example.net" ];
    ipv4 = [ "203.0.113.7" ];
    ipv6 = [ "2001:db8::7" ];
  };
};
```

Knot answers on port 53, TCP and UDP; open both on the VPS's firewall at the
provider too. The edge needs its identity certificate
([edge identity](../edge-identity/README.md)); without one the zone carries
no box names.

## 2. Nameservers at the registrar

In Namecheap, for the domain:

1. **Advanced DNS → Personal DNS Server**: add `ns1` and `ns2`, both with
   the edge's IPv4 address. These are the glue records.
2. **Domain → Nameservers → Custom DNS**: `ns1.example.net` and
   `ns2.example.net`.

Check from any machine once the registry has it (minutes to an hour):

```sh
dig +short NS example.net            # ns1.example.net. ns2.example.net.
dig +short @ns1.example.net SOA example.net
```

## 3. The DS record

On the edge:

```sh
keymgr example.net ds
# example.net. DS 12345 13 2 9F86D0…
```

In Namecheap, **Advanced DNS → DNSSEC**, add one record from the line with
digest type `2`: key tag `12345`, algorithm `13` (ECDSA P-256 SHA-256),
digest type `2` (SHA-256), and the digest.

The DS names Knot's key-signing key, kept in `/var/lib/knot`. Knot's default
policy never rolls that key over by itself; it rolls only the zone-signing
key, which needs nothing at the registrar. If `/var/lib/knot` is lost, Knot
makes new keys and the DS has to be replaced the same way.

## 4. Check

```sh
delv @1.1.1.1 mattbox.example.net TXT   # "; fully validated" and ENS1 dnsname.ens.eth 0x…
```

Within half a minute of that, the box's Network pane reads **Wallets find
this name** beside it, and a wallet that sends to ENS names pays
`mattbox.example.net` to the box's wallet.
