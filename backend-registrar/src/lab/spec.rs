//! What a Lab guest is: the request checked field by field, and the libvirt
//! domain XML written from the checked values.
//!
//! Everything here is pure, so the rules and the exact XML are unit-tested
//! without libvirt. Request text never reaches the XML unchecked: every field
//! is held to a narrow character set first, and every value is escaped on the
//! way into the document anyway, so a rule loosened later cannot turn into
//! markup.

use std::fmt::Write as _;
use std::path::Path;

use serde::Deserialize;

/// Longest kernel command line accepted. The Lab's longest (a router with a
/// full lease table) is a few hundred characters; Linux takes 2048 on x86.
pub const MAX_CMDLINE: usize = 1024;
/// Network cards per guest. The Lab gives each device one.
pub const MAX_NICS: usize = 4;

/// The device kinds the Lab boots, and which image each one runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Box,
    EdgeLocal,
    EdgeOfficial,
    Router,
    Switch,
    Ap,
}

impl Role {
    #[must_use]
    pub fn parse(s: &str) -> Option<Self> {
        Some(match s {
            "box" => Role::Box,
            "edge-local" => Role::EdgeLocal,
            "edge-official" => Role::EdgeOfficial,
            "router" => Role::Router,
            "switch" => Role::Switch,
            "ap" => Role::Ap,
            _ => return None,
        })
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Role::Box => "box",
            Role::EdgeLocal => "edge-local",
            Role::EdgeOfficial => "edge-official",
            Role::Router => "router",
            Role::Switch => "switch",
            Role::Ap => "ap",
        }
    }

    /// LosOS devices boot the busybox stand-in; network gear boots
    /// Netzgeräte Betriebssystem. The same split as the qemu-wasm engine.
    #[must_use]
    pub const fn image(self) -> Image {
        match self {
            Role::Box | Role::EdgeLocal | Role::EdgeOfficial => Image::Rootfs,
            Role::Router | Role::Switch | Role::Ap => Image::Gear,
        }
    }
}

/// The two root filesystems `admin-ui/lab/engine/build.sh` makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Image {
    Rootfs,
    Gear,
}

impl Image {
    #[must_use]
    pub const fn file(self) -> &'static str {
        match self {
            Image::Rootfs => "rootfs.bin",
            Image::Gear => "gear.bin",
        }
    }
}

/// The kernel every guest boots, beside the two images.
pub const KERNEL: &str = "bzImage";

/// libvirt's domain type: hardware virtualisation or plain emulation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainType {
    Kvm,
    Qemu,
}

impl DomainType {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            DomainType::Kvm => "kvm",
            DomainType::Qemu => "qemu",
        }
    }

    /// What the Lab's console header says runs the guest.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            DomainType::Kvm => "KVM via libvirt",
            DomainType::Qemu => "QEMU via libvirt (no KVM)",
        }
    }
}

/// `POST /lab/v1/guests`, as sent. Unknown fields are refused.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GuestRequest {
    /// The device's id on the canvas (`d12`).
    pub id: String,
    /// Its host name, for the domain's title only.
    pub name: String,
    pub role: String,
    pub cmdline: String,
    /// One MAC per network card.
    pub macs: Vec<String>,
}

/// A request that passed [`check`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Checked {
    pub id: String,
    pub name: String,
    pub role: Role,
    pub cmdline: String,
    pub macs: Vec<String>,
}

/// A device id: letters, digits, `-` and `_`, at most 24 of them, so the
/// domain name stays short and plain.
#[must_use]
pub fn valid_id(s: &str) -> bool {
    (1..=24).contains(&s.len())
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
}

/// A host name as the canvas makes them: letters, digits and `-`.
#[must_use]
pub fn valid_name(s: &str) -> bool {
    (1..=63).contains(&s.len()) && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
}

/// Six two-digit hex groups joined by `:`, unicast. Returned lower case.
#[must_use]
pub fn parse_mac(s: &str) -> Option<String> {
    let groups: Vec<&str> = s.split(':').collect();
    if groups.len() != 6
        || groups
            .iter()
            .any(|g| g.len() != 2 || !g.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return None;
    }
    let first = u8::from_str_radix(groups[0], 16).ok()?;
    if first & 1 == 1 {
        return None;
    }
    Some(s.to_ascii_lowercase())
}

