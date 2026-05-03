use std::fmt::Write;

pub(crate) fn encode_hex(bytes: &[u8]) -> String {
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        write!(&mut encoded, "{byte:02x}").expect("writing to string should not fail");
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::encode_hex;
    use sha2::{Digest, Sha256};

    #[test]
    fn encodes_sha256_output_as_lowercase_hex() {
        assert_eq!(
            encode_hex(&Sha256::digest(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
