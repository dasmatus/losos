//! Against a real daemon: open, version, create a transient guest, read its
//! serial console to the shell prompt, type a command, destroy it.
//! LOSOS_VIRT_GUEST names a directory holding the Lab's `bzImage` and
//! `rootfs.bin`; then LOSOS_VIRT_SOCK (a unix socket such as
//! /run/libvirt/virtqemud-sock) and/or LOSOS_VIRT_TCP (host:port of a daemon
//! with listen_tcp) pick the transports. Each test is skipped without its
//! variables. LOSOS_VIRT_URI (default qemu:///system) and
//! LOSOS_VIRT_EMULATOR (a qemu-system-x86_64) are optional.

#![cfg(unix)]

use losos_lab_virt::blocking::Blocking;
use losos_lab_virt::{version_string, START_AUTODESTROY, START_PAUSED};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

fn guest_xml(name: &str, dir: &str, emulator: Option<&str>) -> String {
    let emulator = emulator
        .map(|e| format!("<emulator>{e}</emulator>"))
        .unwrap_or_default();
    format!(
        "<domain type='qemu'><name>{name}</name><memory unit='MiB'>96</memory><vcpu>1</vcpu>\
         <os><type arch='x86_64' machine='pc'>hvm</type><kernel>{dir}/bzImage</kernel>\
         <cmdline>console=ttyS0 root=/dev/vda ro rootwait loglevel=4 no_timer_check \
         losos.host={name} losos.role=pc</cmdline></os><on_poweroff>destroy</on_poweroff>\
         <devices>{emulator}<disk type='file' device='disk'><driver name='qemu' type='raw'/>\
         <source file='{dir}/rootfs.bin'/><target dev='vda' bus='virtio'/><readonly/></disk>\
         <serial type='pty'><target port='0'/></serial>\
         <console type='pty'><target type='serial' port='0'/></console>\
         <memballoon model='none'/></devices></domain>"
    )
}

fn ends_in_prompt(b: &[u8]) -> bool {
    // The Lab guest's busybox shell: "<host>:~# ".
    String::from_utf8_lossy(b).ends_with("~# ")
}

#[test]
fn guest_boots_to_a_shell_over_the_unix_socket() {
    let (Ok(sock), Ok(dir)) = (
        std::env::var("LOSOS_VIRT_SOCK"),
        std::env::var("LOSOS_VIRT_GUEST"),
    ) else {
        eprintln!("skipped: set LOSOS_VIRT_SOCK and LOSOS_VIRT_GUEST to run against a daemon");
        return;
    };
    let io = UnixStream::connect(&sock).expect("connect");
    io.set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    scenario(io, &dir, "unix");
}

#[test]
fn guest_boots_to_a_shell_over_tcp() {
    let (Ok(addr), Ok(dir)) = (
        std::env::var("LOSOS_VIRT_TCP"),
        std::env::var("LOSOS_VIRT_GUEST"),
    ) else {
        eprintln!("skipped: set LOSOS_VIRT_TCP and LOSOS_VIRT_GUEST to run against a daemon");
        return;
    };
    let io = TcpStream::connect(&addr).expect("connect");
    io.set_read_timeout(Some(Duration::from_millis(250)))
        .unwrap();
    io.set_nodelay(true).unwrap();
    scenario(io, &dir, "tcp");
}

fn scenario<S: Read + Write>(io: S, dir: &str, transport: &str) {
    let uri = std::env::var("LOSOS_VIRT_URI").unwrap_or_else(|_| "qemu:///system".into());
    let emulator = std::env::var("LOSOS_VIRT_EMULATOR").ok();

    let t0 = Instant::now();
    let mut c = Blocking::new(io);
    c.open(&uri).expect("open");
    let (hv, lib) = c.version().expect("version");
    eprintln!(
        "[{transport} {:>6} ms] open {uri}: hypervisor {} library {}",
        t0.elapsed().as_millis(),
        version_string(hv),
        version_string(lib)
    );

    let name = format!("lab-{transport}-{}", std::process::id());
    let dom = c
        .create(
            &guest_xml(&name, dir, emulator.as_deref()),
            START_AUTODESTROY | START_PAUSED,
        )
        .expect("create");
    eprintln!(
        "[{transport} {:>6} ms] created {} id {} uuid {}",
        t0.elapsed().as_millis(),
        dom.name,
        dom.id,
        dom.uuid_string()
    );
    assert_eq!(c.lookup(&name).expect("lookup").uuid, dom.uuid);

    // Paused until the console is attached, so it carries the whole boot.
    let console = c.open_console(&dom, 0).expect("console");
    c.resume(&dom).expect("resume");
    let boot = c
        .console_until(console, Duration::from_secs(120), ends_in_prompt)
        .unwrap_or_else(|e| {
            let seen = String::from_utf8_lossy(&c.console_take(console)).into_owned();
            panic!("no prompt: {e}\n{seen}")
        });
    eprintln!(
        "[{transport} {:>6} ms] prompt after {} console bytes: {:?}",
        t0.elapsed().as_millis(),
        boot.len(),
        String::from_utf8_lossy(&boot)
    );
    // loglevel=4 keeps the kernel quiet; the first byte on ttyS0 is the
    // guest init's banner, which a console attached late would have missed.
    assert!(
        boot.starts_with(b"\x1b[44;97m"),
        "the console must carry the guest's first byte"
    );

    c.console_send(console, b"uname -sm; echo lab-$((6*7))\r")
        .unwrap();
    let out = c
        .console_until(console, Duration::from_secs(30), |b| {
            String::from_utf8_lossy(b).contains("lab-42")
        })
        .expect("command output");
    let out = String::from_utf8_lossy(&out).into_owned();
    eprintln!(
        "[{transport} {:>6} ms] typed a command: {out:?}",
        t0.elapsed().as_millis()
    );
    assert!(out.contains("Linux"), "{out}");

    c.destroy(&dom).expect("destroy");
    eprintln!("[{transport} {:>6} ms] destroyed", t0.elapsed().as_millis());
    assert!(c.lookup(&name).is_err(), "transient domain must be gone");
    c.close().expect("close");
}