/// A kernel command line: the characters the Lab's own lines use
/// (`losos.leases=52:54:00:4c:03:20@10.0.1.100,...`) and nothing else, so no
/// quote, angle bracket, ampersand or control character can be in it.
#[must_use]
pub fn valid_cmdline(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= MAX_CMDLINE
        && s.bytes().all(|b| {
            b.is_ascii_alphanumeric()
                || matches!(
                    b,
                    b' ' | b'=' | b'.' | b'_' | b',' | b':' | b'/' | b'@' | b'+' | b'-'
                )
        })
}

/// Check every field. The error names the field and never repeats its value.
pub fn check(req: GuestRequest) -> Result<Checked, String> {
    if !valid_id(&req.id) {
        return Err("id must be 1 to 24 letters, digits, - or _".to_string());
    }
    if !valid_name(&req.name) {
        return Err("name must be 1 to 63 letters, digits or -".to_string());
    }
    let role = Role::parse(&req.role).ok_or_else(|| {
        "role must be box, edge-local, edge-official, router, switch or ap".to_string()
    })?;
    if !valid_cmdline(&req.cmdline) {
        return Err(format!(
            "cmdline must be at most {MAX_CMDLINE} characters of letters, digits, space and = . _ , : / @ + -"
        ));
    }
    if req.macs.is_empty() || req.macs.len() > MAX_NICS {
        return Err(format!("macs must list 1 to {MAX_NICS} addresses"));
    }
    let macs = req
        .macs
        .iter()
        .map(|m| parse_mac(m))
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| "macs must be unicast addresses like 52:54:00:12:34:56".to_string())?;
    Ok(Checked {
        id: req.id,
        name: req.name,
        role,
        cmdline: req.cmdline,
        macs,
    })
}

/// One network card: a UDP tunnel between QEMU and the helper, both ends on
/// loopback. QEMU binds `qemu_port` and sends to `helper_port`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Nic {
    pub mac: String,
    pub qemu_port: u16,
    pub helper_port: u16,
}

/// Everything the domain XML says.
#[derive(Debug, Clone)]
pub struct Domain<'a> {
    pub name: &'a str,
    pub title: &'a str,
    pub domain_type: DomainType,
    pub memory_mib: u32,
    pub kernel: &'a Path,
    pub disk: &'a Path,
    pub cmdline: &'a str,
    /// The helper listens here; QEMU connects its serial port to it.
    pub serial_port: u16,
    pub nics: &'a [Nic],
}

/// XML-escape text for an element body or a single-quoted attribute.
#[must_use]
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '\'' => out.push_str("&apos;"),
            '"' => out.push_str("&quot;"),
            c => out.push(c),
        }
    }
    out
}

