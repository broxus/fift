use anyhow::{Context as _, Result};
use everscale_crypto::ed25519;

use crate::core::*;
use crate::util::{CRC_16, CRC_32, CRC_32_C};

pub struct Crypto;

#[fift_module]
impl Crypto {
    #[cmd(name = "newkeypair", stack)]
    fn interpret_newkeypair(stack: &mut Stack) -> Result<()> {
        let secret = ed25519::SecretKey::generate(&mut rand::thread_rng());
        let public = ed25519::PublicKey::from(&secret);
        stack.push(secret.as_bytes().to_vec())?;
        stack.push(public.as_bytes().to_vec())
    }

    #[cmd(name = "priv>pub", stack)]
    fn interpret_priv_key_to_pub(stack: &mut Stack) -> Result<()> {
        let secret = pop_secret_key(stack)?;
        stack.push(ed25519::PublicKey::from(&secret).as_bytes().to_vec())
    }

    #[cmd(name = "ed25519_sign", stack)]
    fn interpret_ed25519_sign(stack: &mut Stack) -> Result<()> {
        let secret = pop_secret_key(stack)?;
        let public = ed25519::PublicKey::from(&secret);
        let data = stack.pop_bytes()?;
        let signature = secret.expand().sign_raw(&data, &public);
        stack.push(signature.to_vec())
    }

    #[cmd(name = "ed25519_chksign", stack)]
    fn interpret_ed25519_chksign(stack: &mut Stack) -> Result<()> {
        let public = pop_public_key(stack)?;
        let signature = pop_signature(stack)?;
        let data = stack.pop_bytes()?;
        stack.push_bool(public.verify_raw(&data, &signature))
    }

    #[cmd(name = "ed25519_sign_uint", stack)]
    fn interpret_ed25519_sign_uint(stack: &mut Stack) -> Result<()> {
        let secret = pop_secret_key(stack)?;
        let public = ed25519::PublicKey::from(&secret);
        let int = stack.pop_int()?;
        anyhow::ensure!(
            int.sign() != num_bigint::Sign::Minus,
            "Expected a positive number"
        );
        anyhow::ensure!(
            int.bits() <= 256,
            "Ed25519 data to be signed must fit into 256 bits"
        );
        let (_, mut data) = int.to_bytes_le();
        data.resize(32, 0);
        data.reverse();

        let signature = secret.expand().sign_raw(&data, &public);
        stack.push(signature.to_vec())
    }

    #[cmd(name = "crc16", stack)]
    fn interpret_crc16(stack: &mut Stack) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        let mut res = CRC_16.digest();
        res.update(bytes.as_slice());
        stack.push_int(res.finalize())
    }

    #[cmd(name = "crc32", stack)]
    fn interpret_crc32(stack: &mut Stack) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        let mut res = CRC_32.digest();
        res.update(bytes.as_slice());
        stack.push_int(res.finalize())
    }

    #[cmd(name = "crc32c", stack)]
    fn interpret_crc32c(stack: &mut Stack) -> Result<()> {
        let bytes = stack.pop_bytes()?;
        let mut res = CRC_32_C.digest();
        res.update(bytes.as_slice());
        stack.push_int(res.finalize())
    }
}

fn pop_secret_key(stack: &mut Stack) -> Result<ed25519::SecretKey> {
    let b = stack.pop_bytes()?;
    Ok(ed25519::SecretKey::from_bytes(
        b.as_slice().try_into().ok().context("Invalid secret key")?,
    ))
}

fn pop_public_key(stack: &mut Stack) -> Result<ed25519::PublicKey> {
    let b = stack.pop_bytes()?;
    if let Ok(b) = b.as_slice().try_into() {
        if let Some(key) = ed25519::PublicKey::from_bytes(b) {
            return Ok(key);
        }
    }
    anyhow::bail!("Invalid public key")
}

fn pop_signature(stack: &mut Stack) -> Result<[u8; 64]> {
    let b = stack.pop_bytes()?;
    b.as_slice().try_into().ok().context("Invalid signature")
}
