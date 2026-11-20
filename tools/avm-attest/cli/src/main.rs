#![forbid(unsafe_code)]

use attestation_protocol::{Claim, Outcome, MAX_CLAIM_BYTES, SIGNATURE_BYTES};
use ed25519_dalek::{Signature, Signer, SigningKey, VerifyingKey};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::error::Error;
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::Path;
use std::process::ExitCode;
use zeroize::Zeroizing;

type Result<T> = std::result::Result<T, Box<dyn Error>>;

const USAGE: &str = "avm-attest keygen <private.seed>\n\
avm-attest sign <private.seed> <owner/repo> <40-hex-commit> <report-sdk> <pass|fail> <report>\n\
avm-attest verify <trusted-pubkey-file> <attestation.json> <report>";

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Envelope {
    signer: String,
    signature: String,
    payload: String,
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("avm-attest: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.as_slice() {
        [help] if help == "--help" || help == "-h" => println!("{USAGE}"),
        [command, path] if command == "keygen" => generate_key(path)?,
        [command, key, repo, commit, sdk, outcome, report] if command == "sign" => {
            let outcome = Outcome::from_label(outcome).ok_or("outcome must be pass or fail")?;
            let claim = Claim::new(repo, decode(commit)?, sdk, outcome, report_digest(report)?)
                .ok_or("invalid repository, commit or report SDK")?;
            write_json(&sign(key, claim)?)?;
        }
        [command, pubkey, envelope, report] if command == "verify" => {
            write_json(&verify(pubkey, envelope, report)?)?;
        }
        _ => return Err(USAGE.into()),
    }
    Ok(())
}

fn generate_key(path: &str) -> Result<()> {
    let mut seed = Zeroizing::new([0u8; 32]);
    getrandom::fill(seed.as_mut())
        .map_err(|error| format!("OS randomness unavailable: {error}"))?;
    let signing_key = SigningKey::from_bytes(&seed);
    save_seed(Path::new(path), &seed)?;
    println!("{}", hex::encode(signing_key.verifying_key().to_bytes()));
    Ok(())
}

#[cfg(unix)]
fn save_seed(path: &Path, seed: &[u8; 32]) -> Result<()> {
    use std::{fs::OpenOptions, os::unix::fs::OpenOptionsExt};
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)?;
    file.write_all(seed)?;
    file.sync_all()?;
    Ok(())
}

#[cfg(not(unix))]
fn save_seed(_: &Path, _: &[u8; 32]) -> Result<()> {
    Err("generate keys on Linux or WSL".into())
}

fn sign(key_path: &str, claim: Claim<'_>) -> Result<Envelope> {
    let seed = Zeroizing::new(read_bounded(key_path, 32)?);
    let key = SigningKey::from_bytes(seed.as_slice().try_into()?);
    let mut buffer = [0; MAX_CLAIM_BYTES];
    let len = claim.encode(&mut buffer);
    let payload = &buffer[..len];
    let digest = Sha256::digest(payload);
    Ok(Envelope {
        signer: hex::encode(key.verifying_key().to_bytes()),
        signature: hex::encode(key.sign(&digest).to_bytes()),
        payload: hex::encode(payload),
    })
}

fn verify(pubkey: &str, envelope: &str, report: &str) -> Result<Value> {
    let trusted_bytes = read_bounded(pubkey, 256)?;
    let trusted_key = std::str::from_utf8(&trusted_bytes)?.trim();
    let trusted = VerifyingKey::from_bytes(&decode(trusted_key)?)?;
    let envelope: Envelope = serde_json::from_slice(&read_bounded(envelope, 4096)?)?;
    if decode::<32>(&envelope.signer)? != trusted.to_bytes() {
        return Err("untrusted signer".into());
    }
    let signature = Signature::from_bytes(&decode::<SIGNATURE_BYTES>(&envelope.signature)?);
    let payload = hex::decode(&envelope.payload)?;
    let claim = Claim::decode(&payload).ok_or("invalid canonical payload")?;
    let digest = Sha256::digest(&payload);
    trusted
        .verify_strict(&digest, &signature)
        .map_err(|_| "invalid signature")?;
    if *claim.report_sha256() != report_digest(report)? {
        return Err("report SHA-256 mismatch".into());
    }

    Ok(json!({
        "repository": claim.repository(),
        "commit": hex::encode(claim.commit()),
        "report_sdk": claim.report_sdk(),
        "outcome": claim.outcome().label(),
        "report_sha256": hex::encode(claim.report_sha256()),
        "signer": hex::encode(trusted.to_bytes()),
        "attestation_id": hex::encode(digest)
    }))
}

fn decode<const N: usize>(value: &str) -> Result<[u8; N]> {
    let mut bytes = [0; N];
    hex::decode_to_slice(value, &mut bytes)?;
    Ok(bytes)
}

fn read_bounded(path: &str, limit: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.take(limit + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > limit {
        return Err(format!("{path}: file exceeds {limit} bytes").into());
    }
    Ok(bytes)
}

fn report_digest(path: &str) -> Result<[u8; 32]> {
    let mut digest = Sha256::new();
    io::copy(&mut File::open(path)?, &mut digest)?;
    Ok(digest.finalize().into())
}

fn write_json(value: &impl Serialize) -> Result<()> {
    let mut stdout = io::stdout().lock();
    serde_json::to_writer_pretty(&mut stdout, value)?;
    writeln!(stdout)?;
    Ok(())
}
