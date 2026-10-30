//! `losos-cloud` — LosOS Cloud, the owner's files and the people's.
//!
//! `serve` is the systemd unit (`modules/cloud.nix`); `scan` brings the
//! table in line with files changed on disk; `photos` reads the pictures'
//! tags and faces; `import` moves a Nextcloud
//! data folder's files in. The rules are in [`losos_ctl::cloud`].

use clap::{Parser, Subcommand};
use losos_ctl::cloud::{self, server, Cloud, Config};
use losos_ctl::office::actix::Sessions;
use losos_ctl::office::discovery::Coolwsd;
use losos_ctl::office::{TokenKey, Wopi};
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Parser)]
#[command(
    version,
    about = "LosOS Cloud: files, WebDAV and the Nextcloud app protocol"
)]
struct Cli {
    /// Where the files and the database are.
    #[arg(long, default_value = "/var/lib/losos-cloud")]
    root: PathBuf,
    #[command(subcommand)]
    command: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Serve on a Unix socket (the box) or a TCP address (development).
    Serve {
        #[arg(long, conflicts_with = "listen")]
        socket: Option<PathBuf>,
        #[arg(long)]
        listen: Option<String>,
        /// Believe X-Losos-Cloud-User on a TCP listener too. Never on a
        /// port anyone but a proxy can reach.
        #[arg(long)]
        trust_header: bool,
        #[arg(long, default_value_t = 4)]
        workers: usize,
        #[arg(long, default_value = "LosOS Cloud")]
        product: String,
        #[arg(long, default_value_t = 30)]
        trash_days: i64,
        /// Let people publish folders of notes as knowledge bases. On when
        /// the box shares and has joined a mesh.
        #[arg(long)]
        knowledge_bases: bool,
        /// The name other boxes reach this one by
        /// (`<name>/mesh/v1/link`), and the part after the @ in people's
        /// addresses here.
        #[arg(long, default_value = "localhost")]
        box_name: String,
        /// Share with people on other boxes and edit together across them.
        /// Without it people on this box still edit text together.
        #[arg(long)]
        mesh: bool,
        /// Accept other boxes' certificates nothing vouches for. Who a box
        /// is rests on its key either way; see `cloud::mesh::dial`.
        #[arg(long)]
        mesh_self_signed: bool,
        /// Dial a box at this address instead of `wss://<name>/mesh/v1/link`,
        /// as `<name>=<url>`. For a developer's machine.
        #[arg(long, value_name = "NAME=URL")]
        mesh_peer: Vec<String>,
        /// Open Office documents in Collabora: the address coolwsd reaches
        /// this cloud's `/wopi/files/` at, e.g. `http://127.0.0.1:8093`. On
        /// the box nginx listens there on loopback and relays to the socket;
        /// the access token is the whole authentication on that path.
        #[arg(long, value_name = "URL")]
        office_wopi: Option<String>,
        /// Collabora's discovery document: coolwsd's address, or the path
        /// of a saved copy (the box's cloud has no network to ask it on).
        #[arg(long, default_value = "http://127.0.0.1:9980")]
        coolwsd: String,
        /// Watch the files on disk and bring changes made behind the
        /// service's back into the table as they happen (the Windows share).
        #[arg(long)]
        watch: bool,
        /// The fetcher that reads subscribed calendars (`webcal-fetch`
        /// behind a socket). Without it this process runs curl itself,
        /// which on the box has no network.
        #[arg(long)]
        webcal_socket: Option<PathBuf>,
    },
    /// Read one calendar address for LosOS Cloud: a JSON request line on
    /// stdin, the answer's header line and body on stdout. The box runs it
    /// once per connection on `/run/losos-webcal/fetch.sock`, with network
    /// that the cloud itself does not have.
    WebcalFetch {
        #[arg(long, default_value = "curl")]
        curl: String,
    },
    /// Bring the table in line with the files on disk, for one person or all.
    Scan {
        #[arg(long)]
        owner: Option<String>,
    },
    /// Read the tags of every new or changed picture and, with `--models`,
    /// find the faces in it and group them into people. The box runs it on a
    /// timer at the lowest priority; without `--models` every face and
    /// person is deleted.
    Photos {
        #[arg(long)]
        owner: Option<String>,
        /// Where `yunet.onnx` and `sface.onnx` are.
        #[arg(long)]
        models: Option<PathBuf>,
    },
    /// Make an app password for a person, printed once. For the box's own
    /// tools and for tests; people get theirs through Login Flow v2.
    AppPassword {
        #[arg(long)]
        login: String,
        #[arg(long, default_value = "losos-cloud")]
        name: String,
    },
    /// Move everything out of a Nextcloud: with `--dump`, first its
    /// calendars, contacts, boards and trash bin from a dump of its database
    /// (`modules/cloud.nix` makes it), then every person's files out of its
    /// data folder (`<from>/<login>/files/`). Files already in the cloud
    /// keep their place; a name taken gets the Nextcloud copy beside it.
    Import {
        #[arg(long)]
        from: PathBuf,
        #[arg(long)]
        dump: Option<PathBuf>,
        /// Nextcloud's `config.php`, whose secret keeps its app passwords
        /// working.
        #[arg(long)]
        config: Option<PathBuf>,
        /// Write what came across, per person, as JSON here.
        #[arg(long)]
        report: Option<PathBuf>,
    },
}

