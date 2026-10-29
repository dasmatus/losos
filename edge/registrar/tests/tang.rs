//! The edge's Tang keys (`losos_registrar::tang`) against Tang's own
//! `tangd` and `tangd-keygen`. Runs when `LOSOS_TEST_TANG` names a Tang
//! package (its `libexec/`), as CI's test job and devenv's `test-rust` set it:
//!
//! LOSOS_TEST_TANG=$(nix build --no-link --print-out-paths 'nixpkgs#tang^out') cargo test --test tang

use std::path::PathBuf;

use base64::Engine as _;
use losos_registrar::tang::{
    ec_thumbprint, lost_secret, sha256_hex, Request, Tang, TangError, TangOpts,
};

fn tang_package() -> Option<PathBuf> {
    match std::env::var_os("LOSOS_TEST_TANG") {
        Some(p) => Some(PathBuf::from(p)),
        None => {
            eprintln!("LOSOS_TEST_TANG is not set; skipping");
            None
        }
    }
}

fn random_hex() -> String {
    rand::random::<[u8; 32]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[tokio::test]
async fn a_box_key_answers_until_its_owner_reports_it_stolen() {
    let Some(pkg) = tang_package() else { return };
    let dir = std::env::temp_dir().join(format!("losos-tang-test-{}", random_hex()));
    let opts = TangOpts {
        dir: dir.clone(),
        tangd: pkg.join("libexec/tangd"),
        keygen: pkg.join("libexec/tangd-keygen"),
    };
    let tang = Tang::open(opts.clone()).await.unwrap();

    let claim = random_hex();
    let (id, thumbprint) = tang.create(&sha256_hex(claim.as_bytes())).await.unwrap();
    assert_eq!(id.len(), 32);
    // The key file is named by the same thumbprint tangd-keygen computes.
    assert!(dir
        .join(&id)
        .join("keys")
        .join(format!("{thumbprint}.jwk"))
        .exists());

    // The advertisement is tangd's, signed by the key the thumbprint names.
    let adv = tang.serve(&id, Request::Adv(None)).await.unwrap();
    assert_eq!(adv.status, 200);
    assert_eq!(adv.content_type.as_deref(), Some("application/jose+json"));
    let jws: serde_json::Value = serde_json::from_slice(&adv.body).unwrap();
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(jws["payload"].as_str().unwrap())
        .unwrap();
    let keys: serde_json::Value = serde_json::from_slice(&payload).unwrap();
    let verify = keys["keys"]
        .as_array()
        .unwrap()
        .iter()
        .find(|k| {
            k["key_ops"]
                .as_array()
                .unwrap()
                .iter()
                .any(|o| o == "verify")
        })
        .unwrap();
    assert_eq!(ec_thumbprint(verify).as_deref(), Some(thumbprint.as_str()));
    let by_kid = tang
        .serve(&id, Request::Adv(Some(thumbprint.clone())))
        .await
        .unwrap();
    assert_eq!(by_kid.status, 200);
    // A recovery request that is not a key is tangd's to refuse.
    let rec = tang
        .serve(&id, Request::Rec(thumbprint.clone(), b"{}".to_vec()))
        .await
        .unwrap();
    assert_eq!(rec.status, 400);

    // Only the box with the claim names the owner's hash.
    let code = "6f1c2a8e-4b3d-4f0a-9c1e-2d7b5a9e8f10";
    let lost = sha256_hex(lost_secret(code).as_bytes());
    assert!(matches!(
        tang.set_owner(&id, &random_hex(), &lost).await,
        Err(TangError::Forbidden)
    ));
    tang.set_owner(&id, &claim, &lost).await.unwrap();

    // The owner's switch finds the box by the secret alone.
    assert!(matches!(
        tang.mark(&lost_secret("0b9d7e3c-1a2f-4c5d-8e6f-7a8b9c0d1e2f"), true)
            .await,
        Err(TangError::NotFound)
    ));
    assert_eq!(tang.mark(&lost_secret(code), true).await.unwrap(), id);
    assert!(matches!(
        tang.serve(&id, Request::Adv(None)).await,
        Err(TangError::Lost)
    ));

    // A restart keeps the index: the switch still finds the box.
    let tang = Tang::open(opts).await.unwrap();
    assert_eq!(tang.mark(&lost_secret(code), false).await.unwrap(), id);
    assert_eq!(
        tang.serve(&id, Request::Adv(None)).await.unwrap().status,
        200
    );

    // A new recovery code replaces the old one's hash.
    let new_code = "0b9d7e3c-1a2f-4c5d-8e6f-7a8b9c0d1e2f";
    tang.set_owner(&id, &claim, &sha256_hex(lost_secret(new_code).as_bytes()))
        .await
        .unwrap();
    assert!(matches!(
        tang.mark(&lost_secret(code), true).await,
        Err(TangError::NotFound)
    ));
    assert_eq!(tang.mark(&lost_secret(new_code), true).await.unwrap(), id);

    // Ids that are not ours, and paths that would climb, find nothing.
    assert!(matches!(
        tang.serve("0123456789abcdef0123456789abcdef", Request::Adv(None))
            .await,
        Err(TangError::NotFound)
    ));
    assert!(matches!(
        tang.serve("../../etc", Request::Adv(None)).await,
        Err(TangError::Invalid(_))
    ));
    assert!(matches!(
        tang.serve(&id, Request::Adv(Some("../keys".into()))).await,
        Err(TangError::Invalid(_))
    ));

    let _ = std::fs::remove_dir_all(&dir);
}
