//! What a box and an edge answer, for the pane tests and the screenshots.
//! The shapes are the web UI's (admin-ui/app/src/lib/api.ts) and the edge's
//! (backend-registrar/src/server/operator.rs).

#![allow(dead_code)]

use std::sync::Mutex;

use losos::envelope::{Answer, Transport};
use losos::problem::Result;
use serde_json::{json, Value};

const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

pub fn settings() -> Value {
    json!({
      "sharingMyStorage": false,
      "nextcloudMode": "container",
      "forgejoMode": "container",
      "hostName": "mattbox",
      "https": true,
      "gpuEnable": false,
      "apachePort": 11000,
      "proxyEnable": false,
      "clusterEnable": true,
      "shareCompute": true,
      "computeWindowStart": "23:00",
      "computeWindowEnd": "07:00",
      "hardeningApparmor": false,
      "hardeningMalloc": true,
      "hardeningNosmt": false,
      "hardeningUsbguard": false
    })
}

pub fn state() -> Value {
    json!({
      "mode": "local",
      "sharing": false
    })
}

pub fn status() -> Value {
    json!({
      "state": "idle",
      "progress": 0,
      "message": ""
    })
}

pub fn storage() -> Value {
    json!({
      "totalBytes": 500363689984_u64,
      "usedBytes": 140660178944_u64,
      "reserveBytes": 51539607552_u64
    })
}

pub fn options() -> Value {
    json!({
      "available": true,
      "version": 1,
      "options": [
        {
          "name": "hostName",
          "group": "general",
          "editor": {
            "kind": "str"
          },
          "nixType": "string",
          "description": "The name this box answers to on the local network.",
          "default": "mattbox",
          "defaultText": "\"mattbox\"",
          "current": "mattbox",
          "danger": true,
          "fixed": null,
          "readOnly": false,
          "set": "\"mattbox\""
        },
        {
          "name": "sharingMyStorage",
          "group": "general",
          "editor": {
            "kind": "bool"
          },
          "nixType": "boolean",
          "description": "Lend spare disk to the mesh.",
          "default": false,
          "defaultText": "false",
          "current": false,
          "danger": false,
          "fixed": null,
          "readOnly": false,
          "set": "false"
        },
        {
          "name": "tpm.enable",
          "group": "tpm",
          "editor": {
            "kind": "bool"
          },
          "nixType": "boolean",
          "description": "Unlock the encrypted disk with the TPM.",
          "default": true,
          "defaultText": "true",
          "current": true,
          "danger": true,
          "fixed": "installer",
          "readOnly": true,
          "set": null
        },
        {
          "name": "nextcloud.apachePort",
          "group": "nextcloud",
          "editor": {
            "kind": "int",
            "min": 1024,
            "max": 65535
          },
          "nixType": "16 bit unsigned integer",
          "description": "The port the files app listens on behind the front door.",
          "default": 11000,
          "defaultText": "11000",
          "current": 11000,
          "danger": false,
          "fixed": null,
          "readOnly": false,
          "set": "11000"
        },
        {
          "name": "nextcloud.defaultPhoneRegion",
          "group": "nextcloud",
          "editor": {
            "kind": "str"
          },
          "nixType": "string",
          "description": "The country phone numbers are read in when they have no prefix.",
          "default": "SK",
          "defaultText": "\"SK\"",
          "current": "SK",
          "danger": false,
          "fixed": null,
          "readOnly": false,
          "set": null
        },
        {
          "name": "updates.extraSubstituters",
          "group": "updates",
          "editor": {
            "kind": "list"
          },
          "nixType": "list of string",
          "description": "More binary caches to fetch updates from.",
          "default": [],
          "defaultText": "[ ]",
          "current": [
            "https://cache.losos.dasmat.us"
          ],
          "danger": false,
          "fixed": null,
          "readOnly": false,
          "set": "[ \"https://cache.losos.dasmat.us\" ]"
        }
      ],
      "stray": [],
      "excluded": {}
    })
}

pub fn apps() -> Value {
    json!({
      "available": true,
      "sharedAvailable": true,
      "apps": [
        {
          "release": "jellyfin",
          "chart": {
            "repo": "https://jellyfin.github.io/jellyfin-helm",
            "name": "jellyfin",
            "version": "2.3.0"
          },
          "runAs": "notshared",
          "values": {},
          "valuesYaml": null,
          "frontPort": 31080,
          "appPort": 8096,
          "phase": "running",
          "message": null,
          "updatedAt": 1791960000
        },
        {
          "release": "vaultwarden",
          "chart": {
            "repo": "https://guerzon.github.io/vaultwarden",
            "name": "vaultwarden",
            "version": "0.31.8"
          },
          "runAs": "notshared",
          "values": {},
          "valuesYaml": null,
          "frontPort": 31081,
          "appPort": null,
          "phase": "installing",
          "message": null,
          "updatedAt": 1791960100
        }
      ]
    })
}

