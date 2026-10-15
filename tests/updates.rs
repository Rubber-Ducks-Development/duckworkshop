use base64::{Engine, engine::general_purpose::STANDARD};
use duck_workshop::updates::*;
use serde::Deserialize;

#[derive(Deserialize)]
struct Vectors {
    public_key: String,
    cases: Vec<Case>,
}
#[derive(Deserialize)]
struct Case {
    name: String,
    envelope: Envelope,
    valid: bool,
}
fn vectors() -> Vectors {
    serde_json::from_str(include_str!("fixtures/updates.json")).unwrap()
}

#[test]
fn dotnet_pss_signatures_and_invalid_signed_fields() {
    let vectors = vectors();
    let key = STANDARD.decode(vectors.public_key).unwrap();
    for case in vectors.cases {
        let bytes = serde_json::to_vec(&case.envelope).unwrap();
        assert_eq!(
            verify_manifest(&bytes, &key).is_ok(),
            case.valid,
            "{}",
            case.name
        );
    }
}

#[test]
fn wrong_key_tampered_payload_and_signature_are_rejected() {
    let vectors = vectors();
    let key = STANDARD.decode(vectors.public_key).unwrap();
    let mut envelope = vectors
        .cases
        .into_iter()
        .find(|c| c.valid)
        .unwrap()
        .envelope;
    let bytes = serde_json::to_vec(&envelope).unwrap();
    assert!(verify_manifest(&bytes, PUBLIC_KEY).is_err());
    let original = envelope.payload.clone();
    envelope.payload = STANDARD.encode(b"{}");
    assert!(verify_manifest(&serde_json::to_vec(&envelope).unwrap(), &key).is_err());
    envelope.payload = original;
    let mut signature = STANDARD.decode(&envelope.signature).unwrap();
    signature[0] ^= 1;
    envelope.signature = STANDARD.encode(signature);
    assert!(verify_manifest(&serde_json::to_vec(&envelope).unwrap(), &key).is_err());
    assert!(verify_manifest(&vec![b' '; MANIFEST_LIMIT as usize + 1], &key).is_err());
}

#[test]
fn size_checksum_and_semantic_versions_are_enforced() {
    let vectors = vectors();
    let key = STANDARD.decode(vectors.public_key).unwrap();
    let case = vectors.cases.into_iter().find(|c| c.valid).unwrap();
    let release = verify_manifest(&serde_json::to_vec(&case.envelope).unwrap(), &key).unwrap();
    let folder = tempfile::tempdir().unwrap();
    let file = folder.path().join("update.exe");
    std::fs::write(&file, b"signed fixture").unwrap();
    verify_file(&file, &release).unwrap();
    std::fs::write(&file, b"tamper fixture").unwrap();
    assert!(verify_file(&file, &release).is_err());
    std::fs::write(&file, b"short").unwrap();
    assert!(verify_file(&file, &release).is_err());
    assert!(newer("0.10.0", "0.9.0").unwrap());
    assert!(!newer("0.3.0", "0.3.0").unwrap());
    assert!(!newer("0.2.9", "0.3.0").unwrap());
}

#[test]
fn insecure_locations_and_invalid_install_destinations_are_rejected() {
    for url in [
        "http://example.com/update",
        "file:///C:/update.exe",
        "https://user:pass@example.com",
        "https://example.com/#secret",
        "not a url",
    ] {
        assert!(https_url(url).is_err(), "{url}");
    }
    assert!(https_url(PUBLIC_FEED).is_ok());
    let folder = tempfile::tempdir().unwrap();
    assert!(apply_staged(folder.path(), std::path::Path::new("relative.exe")).is_err());
    assert!(apply_staged(folder.path(), &folder.path().join("unrelated.txt")).is_err());
}

#[test]
fn release_signer_roundtrip_when_requested() {
    // Build-Release integration can point this at a real signed executable.
    if let (Ok(manifest), Ok(application)) = (
        std::env::var("DUCK_TEST_MANIFEST"),
        std::env::var("DUCK_TEST_APPLICATION"),
    ) {
        let bytes = std::fs::read(manifest).unwrap();
        let release = verify_manifest(&bytes, PUBLIC_KEY).unwrap();
        verify_file(std::path::Path::new(&application), &release).unwrap();
    }
}

#[test]
fn portable_install_and_failed_launch_rollback_when_requested() {
    let Ok(root) = std::env::var("DUCK_TEST_INSTALL_ROOT") else {
        return;
    };
    let root = std::path::Path::new(&root);
    let success = root.join("success");
    let target = success.join("Duck-Workshop.exe");
    let original = std::fs::read(&target).unwrap();
    apply_staged(&success.join("staging"), &target).unwrap();
    assert_eq!(
        std::fs::read(success.join("staging/previous.exe")).unwrap(),
        original
    );
    for _ in 0..100 {
        if target.with_extension("launched").exists() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(30));
    }
    assert!(
        target.with_extension("launched").exists(),
        "updated executable was launched"
    );

    let failure = root.join("failure");
    let target = failure.join("Duck-Workshop.exe");
    let original = std::fs::read(&target).unwrap();
    assert!(apply_staged(&failure.join("staging"), &target).is_err());
    assert_eq!(
        std::fs::read(&target).unwrap(),
        original,
        "old app restored after invalid PE launch"
    );
}
