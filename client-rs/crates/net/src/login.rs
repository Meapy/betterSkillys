//! Hello carries the account name and password RSA-encrypted (PKCS#1 v1.5) and base64-encoded,
//! with the public half of the key in the world server's `networking/RSA.cs`
//! (`Parameters.RSA_PUBLIC_KEY` in the AIR client).

use base64::Engine;
use rsa::pkcs8::DecodePublicKey;
use rsa::{Pkcs1v15Encrypt, RsaPublicKey};

const PUBLIC_KEY: &str = "-----BEGIN PUBLIC KEY-----
MIGfMA0GCSqGSIb3DQEBAQUAA4GNADCBiQKBgQDTa2VXtjKzQ8HO2hCRuXZPhezl
0HcWdO0QxUhz1b+N5xJIXjvPGYpawLnJHgVgjcTI4dqDW9sthI3hEActKdKV6Zm/
dpPMuCvgEXq1ajOcr8WEX+pDji5kr9ELH0iZjjlvgfzUiOBI6q4ba3SRYiAJFgOo
e1TCC1sDk+rDZEPcMwIDAQAB
-----END PUBLIC KEY-----";

/// The `GAME_VERSION` the world server expects in Hello (`Parameters.as`).
pub const BUILD_VERSION: &str = "1.0.0";

/// Encrypt for Hello. An empty string stays empty, as the server expects.
pub fn encrypt(text: &str) -> String {
    if text.is_empty() {
        return String::new();
    }
    let key = RsaPublicKey::from_public_key_pem(PUBLIC_KEY).expect("built-in key parses");
    let data = key
        .encrypt(&mut rand::thread_rng(), Pkcs1v15Encrypt, text.as_bytes())
        .expect("account names and passwords fit one RSA block");
    base64::engine::general_purpose::STANDARD.encode(data)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rsa::pkcs1::DecodeRsaPrivateKey;

    /// The private key from the world server's RSA.cs, to check our output decrypts there.
    const SERVER_KEY: &str = "-----BEGIN RSA PRIVATE KEY-----
MIICXgIBAAKBgQDTa2VXtjKzQ8HO2hCRuXZPhezl0HcWdO0QxUhz1b+N5xJIXjvP
GYpawLnJHgVgjcTI4dqDW9sthI3hEActKdKV6Zm/dpPMuCvgEXq1ajOcr8WEX+pD
ji5kr9ELH0iZjjlvgfzUiOBI6q4ba3SRYiAJFgOoe1TCC1sDk+rDZEPcMwIDAQAB
AoGBAKB+m81NFAoAOuVjp0Zoy0atPVxst6rFkp2zlj/RGPyJWNi1KKQcGGqyeZcS
gjR9CtEQm0gy+B0Czo33E+uWHzSrh80lvmYxeHVgPfnyKf1bfCRvYdmm5YsWnvhV
Dsif5kC8BWfH9wxdmY3Li7UC38kzcqzYAbpMhBDFMtDh/xIJAkEA6uwmAbXk3sth
9GibetDdudJDSk2Xbf10GF2aiRlfHeKCj5OPwR/3rI0RBVcuA9LAPuYgWIJHEvWa
goQmjFI6RwJBAOZjaqd8ljbmhDEsQBrIxU2IBRLND8hJlmH/dSfkfq6GaptYtLdf
o7/caVCIDdmotNsmcUfiGIM9GR55DI0GGLUCQQCNtJzIc1v3OF+B+oeu8caNjFOi
wmMRqc0Z1XyeLnu9nyB6Utxn9kyD/SPDQO80xy/HwTDJsuwEd7oX+Hb4NbGJAkAC
dfVhrJb+JyAqVkqo/pP87AMB3GbawM52ZYAe2PXxb0YcOqpTexYIqpYFYi6jsIWe
AZ8cIXIZlMF77dcQeowxAkEAgCEggW6P+y0GKQDazGQiFEbq/tmu7vw/YqTnmEbP
aaLMuU/Y0INAj0MidC1vxVhS69+ceUK9WDxuAGULPYDYNw==
-----END RSA PRIVATE KEY-----";

    #[test]
    fn server_key_decrypts_it() {
        let private = rsa::RsaPrivateKey::from_pkcs1_pem(SERVER_KEY).unwrap();
        let sealed = base64::engine::general_purpose::STANDARD.decode(encrypt("someone@example.com")).unwrap();
        let opened = private.decrypt(Pkcs1v15Encrypt, &sealed).unwrap();
        assert_eq!(opened, b"someone@example.com");
        assert_eq!(encrypt(""), "");
    }
}
