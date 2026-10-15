//! install.sh under sh, with a fake curl that serves files from a folder and
//! a fake uname, so every branch runs without the network.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

const PROGRAM: &str = "#!/bin/sh\necho 'losos 9.9.9'\n";

struct Fake {
    dir: tempfile::TempDir,
}

fn write_exe(path: &Path, text: &str) {
    fs::write(path, text).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

impl Fake {
    fn new() -> Fake {
        let dir = tempfile::tempdir().unwrap();
        let served = dir.path().join("served/main/x86_64");
        fs::create_dir_all(&served).unwrap();
        let bin = dir.path().join("bin");
        fs::create_dir_all(&bin).unwrap();
        fs::write(served.join("losos"), PROGRAM).unwrap();
        let sum = hex::encode(Sha256::digest(PROGRAM));
        fs::write(
            served.join("SHA256SUMS"),
            format!("{sum}  losos\n{}  other\n", "0".repeat(64)),
        )
        .unwrap();
        // curl -fsSL --proto '=https' --tlsv1.2 -o <file> <url>
        write_exe(
            &bin.join("curl"),
            "#!/bin/sh\n\
             for a; do last=$a; done\n\
             while [ $# -gt 0 ]; do [ \"$1\" = -o ] && out=$2; shift; done\n\
             path=${last#https://proxy.losos.dasmat.us/updates/}\n\
             echo \"$last\" >> \"$FAKE_SERVED/requests\"\n\
             [ -f \"$FAKE_SERVED/$path\" ] || exit 22\n\
             cp \"$FAKE_SERVED/$path\" \"$out\"\n",
        );
        write_exe(
            &bin.join("uname"),
            "#!/bin/sh\ncase \"$1\" in -s) echo \"${FAKE_OS:-Linux}\";; -m) echo \"${FAKE_ARCH:-x86_64}\";; esac\n",
        );
        // The suite may run as root; a fake id keeps it out of /usr/local/bin.
        write_exe(&bin.join("id"), "#!/bin/sh\necho \"${FAKE_UID:-1000}\"\n");
        write_exe(
            &bin.join("sudo"),
            "#!/bin/sh\necho sudo was called >&2\nexit 99\n",
        );
        Fake { dir }
    }

    fn served(&self) -> PathBuf {
        self.dir.path().join("served")
    }

    fn run(&self, env: &[(&str, &str)]) -> Output {
        let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("install.sh");
        let mut cmd = Command::new("sh");
        cmd.arg(script)
            .env_clear()
            .env(
                "PATH",
                format!("{}:{}", self.dir.path().join("bin").display(), real_path()),
            )
            .env("HOME", self.dir.path().join("home"))
            .env("FAKE_SERVED", self.served());
        for (k, v) in env {
            cmd.env(k, v);
        }
        cmd.output().unwrap()
    }
}

/// The fakes go first; the rest (sh, awk, sha256sum) is this machine's own,
/// wherever it keeps them.
fn real_path() -> String {
    std::env::var("PATH").unwrap_or_else(|_| "/usr/bin:/bin".into())
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn installs_into_local_bin_after_checking_the_checksum() {
    let fake = Fake::new();
    let r = fake.run(&[]);
    let (out, err) = (text(&r.stdout), text(&r.stderr));
    assert!(r.status.success(), "{err}");
    let installed = fake.dir.path().join("home/.local/bin/losos");
    assert_eq!(fs::read_to_string(installed).unwrap(), PROGRAM);
    assert!(
        out.contains("matches") && out.contains("losos 9.9.9"),
        "{out}"
    );
    assert!(out.contains("not on your PATH"), "{out}");
    assert!(!err.contains("sudo"));
    let requests = fs::read_to_string(fake.served().join("requests")).unwrap();
    assert!(requests.contains("https://proxy.losos.dasmat.us/updates/main/x86_64/SHA256SUMS\n"));
    assert!(requests.contains("https://proxy.losos.dasmat.us/updates/main/x86_64/losos\n"));
    let has_secret_tool = Command::new("sh")
        .args(["-c", "command -v secret-tool"])
        .env("PATH", real_path())
        .output()
        .unwrap()
        .status
        .success();
    if has_secret_tool {
        assert!(!out.contains("secret-tool is not installed"), "{out}");
        return;
    }
    for word in [
        "secret-tool is not installed",
        "libsecret-tools",
        "dnf install libsecret",
        "pacman -S libsecret",
    ] {
        assert!(out.contains(word), "{word}: {out}");
    }
}

#[test]
fn refuses_another_architecture_or_system_with_a_sentence() {
    let fake = Fake::new();
    let arm = fake.run(&[("FAKE_ARCH", "aarch64")]);
    assert_eq!(arm.status.code(), Some(1));
    assert!(text(&arm.stderr).contains("built for x86_64 only, and this computer is aarch64"));
    let mac = fake.run(&[("FAKE_OS", "Darwin")]);
    assert_eq!(mac.status.code(), Some(1));
    assert!(text(&mac.stderr).contains("Linux only"));
}

#[test]
fn stops_on_a_checksum_that_does_not_match() {
    let fake = Fake::new();
    fs::write(
        fake.served().join("main/x86_64/losos"),
        format!("{PROGRAM}# tampered\n"),
    )
    .unwrap();
    let r = fake.run(&[]);
    assert_eq!(r.status.code(), Some(1));
    assert!(text(&r.stderr).contains("does not match its checksum"));
    assert!(!fake.dir.path().join("home/.local/bin/losos").exists());
}

#[test]
fn takes_the_channel_from_losos_channel() {
    let fake = Fake::new();
    let r = fake.run(&[("LOSOS_CHANNEL", "nightly")]);
    assert_eq!(r.status.code(), Some(1));
    assert!(text(&r.stderr).contains("updates/nightly/x86_64/SHA256SUMS"));
}
