#![no_std]
#![forbid(unsafe_code)]

const DOMAIN: &[u8] = b"avm-build-attestation/v1\0";
pub const MAX_REPO: usize = 128;
pub const MAX_SDK: usize = 64;
pub const SIGNATURE_BYTES: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Outcome {
    Pass,
    Fail,
}

impl Outcome {
    pub fn from_label(label: &str) -> Option<Self> {
        match label {
            "pass" => Some(Self::Pass),
            "fail" => Some(Self::Fail),
            _ => None,
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Claim<'a> {
    repository: &'a str,
    commit: [u8; 20],
    report_sdk: &'a str,
    outcome: Outcome,
    report_sha256: [u8; 32],
}

impl<'a> Claim<'a> {
    pub fn new(
        repository: &'a str,
        commit: [u8; 20],
        report_sdk: &'a str,
        outcome: Outcome,
        report_sha256: [u8; 32],
    ) -> Option<Self> {
        let (owner, repo) = repository.split_once('/')?;
        if repository.len() > MAX_REPO
            || !valid_name(owner)
            || !valid_name(repo)
            || report_sdk.len() > MAX_SDK
            || !valid_name(report_sdk)
            || commit == [0; 20]
        {
            return None;
        }
        Some(Self {
            repository,
            commit,
            report_sdk,
            outcome,
            report_sha256,
        })
    }

    pub fn repository(&self) -> &'a str {
        self.repository
    }

    pub fn commit(&self) -> &[u8; 20] {
        &self.commit
    }

    pub fn report_sdk(&self) -> &'a str {
        self.report_sdk
    }

    pub fn outcome(&self) -> Outcome {
        self.outcome
    }

    pub fn report_sha256(&self) -> &[u8; 32] {
        &self.report_sha256
    }

    pub fn encode(&self, out: &mut [u8; MAX_CLAIM_BYTES]) -> usize {
        let mut tail = out.as_mut_slice();
        let repo = self.repository.as_bytes();
        let sdk = self.report_sdk.as_bytes();
        let outcome = match self.outcome {
            Outcome::Pass => 1,
            Outcome::Fail => 2,
        };
        for field in [
            DOMAIN,
            &[repo.len() as u8],
            repo,
            &self.commit,
            &[sdk.len() as u8],
            sdk,
            &[outcome],
            &self.report_sha256,
        ] {
            let (head, rest) = tail.split_at_mut(field.len());
            head.copy_from_slice(field);
            tail = rest;
        }
        MAX_CLAIM_BYTES - tail.len()
    }

    pub fn decode(bytes: &'a [u8]) -> Option<Self> {
        let mut rest = bytes.strip_prefix(DOMAIN)?;
        let repository = read_name(&mut rest)?;
        let commit = take(&mut rest, 20)?.try_into().ok()?;
        let report_sdk = read_name(&mut rest)?;
        let outcome = match *take(&mut rest, 1)?.first()? {
            1 => Outcome::Pass,
            2 => Outcome::Fail,
            _ => return None,
        };
        let report_sha256 = rest.try_into().ok()?;
        Self::new(repository, commit, report_sdk, outcome, report_sha256)
    }
}

pub const MAX_CLAIM_BYTES: usize = DOMAIN.len() + 1 + MAX_REPO + 20 + 1 + MAX_SDK + 1 + 32;

fn valid_name(name: &str) -> bool {
    name.as_bytes()
        .first()
        .is_some_and(u8::is_ascii_alphanumeric)
        && name
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b"._-".contains(&b))
}

fn take<'a>(rest: &mut &'a [u8], len: usize) -> Option<&'a [u8]> {
    let (field, tail) = rest.split_at_checked(len)?;
    *rest = tail;
    Some(field)
}

fn read_name<'a>(rest: &mut &'a [u8]) -> Option<&'a str> {
    let len = take(rest, 1)?[0] as usize;
    core::str::from_utf8(take(rest, len)?).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_round_trip_and_rejects_truncation_or_trailing_bytes() {
        let claim = Claim::new("owner/project", [7; 20], "0.6.5", Outcome::Pass, [8; 32]).unwrap();
        let mut buffer = [0; MAX_CLAIM_BYTES];
        let len = claim.encode(&mut buffer);
        assert_eq!(Claim::decode(&buffer[..len]), Some(claim));
        for end in 0..len {
            assert!(Claim::decode(&buffer[..end]).is_none());
        }
        assert!(Claim::decode(&buffer[..=len]).is_none());
    }

    #[test]
    fn rejects_noncanonical_names_or_empty_commit() {
        for repo in [
            "Owner/project",
            "owner/project/extra",
            "owner/",
            "owner/../project",
        ] {
            assert!(Claim::new(repo, [7; 20], "0.6.5", Outcome::Pass, [8; 32]).is_none());
        }
        assert!(Claim::new("owner/project", [0; 20], "0.6.5", Outcome::Pass, [8; 32]).is_none());
    }
}