/// The transient domain `virsh create` starts.
///
/// No graphics, no USB, no balloon: one CPU, the kernel booted directly,
/// the image as a read-only virtio disk (every guest shares the file), the
/// serial port as a TCP client of the helper, and each NIC a UDP tunnel the
/// helper owns. The guest has no other way out: it is on no libvirt network
/// and no bridge, so it reaches only what the Lab's browser fabric hands it.
#[must_use]
pub fn domain_xml(d: &Domain<'_>) -> String {
    let mut x = String::new();
    let e = escape;
    // `write!` into a String cannot fail.
    let _ = writeln!(x, "<domain type='{}'>", d.domain_type.as_str());
    let _ = writeln!(x, "  <name>{}</name>", e(d.name));
    let _ = writeln!(x, "  <title>{}</title>", e(d.title));
    let _ = writeln!(x, "  <memory unit='MiB'>{}</memory>", d.memory_mib);
    let _ = writeln!(x, "  <vcpu>1</vcpu>");
    let _ = writeln!(x, "  <os>");
    let _ = writeln!(x, "    <type arch='x86_64' machine='pc'>hvm</type>");
    let _ = writeln!(x, "    <kernel>{}</kernel>", e(&d.kernel.to_string_lossy()));
    let _ = writeln!(x, "    <cmdline>{}</cmdline>", e(d.cmdline));
    let _ = writeln!(x, "  </os>");
    let _ = writeln!(x, "  <features><acpi/></features>");
    let _ = writeln!(x, "  <clock offset='utc'/>");
    let _ = writeln!(x, "  <on_poweroff>destroy</on_poweroff>");
    let _ = writeln!(x, "  <on_reboot>restart</on_reboot>");
    let _ = writeln!(x, "  <on_crash>destroy</on_crash>");
    let _ = writeln!(x, "  <devices>");
    let _ = writeln!(x, "    <disk type='file' device='disk'>");
    let _ = writeln!(x, "      <driver name='qemu' type='raw'/>");
    let _ = writeln!(x, "      <source file='{}'/>", e(&d.disk.to_string_lossy()));
    let _ = writeln!(x, "      <target dev='vda' bus='virtio'/>");
    let _ = writeln!(x, "      <readonly/>");
    let _ = writeln!(x, "    </disk>");
    for n in d.nics {
        let _ = writeln!(x, "    <interface type='udp'>");
        let _ = writeln!(x, "      <mac address='{}'/>", e(&n.mac));
        let _ = writeln!(
            x,
            "      <source address='127.0.0.1' port='{}'>",
            n.helper_port
        );
        let _ = writeln!(
            x,
            "        <local address='127.0.0.1' port='{}'/>",
            n.qemu_port
        );
        let _ = writeln!(x, "      </source>");
        let _ = writeln!(x, "      <model type='virtio'/>");
        let _ = writeln!(x, "    </interface>");
    }
    let _ = writeln!(x, "    <serial type='tcp'>");
    let _ = writeln!(
        x,
        "      <source mode='connect' host='127.0.0.1' service='{}'/>",
        d.serial_port
    );
    let _ = writeln!(x, "      <protocol type='raw'/>");
    let _ = writeln!(x, "      <target port='0'/>");
    let _ = writeln!(x, "    </serial>");
    let _ = writeln!(x, "    <controller type='usb' model='none'/>");
    let _ = writeln!(x, "    <memballoon model='none'/>");
    let _ = writeln!(x, "  </devices>");
    let _ = writeln!(x, "</domain>");
    x
}

#[cfg(test)]
mod tests {
    use super::*;

    fn req() -> GuestRequest {
        GuestRequest {
            id: "d3".into(),
            name: "losos".into(),
            role: "box".into(),
            cmdline: "console=ttyS0 root=/dev/vda ro losos.host=losos losos.ip=10.0.1.20/24".into(),
            macs: vec!["52:54:00:4C:03:20".into()],
        }
    }

    #[test]
    fn a_lab_request_passes_and_the_mac_is_lowered() {
        let c = check(req()).expect("valid");
        assert_eq!(c.role, Role::Box);
        assert_eq!(c.macs, vec!["52:54:00:4c:03:20".to_string()]);
    }

    #[test]
    fn a_router_line_with_leases_passes() {
        let mut r = req();
        r.role = "router".into();
        r.cmdline = "console=ttyS0 losos.dhcp=10.0.1.100-10.0.1.199 losos.leases=52:54:00:4c:03:20@10.0.1.100,52:54:00:4c:04:21@10.0.1.101".into();
        assert_eq!(check(r).expect("valid").role.image(), Image::Gear);
    }

    #[test]
    fn ids_names_and_roles_are_held_to_their_sets() {
        for bad in ["", "d 3", "d3'", "a".repeat(25).as_str(), "../x"] {
            let mut r = req();
            r.id = bad.to_string();
            assert!(check(r).is_err(), "id {bad:?}");
        }
        let mut r = req();
        r.name = "x<y".into();
        assert!(check(r).is_err());
        let mut r = req();
        r.role = "laptop".into();
        assert!(check(r).is_err());
    }

    #[test]
    fn cmdlines_with_markup_quotes_or_control_characters_are_refused() {
        for bad in [
            "",
            "a<b",
            "a'b",
            "a\"b",
            "a&b",
            "a\nb",
            "a\tb",
            "init=$(reboot)",
            &"x".repeat(MAX_CMDLINE + 1),
        ] {
            assert!(!valid_cmdline(bad), "{bad:?}");
        }
        assert!(valid_cmdline(&"x".repeat(MAX_CMDLINE)));
    }