pub fn search() -> Value {
    json!({
      "sources": [
        "Artifact Hub"
      ],
      "results": [
        {
          "id": "immich",
          "name": "immich",
          "source": "Artifact Hub",
          "summary": "Self-hosted photo and video backup.",
          "version": "0.9.3",
          "chart": {
            "repo": "https://immich-app.github.io/immich-charts",
            "name": "immich",
            "version": "0.9.3"
          }
        },
        {
          "id": "jellyfin",
          "name": "jellyfin",
          "source": "Artifact Hub",
          "summary": "The free software media system.",
          "version": "2.3.0",
          "chart": {
            "repo": "https://jellyfin.github.io/jellyfin-helm",
            "name": "jellyfin",
            "version": "2.3.0"
          }
        }
      ]
    })
}

pub fn edge() -> Value {
    json!({
      "reachable": true,
      "official": true,
      "edges": [
        {
          "name": "register.losos.dasmat.us",
          "official": true,
          "url": "https://register.losos.dasmat.us",
          "source": "configured"
        }
      ],
      "lanSearched": true,
      "configuredUrl": "https://register.losos.dasmat.us",
      "checkedAt": 1791960000
    })
}

pub fn config() -> Value {
    json!({
      "enabled": true,
      "repository": {
        "owner": "matus",
        "name": "losos-config",
        "url": "/forgejo/matus/losos-config",
        "clone": "http://127.0.0.1:3000/matus/losos-config.git",
        "viaForgejo": true
      },
      "head": {
        "sha": "4f2a9c1e0b7d",
        "branch": "main"
      },
      "log": [
        {
          "sha": "4f2a9c1e0b7d",
          "when": "2026-10-13T21:04:00Z",
          "subject": "Lend compute 23:00 to 07:00"
        },
        {
          "sha": "b81d3e5f2c90",
          "when": "2026-10-12T18:30:00Z",
          "subject": "Turn on hardened malloc"
        },
        {
          "sha": "09ac77d1e4b2",
          "when": "2026-10-10T09:12:00Z",
          "subject": "Install the box"
        }
      ],
      "sync": {
        "state": "ok",
        "detail": "Up to date with LosOS Git.",
        "syncedAt": "2026-10-14T08:00:00Z",
        "remoteHead": "4f2a9c1e0b7d"
      },
      "upstream": {
        "url": "https://github.com/dasmatus/losos",
        "branch": "main",
        "report": {
          "state": "ok",
          "detail": "Following the published tree.",
          "syncedAt": "2026-10-14T03:00:00Z",
          "remoteHead": "c0ffee123456"
        }
      }
    })
}

pub fn backup() -> Value {
    json!({
      "target": {
        "endpoint": "https://s3.eu-central-1.amazonaws.com",
        "bucket": "matus-losos",
        "prefix": "mattbox",
        "region": "eu-central-1",
        "accessKeyId": "AKIAEXAMPLE7Q",
        "hasSecret": true,
        "storageClass": "standard"
      },
      "last": {
        "time": 1791932400,
        "snapshot": "a1b2c3d4",
        "files": 48213,
        "bytes": 98784247808_u64
      },
      "job": null,
      "erase": null,
      "lastErase": null,
      "graceSeconds": 600
    })
}

pub fn edge_overview() -> Value {
    json!({
      "health": true,
      "version": "0.1.0",
      "identity": {
        "official": true,
        "name": "losos edge eu-1",
        "url": "https://register.losos.dasmat.us",
        "notAfter": 1806451200
      },
      "tenants": [
        {
          "id": "mattbox",
          "hostname": "mattbox",
          "seenAgoSecs": 21,
          "via": null
        },
        {
          "id": "zuzkabox",
          "hostname": "zuzka-nas",
          "seenAgoSecs": 48,
          "via": null
        },
        {
          "id": "office-01",
          "hostname": "office-01",
          "seenAgoSecs": 3900,
          "via": null
        }
      ],
      "enrolled": 1,
      "routes": 2,
      "domains": 1,
      "optIns": {
        "mesh": true,
        "market": true,
        "builder": false,
        "domains": true,
        "enrolment": false,
        "uplink": false
      }
    })
}

pub fn edge_secrets() -> Value {
    json!([
      {
        "name": "stripe-secret-key",
        "sealed": true,
        "updated": 1791835980
      },
      {
        "name": "stripe-webhook-secret",
        "sealed": true,
        "updated": 1791836040
      },
      {
        "name": "claude-api-key",
        "sealed": false,
        "updated": null
      }
    ])
}

