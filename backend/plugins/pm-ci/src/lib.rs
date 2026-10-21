//! A pm plugin, not a lososd one: it runs Mesh CI's job containers for
//! `pm container`, with rootless Podman, as the `shared` user.
//!
//! The Mesh CI runner (`losos-ci runner`, backend/src/ci/runner/) starts and
//! stops every container of a job through pm, and pm asks this plugin what to
//! run. It is the only plugin in the runner's own pm configuration, so it is
//! the one that manages containers there. Like pm's own podman plugin it only
//! writes command lines: it runs in pm's metered WebAssembly sandbox, whose
//! one import is `log`, and pm runs nothing but [`LAUNCHERS`].
//!
//! What it adds to pm's podman plugin is what a CI job needs, and a fence:
//!
//! * It answers for Mesh CI's containers alone, the ones named `lci-…`, and
//!   for compose files named `lci-….yml`. Anything else is refused, so the
//!   runner's pm cannot touch another container of the user.
//! * Before a job's compose file goes up, LosOS's own base image is loaded
//!   from the Nix store unless Podman already has it, so a job that names no
//!   image never pulls one.
//! * A container's status is its state, then its exit code once it has
//!   exited, then its health when it has a health check: `exited 1`,
//!   `running healthy`. The runner waits on services and on container
//!   actions by it.
//! * A compose file whose name ends `.action.yml` is a container action run
//!   once: taking it down also removes the image built for it.
//!
//! Root's requests are refused, so no job runs with the whole system's
//! privilege.

wit_bindgen::generate!({ path: "wit", world: "container-plugin" });

/// Every program the plugin may have pm run.
pub const LAUNCHERS: [&str; 2] = ["podman", "podman-compose"];

/// What every Mesh CI container and compose file is named after.
pub const PREFIX: &str = "lci-";

/// The image jobs run in unless they name one, and its archive in the store,
/// as nix/packages.nix builds the plugin. Without them nothing is loaded.
const BASE_IMAGE: Option<&str> = option_env!("LOSOS_CI_BASE_IMAGE");
const BASE_ARCHIVE: Option<&str> = option_env!("LOSOS_CI_BASE_ARCHIVE");

struct MeshCi;

impl Guest for MeshCi {
    fn describe() -> Manifest {
        Manifest {
            name: "losos-ci".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            summary: "Runs LosOS Mesh CI's job containers with rootless Podman".into(),
            hooks: Vec::new(),
            grants_at_most: Vec::new(),
            source_extensions: Vec::new(),
            symbols: Vec::new(),
        }
    }

    // It classifies nothing and scans nothing: `hooks` is empty, so pm never
    // calls these, but every world still exports them.
    fn classify_command(_command: String) -> Option<Verdict> {
        None
    }

    fn scan_source(_file: SourceFile) -> Vec<Grant> {
        Vec::new()
    }

    fn launchers() -> Vec<String> {
        LAUNCHERS.map(Into::into).to_vec()
    }

    fn container_plan(
        request: ContainerRequest,
        place: ContainerPlace,
    ) -> Result<Vec<MachineStep>, String> {
        plan(request, place.rootless, BASE_IMAGE.zip(BASE_ARCHIVE))
    }

    fn launch_machine(_machine: Machine) -> Result<Option<Launch>, String> {
        Ok(None)
    }
}

export!(MeshCi);

