use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::io::{self, Read};

const SHA256_ALGORITHM: &str = "SHA-256";
const SHA256_HEX_LENGTH: usize = 64;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContentIdentity {
    pub algorithm: String,
    pub digest: String,
}

impl ContentIdentity {
    pub fn from_reader(mut reader: impl Read) -> io::Result<Self> {
        let mut hasher = Sha256::new();
        let mut buffer = [0_u8; 64 * 1024];

        loop {
            let bytes_read = reader.read(&mut buffer)?;
            if bytes_read == 0 {
                break;
            }
            hasher.update(&buffer[..bytes_read]);
        }

        Ok(Self {
            algorithm: SHA256_ALGORITHM.to_owned(),
            digest: format!("{:x}", hasher.finalize()),
        })
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.algorithm != SHA256_ALGORITHM {
            return Err("Content identity algorithm must be literal SHA-256".to_owned());
        }
        if self.digest.len() != SHA256_HEX_LENGTH
            || !self
                .digest
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        {
            return Err("SHA-256 digest must be 64 lowercase hexadecimal characters".to_owned());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::ContentIdentity;
    use std::io::Cursor;

    #[test]
    fn hashes_bytes_as_literal_sha256_identity() {
        let identity =
            ContentIdentity::from_reader(Cursor::new(b"archive me")).expect("hash bytes");

        assert_eq!(identity.algorithm, "SHA-256");
        assert_eq!(
            identity.digest,
            "46a072ffae872e7f69b3a25152f8685d2762232422df6cd5ecac2787d8ab6e63"
        );
    }

    #[test]
    fn validates_only_literal_sha256_and_lowercase_hex() {
        let valid = ContentIdentity {
            algorithm: "SHA-256".to_owned(),
            digest: "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_owned(),
        };
        assert_eq!(valid.validate(), Ok(()));

        for invalid in [
            ContentIdentity {
                algorithm: "sha256".to_owned(),
                digest: valid.digest.clone(),
            },
            ContentIdentity {
                algorithm: "SHA-256".to_owned(),
                digest: "0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF"
                    .to_owned(),
            },
            ContentIdentity {
                algorithm: "SHA-256".to_owned(),
                digest: "abc".to_owned(),
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn identity_depends_on_bytes_not_a_path_or_name() {
        let original =
            ContentIdentity::from_reader(Cursor::new(b"same bytes")).expect("hash original");
        let renamed =
            ContentIdentity::from_reader(Cursor::new(b"same bytes")).expect("hash renamed");

        assert_eq!(original, renamed);
    }
}