pub fn edge_releaser() -> Value {
    json!({
      "address": "0x1111111111111111111111111111111111111111",
      "networks": [
        {
          "network": "base-sepolia",
          "escrow": "0xe5c0000000000000000000000000000000000001",
          "balanceWei": "12500000000000000",
          "escrowAgrees": true,
          "error": null
        },
        {
          "network": "arbitrum-sepolia",
          "escrow": "0xe5c0000000000000000000000000000000000002",
          "balanceWei": "0",
          "escrowAgrees": false,
          "error": null
        },
        {
          "network": "optimism-sepolia",
          "escrow": "0xe5c0000000000000000000000000000000000003",
          "balanceWei": null,
          "escrowAgrees": null,
          "error": "the network's endpoint is not sealed yet"
        }
      ]
    })
}

/// The box's routes.
pub fn box_route(method: &str, path: &str, body: &Value) -> (u16, Value) {
    let p = path.split('?').next().unwrap_or(path);
    match (method, p) {
        ("GET", "/api/health") => (200, json!({ "ok": true })),
        ("GET", "/api/state") => (200, state()),
        ("GET", "/api/settings") => (200, settings()),
        ("GET", "/api/status") => (200, status()),
        ("GET", "/api/storage") => (200, storage()),
        ("GET", "/api/options") => (200, options()),
        ("GET", "/api/apps") => (200, apps()),
        ("GET", "/api/apps/search") => (200, search()),
        ("GET", "/api/apps/chart") => (
            200,
            json!({ "chart": search()["results"][0]["chart"], "release": "immich", "valuesYaml": "", "values": {}, "schema": null }),
        ),
        ("GET", "/api/edge") => (200, edge()),
        ("GET", "/api/config") => (200, config()),
        ("GET", "/api/backup") => (200, backup()),
        ("POST", "/api/apply") => match body.as_str() {
            Some(text) if text.contains("losos.hostName") => (200, json!({ "job": "job-1" })),
            _ => (400, json!({ "error": "not overrides.nix" })),
        },
        ("POST", "/api/grow") => (
            200,
            json!({ "grew": true, "beforeBytes": 466.0 * GIB, "afterBytes": 514.0 * GIB, "claimedBytes": 48.0 * GIB }),
        ),
        ("POST", "/api/apps/install") => {
            let mut app = apps()["apps"][0].clone();
            app["release"] = body["release"].clone();
            app["phase"] = json!("installing");
            (200, app)
        }
        ("POST", "/api/apps/remove") => {
            let mut app = apps()["apps"][0].clone();
            app["phase"] = json!("removing");
            (200, app)
        }
        ("POST", "/api/backup/run") => (200, json!({ "job": "backup-1" })),
        ("POST", "/api/config/sync") => (200, config()),
        ("GET" | "POST", _) => (404, json!({ "error": format!("no route {p}") })),
        _ => (405, json!({ "error": "method" })),
    }
}

/// The edge's routes.
pub fn edge_route(method: &str, path: &str, body: &Value) -> (u16, Value) {
    match (method, path) {
        ("GET", "/overview") => (200, edge_overview()),
        ("GET", "/secrets") => (200, edge_secrets()),
        ("POST", "/secrets") => {
            let (Some(name), Some(_)) = (body["name"].as_str(), body["value"].as_str()) else {
                return (400, json!({ "error": "name and value" }));
            };
            if !edge_secrets()
                .as_array()
                .unwrap()
                .iter()
                .any(|s| s["name"] == name)
            {
                return (404, json!({ "error": format!("no secret {name}") }));
            }
            (200, json!({ "sealed": true, "name": name }))
        }
        ("POST", "/reconcile") => (200, json!({ "reconciled": true })),
        ("GET", "/escrow/releaser") => (200, edge_releaser()),
        ("POST", "/escrow/releaser") => {
            if body["replace"] != json!(true) {
                return (
                    409,
                    json!({ "error": "this edge already has a releaser key; pass replace to make a new one" }),
                );
            }
            (
                200,
                json!({ "address": "0x2222222222222222222222222222222222222222", "replaced": true, "report": "usdc-releaser-key: sealed" }),
            )
        }
        _ => (404, json!({ "error": format!("no route {path}") })),
    }
}

/// A transport that answers from a route table and records every call.
pub struct Fixture {
    route: fn(&str, &str, &Value) -> (u16, Value),
    pub calls: Mutex<Vec<(String, String, Value)>>,
}

impl Fixture {
    pub fn new(route: fn(&str, &str, &Value) -> (u16, Value)) -> Fixture {
        Fixture {
            route,
            calls: Mutex::new(Vec::new()),
        }
    }

    pub fn calls(&self) -> Vec<(String, String, Value)> {
        self.calls.lock().unwrap().clone()
    }
}

impl Transport for Fixture {
    fn request(&self, method: &str, path: &str, body: Value) -> Result<Answer> {
        self.calls
            .lock()
            .unwrap()
            .push((method.to_string(), path.to_string(), body.clone()));
        let (status, body) = (self.route)(method, path, &body);
        Ok(Answer {
            status,
            body,
            sealed: true,
        })
    }
}