/// The steps for `request`. `base` is the base image and its archive.
pub fn plan(
    request: ContainerRequest,
    rootless: bool,
    base: Option<(&str, &str)>,
) -> Result<Vec<MachineStep>, String> {
    if !rootless {
        return Err("Mesh CI's containers run rootless, as the shared user, never as root".into());
    }
    Ok(match request {
        ContainerRequest::List => vec![step(
            "list Mesh CI's containers",
            podman(&[
                "ps",
                "--all",
                "--filter",
                "name=^lci-",
                "--format",
                "{{.Names}}",
            ]),
        )],
        ContainerRequest::Status(name) => vec![step(
            &format!("tell the state of {}", ours(&name)?),
            podman(&[
                "inspect",
                "--type",
                "container",
                "--format",
                STATUS,
                "--",
                &name,
            ]),
        )],
        ContainerRequest::Ports(name) => vec![step(
            &format!("list the ports of {}", ours(&name)?),
            podman(&["port", "--", &name]),
        )],
        ContainerRequest::Up(file) => {
            let kind = compose_kind(&file)?;
            let mut steps = Vec::new();
            if let (Kind::Job, Some((image, archive))) = (kind, base) {
                steps.push(MachineStep {
                    summary: format!("load {image}"),
                    run: podman(&["load", "--quiet", "--input", archive]),
                    when: None,
                    unless: Some(podman(&["image", "exists", image])),
                    undo: None,
                });
            }
            steps.push(MachineStep {
                summary: format!("start what {file} describes"),
                run: compose(&file, &["up", "--detach"]),
                when: None,
                unless: None,
                undo: Some(compose(&file, &["down"])),
            });
            steps
        }
        ContainerRequest::Create(file) => {
            compose_kind(&file)?;
            vec![step(
                &format!("make what {file} describes"),
                compose(&file, &["up", "--no-start"]),
            )]
        }
        ContainerRequest::Down(file) => {
            let down: &[&str] = match compose_kind(&file)? {
                Kind::Job => &["down"],
                // Its image was built for this one run, or pulled for it.
                Kind::Action => &["down", "--rmi", "local"],
            };
            vec![step(
                &format!("remove what {file} describes"),
                compose(&file, down),
            )]
        }
        ContainerRequest::Start(name) => action("start", &name)?,
        ContainerRequest::Stop(name) => action("stop", &name)?,
        ContainerRequest::Pause(name) => action("pause", &name)?,
        ContainerRequest::Unpause(name) => action("unpause", &name)?,
        ContainerRequest::Remove(name) => vec![step(
            &format!("remove {}", ours(&name)?),
            podman(&["container", "rm", "--force", "--", &name]),
        )],
        ContainerRequest::Version => vec![
            step("tell Podman's version", podman(&["--version"])),
            step(
                "tell podman-compose's version",
                launch("podman-compose", &["--version"]),
            ),
        ],
    })
}

/// `running`, `exited 3`, `running healthy`: the state, the exit code once
/// there is one, the health when there is a check.
pub const STATUS: &str = "{{.State.Status}}\
    {{if eq .State.Status \"exited\"}} {{.State.ExitCode}}{{end}}\
    {{if .State.Health}} {{.State.Health.Status}}{{end}}";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// `lci-<job>.yml`: the job's container and its services.
    Job,
    /// `lci-<job>-a<n>.action.yml`: a container action, run once.
    Action,
}

/// `name` if it is one of Mesh CI's containers.
fn ours(name: &str) -> Result<&str, String> {
    if name.starts_with(PREFIX) {
        Ok(name)
    } else {
        Err(format!(
            "{name} is not a Mesh CI container: this plugin answers for lci-… alone"
        ))
    }
}

/// What a compose file is, by its name.
pub fn compose_kind(file: &str) -> Result<Kind, String> {
    let name = file.rsplit('/').next().unwrap_or(file);
    let refuse = || {
        format!(
            "{file} is not a Mesh CI compose file: this plugin answers for lci-….yml alone"
        )
    };
    if !name.starts_with(PREFIX) {
        return Err(refuse());
    }
    if name.ends_with(".action.yml") {
        Ok(Kind::Action)
    } else if name.ends_with(".yml") && !name[..name.len() - 4].contains('.') {
        Ok(Kind::Job)
    } else {
        Err(refuse())
    }
}

fn action(verb: &str, name: &str) -> Result<Vec<MachineStep>, String> {
    Ok(vec![step(
        &format!("{verb} {}", ours(name)?),
        podman(&["container", verb, "--", name]),
    )])
}

fn podman(args: &[&str]) -> Launch {
    launch("podman", args)
}

/// podman-compose on `file`, by the absolute path pm gave: pm runs every step
/// in a directory of its own.
fn compose(file: &str, args: &[&str]) -> Launch {
    let mut all = vec!["--file", file];
    all.extend_from_slice(args);
    launch("podman-compose", &all)
}

fn step(summary: &str, run: Launch) -> MachineStep {
    MachineStep {
        summary: summary.into(),
        run,
        when: None,
        unless: None,
        undo: None,
    }
}