fn main() -> anyhow::Result<()> {
    losos_ctl::trace::init("info");
    let cli = Cli::parse();
    match cli.command {
        Cmd::Serve {
            socket,
            listen,
            trust_header,
            workers,
            product,
            trash_days,
            knowledge_bases,
            box_name,
            mesh,
            mesh_self_signed,
            mesh_peer,
            office_wopi,
            coolwsd,
            watch,
            webcal_socket,
        } => {
            let cloud = Arc::new(Cloud::open(
                &cli.root,
                Config {
                    product,
                    trash_days,
                    knowledge_bases,
                    webcal_socket,
                },
            )?);
            if watch {
                cloud::watch::spawn(cloud.clone())?;
            }
            // Subscribed calendars, each read again when its interval is up.
            {
                let cloud = cloud.clone();
                std::thread::Builder::new()
                    .name("webcal".into())
                    .spawn(move || loop {
                        cloud::calendar::refresh_due(&cloud);
                        std::thread::sleep(std::time::Duration::from_secs(60));
                    })?;
            }
            let listen = match (socket, listen) {
                (Some(s), _) => server::Listen::Unix(s),
                (None, Some(l)) => server::Listen::Tcp(l),
                (None, None) => server::Listen::Unix("/run/losos-cloud/http.sock".into()),
            };
            // A dot first: no login starts with one, so no home can take the name.
            let mesh_dir = cli.root.join(".mesh");
            if let Some(url) = office_wopi {
                // A dot first, like .mesh: no home can take the name.
                let key = TokenKey::load_or_create(&cli.root.join(".office-token.key"))?;
                let files = Arc::new(cloud::office::CloudFiles {
                    cloud: cloud.clone(),
                });
                let _ = cloud.office.set(Arc::new(Sessions {
                    wopi: Arc::new(Wopi::new(files, key, &url)),
                    coolwsd: Coolwsd::new(&coolwsd),
                }));
            }
            actix_web::rt::System::new().block_on(async move {
                cloud::mesh::start(
                    &cloud,
                    cloud::mesh::Options {
                        name: box_name,
                        dir: mesh_dir,
                        dialer: cloud::mesh::dial::Dialer {
                            self_signed: mesh_self_signed,
                            urls: mesh_peer
                                .iter()
                                .filter_map(|p| p.split_once('='))
                                .map(|(n, u)| (n.to_string(), u.to_string()))
                                .collect(),
                        },
                        networked: mesh,
                    },
                )?;
                server::serve(
                    cloud,
                    server::Options {
                        listen,
                        trust_header,
                        workers,
                    },
                )
                .await
            })?;
            Ok(())
        }
        Cmd::WebcalFetch { curl } => {
            cloud::webcal::serve_one(&curl, std::io::stdin().lock(), std::io::stdout().lock())?;
            Ok(())
        }
        Cmd::Scan { owner } => {
            let cloud = Cloud::open(&cli.root, Config::default())?;
            let owners = match owner {
                Some(o) => vec![o],
                None => cloud.store.owners()?,
            };
            for o in owners {
                cloud.store.ensure_home(&o)?;
                let n = cloud.store.scan(&o)?;
                println!("{o}: {n} changes");
            }
            Ok(())
        }
        Cmd::Photos { owner, models } => {
            // Idle priority: waiting on the server is fine, failing is not.
            let store =
                cloud::store::Store::open_waiting(&cli.root, std::time::Duration::from_secs(300))?;
            cloud::photos::init(&store)?;
            let owners = match owner {
                Some(o) => vec![o],
                None => store.owners()?,
            };
            let faces = match &models {
                Some(dir) => cloud::photos::index::Faces::In(dir),
                None => cloud::photos::index::Faces::Off,
            };
            let r = cloud::photos::index::run(&store, faces, &owners)?;
            println!(
                "{} photos read, {} faces, {} would not open, {} people",
                r.photos, r.faces, r.broken, r.people
            );
            Ok(())
        }
        Cmd::AppPassword { login, name } => {
            let cloud = Cloud::open(&cli.root, Config::default())?;
            cloud.store.ensure_home(&login)?;
            let (_, pw) = cloud.auth.new_app_password(&cloud.store, &login, &name)?;
            println!("{pw}");
            Ok(())
        }
        Cmd::Import {
            from,
            dump,
            config,
            report,
        } => {
            let cloud = Cloud::open(&cli.root, Config::default())?;
            // The database first: should it fail, the files are still where
            // Nextcloud kept them and the next run starts over.
            let mut moved = match &dump {
                Some(d) => cloud::import::database(&cloud, d)?,
                None => cloud::import::Report::default(),
            };
            if let (Some(d), Some(c)) = (&dump, &config) {
                for (login, n) in cloud::import::passwords(&cloud, d, c)? {
                    moved.people.entry(login).or_default().app_passwords += n;
                }
            }
            if let Some(d) = &dump {
                for (login, n) in cloud::import::trash(&cloud, &from, d)? {
                    moved.people.entry(login).or_default().trash += n;
                }
            }
            for (login, n) in cloud::import::nextcloud(&cloud, &from)? {
                moved.people.entry(login).or_default().files += n;
            }
            cloud::import::write_notes(&cloud, &moved)?;
            for (login, m) in &moved.people {
                println!(
                    "{login}: {} files, {} in the trash, {} calendars ({} events, {} tasks), {} address books ({} contacts), {} boards ({} cards), {} app passwords, {} not moved",
                    m.files, m.trash, m.calendars, m.events, m.tasks, m.address_books, m.contacts, m.boards, m.cards, m.app_passwords, m.not_moved.len()
                );
            }
            if let Some(path) = report {
                std::fs::write(path, serde_json::to_vec_pretty(&moved)?)?;
            }
            Ok(())
        }
    }
}