    #[test]
    fn macs_must_be_six_hex_pairs_and_unicast() {
        assert_eq!(
            parse_mac("52:54:00:0a:01:01").as_deref(),
            Some("52:54:00:0a:01:01")
        );
        for bad in [
            "",
            "52:54:00:0a:01",
            "52:54:00:0a:01:01:02",
            "52:54:00:4c:100:20",
            "52-54-00-0a-01-01",
            "zz:54:00:0a:01:01",
            "01:00:5e:00:00:01",
            "ff:ff:ff:ff:ff:ff",
        ] {
            assert!(parse_mac(bad).is_none(), "{bad:?}");
        }
        let mut r = req();
        r.macs = vec![];
        assert!(check(r).is_err());
        let mut r = req();
        r.macs = vec!["52:54:00:00:00:01".into(); MAX_NICS + 1];
        assert!(check(r).is_err());
    }

    #[test]
    fn unknown_fields_are_refused() {
        let body = r#"{"id":"d1","name":"a","role":"box","cmdline":"x","macs":["52:54:00:00:00:01"],"xml":"<domain/>"}"#;
        assert!(serde_json::from_str::<GuestRequest>(body).is_err());
    }

    #[test]
    fn escape_covers_all_five() {
        assert_eq!(
            escape(r#"<a href="x">&'"#),
            "&lt;a href=&quot;x&quot;&gt;&amp;&apos;"
        );
    }

    #[test]
    fn domain_xml_is_exactly_this() {
        let nics = [Nic {
            mac: "52:54:00:4c:03:20".into(),
            qemu_port: 40002,
            helper_port: 40001,
        }];
        let xml = domain_xml(&Domain {
            name: "losos-lab-d3-a1b2c3",
            title: "LosOS Lab: losos",
            domain_type: DomainType::Kvm,
            memory_mib: 96,
            kernel: Path::new("/srv/lab/guest/bzImage"),
            disk: Path::new("/srv/lab/guest/rootfs.bin"),
            cmdline: "console=ttyS0 root=/dev/vda ro",
            serial_port: 40003,
            nics: &nics,
        });
        let golden = "\
<domain type='kvm'>
  <name>losos-lab-d3-a1b2c3</name>
  <title>LosOS Lab: losos</title>
  <memory unit='MiB'>96</memory>
  <vcpu>1</vcpu>
  <os>
    <type arch='x86_64' machine='pc'>hvm</type>
    <kernel>/srv/lab/guest/bzImage</kernel>
    <cmdline>console=ttyS0 root=/dev/vda ro</cmdline>
  </os>
  <features><acpi/></features>
  <clock offset='utc'/>
  <on_poweroff>destroy</on_poweroff>
  <on_reboot>restart</on_reboot>
  <on_crash>destroy</on_crash>
  <devices>
    <disk type='file' device='disk'>
      <driver name='qemu' type='raw'/>
      <source file='/srv/lab/guest/rootfs.bin'/>
      <target dev='vda' bus='virtio'/>
      <readonly/>
    </disk>
    <interface type='udp'>
      <mac address='52:54:00:4c:03:20'/>
      <source address='127.0.0.1' port='40001'>
        <local address='127.0.0.1' port='40002'/>
      </source>
      <model type='virtio'/>
    </interface>
    <serial type='tcp'>
      <source mode='connect' host='127.0.0.1' service='40003'/>
      <protocol type='raw'/>
      <target port='0'/>
    </serial>
    <controller type='usb' model='none'/>
    <memballoon model='none'/>
  </devices>
</domain>
";
        assert_eq!(xml, golden);
    }

    #[test]
    fn hostile_paths_and_titles_are_escaped_in_the_xml() {
        let xml = domain_xml(&Domain {
            name: "losos-lab-x",
            title: "a</title><devices>",
            domain_type: DomainType::Qemu,
            memory_mib: 64,
            kernel: Path::new("/tmp/it's<here>&/bzImage"),
            disk: Path::new("/tmp/'/rootfs.bin"),
            cmdline: "console=ttyS0",
            serial_port: 1,
            nics: &[],
        });
        assert!(xml.contains("<domain type='qemu'>"));
        assert!(xml.contains("<title>a&lt;/title&gt;&lt;devices&gt;</title>"));
        assert!(xml.contains("<kernel>/tmp/it&apos;s&lt;here&gt;&amp;/bzImage</kernel>"));
        assert!(xml.contains("<source file='/tmp/&apos;/rootfs.bin'/>"));
        assert_eq!(xml.matches("<devices>").count(), 1);
    }
}