fn launch(program: &str, args: &[&str]) -> Launch {
    Launch {
        program: program.into(),
        args: args.iter().map(|&arg| arg.into()).collect(),
        files: Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const BASE: Option<(&str, &str)> = Some((
        "localhost/losos-ci-base:abc",
        "/nix/store/x-docker-image-losos-ci-base.tar.gz",
    ));

    fn lines(request: ContainerRequest) -> Vec<String> {
        plan(request, true, BASE)
            .unwrap()
            .iter()
            .map(|s| {
                std::iter::once(s.run.program.as_str())
                    .chain(s.run.args.iter().map(String::as_str))
                    .collect::<Vec<_>>()
                    .join(" ")
            })
            .collect()
    }

    #[test]
    fn root_is_refused() {
        for request in [
            ContainerRequest::List,
            ContainerRequest::Start("lci-1-job".into()),
            ContainerRequest::Up("/w/lci-1.yml".into()),
        ] {
            assert!(plan(request, false, BASE).is_err());
        }
    }

    #[test]
    fn other_containers_and_files_are_refused() {
        for request in [
            ContainerRequest::Start("nextcloud".into()),
            ContainerRequest::Status("web".into()),
            ContainerRequest::Remove("losos-app-x".into()),
            ContainerRequest::Up("/home/x/compose.yml".into()),
            ContainerRequest::Down("/w/lci-1.yaml".into()),
            ContainerRequest::Up("/w/lci-1.other.yml".into()),
        ] {
            assert!(plan(request.clone(), true, BASE).is_err(), "{request:?}");
        }
    }

    #[test]
    fn a_job_loads_the_base_image_unless_it_is_there() {
        let steps = plan(ContainerRequest::Up("/w/lci-1.yml".into()), true, BASE).unwrap();
        assert_eq!(steps.len(), 2);
        assert_eq!(
            steps[0].run.args,
            [
                "load",
                "--quiet",
                "--input",
                "/nix/store/x-docker-image-losos-ci-base.tar.gz"
            ]
        );
        let unless = steps[0].unless.as_ref().unwrap();
        assert_eq!(
            unless.args,
            ["image", "exists", "localhost/losos-ci-base:abc"]
        );
        assert_eq!(
            lines(ContainerRequest::Up("/w/lci-1.yml".into()))[1],
            "podman-compose --file /w/lci-1.yml up --detach"
        );
        let undo = steps[1].undo.as_ref().unwrap();
        assert_eq!(undo.args, ["--file", "/w/lci-1.yml", "down"]);
        // Without a base image nothing is loaded.
        let bare = plan(ContainerRequest::Up("/w/lci-1.yml".into()), true, None).unwrap();
        assert_eq!(bare.len(), 1);
    }

    #[test]
    fn an_action_loads_nothing_and_leaves_no_image() {
        assert_eq!(
            lines(ContainerRequest::Up("/w/lci-1-a2.action.yml".into())),
            ["podman-compose --file /w/lci-1-a2.action.yml up --detach"]
        );
        assert_eq!(
            lines(ContainerRequest::Down("/w/lci-1-a2.action.yml".into())),
            ["podman-compose --file /w/lci-1-a2.action.yml down --rmi local"]
        );
        // The job's own image is the base image: never removed.
        assert_eq!(
            lines(ContainerRequest::Down("/w/lci-1.yml".into())),
            ["podman-compose --file /w/lci-1.yml down"]
        );
    }

    #[test]
    fn status_tells_the_exit_code_and_the_health() {
        assert_eq!(
            lines(ContainerRequest::Status("lci-1-s-db".into())),
            [format!(
                "podman inspect --type container --format {STATUS} -- lci-1-s-db"
            )]
        );
        assert!(STATUS.contains(".State.ExitCode"));
        assert!(STATUS.contains("{{if .State.Health}}"));
    }

    #[test]
    fn every_program_is_a_launcher() {
        for request in [
            ContainerRequest::List,
            ContainerRequest::Version,
            ContainerRequest::Up("/w/lci-1.yml".into()),
            ContainerRequest::Down("/w/lci-1-a1.action.yml".into()),
            ContainerRequest::Stop("lci-1-job".into()),
            ContainerRequest::Remove("lci-1-a1".into()),
        ] {
            for s in plan(request, true, BASE).unwrap() {
                assert!(LAUNCHERS.contains(&s.run.program.as_str()));
                for l in [&s.when, &s.unless, &s.undo].into_iter().flatten() {
                    assert!(LAUNCHERS.contains(&l.program.as_str()));
                }
            }
        }
    }
}
