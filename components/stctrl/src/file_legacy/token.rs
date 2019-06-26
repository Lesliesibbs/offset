use std::fs::{self, File};
use std::io::{self, Write};
use std::path::Path;

use derive_more::*;

use app::report::MoveTokenHashedReport;
use app::ser_string::{from_base64, from_string, to_base64, to_string, SerStringError};

use app::{HashResult, PublicKey, RandValue, Signature};

use toml;

#[derive(Debug, From)]
pub enum TokenFileError {
    IoError(io::Error),
    TomlDeError(toml::de::Error),
    TomlSeError(toml::ser::Error),
    SerStringError,
    ParseInconsistencyCounterError,
    ParseMoveTokenCounterError,
    ParseBalanceError,
    ParseLocalPendingDebtError,
    ParseRemotePendingDebtError,
    InvalidPublicKey,
}

/// A helper structure for serialize and deserializing Token.
#[derive(Serialize, Deserialize)]
pub struct TokenFile {
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub prefix_hash: HashResult,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub local_public_key: PublicKey,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub remote_public_key: PublicKey,
    pub inconsistency_counter: u64,
    #[serde(serialize_with = "to_string", deserialize_with = "from_string")]
    pub move_token_counter: u128,
    #[serde(serialize_with = "to_string", deserialize_with = "from_string")]
    pub balance: i128,
    #[serde(serialize_with = "to_string", deserialize_with = "from_string")]
    pub local_pending_debt: u128,
    #[serde(serialize_with = "to_string", deserialize_with = "from_string")]
    pub remote_pending_debt: u128,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub rand_nonce: RandValue,
    #[serde(serialize_with = "to_base64", deserialize_with = "from_base64")]
    pub new_token: Signature,
}

impl From<SerStringError> for TokenFileError {
    fn from(_e: SerStringError) -> Self {
        TokenFileError::SerStringError
    }
}

/// Load Token from a file
pub fn load_token_from_file(path: &Path) -> Result<MoveTokenHashedReport, TokenFileError> {
    let data = fs::read_to_string(&path)?;
    let token_file: TokenFile = toml::from_str(&data)?;

    Ok(MoveTokenHashedReport {
        prefix_hash: token_file.prefix_hash,
        local_public_key: token_file.local_public_key,
        remote_public_key: token_file.remote_public_key,
        inconsistency_counter: token_file.inconsistency_counter,
        move_token_counter: token_file.move_token_counter,
        balance: token_file.balance,
        local_pending_debt: token_file.local_pending_debt,
        remote_pending_debt: token_file.remote_pending_debt,
        rand_nonce: token_file.rand_nonce,
        new_token: token_file.new_token,
    })
}

/// Store Token to file
pub fn store_token_to_file(
    token: &MoveTokenHashedReport,
    path: &Path,
) -> Result<(), TokenFileError> {
    let MoveTokenHashedReport {
        ref prefix_hash,
        ref local_public_key,
        ref remote_public_key,
        inconsistency_counter,
        move_token_counter,
        balance,
        local_pending_debt,
        remote_pending_debt,
        ref rand_nonce,
        ref new_token,
    } = token;

    let token_file = TokenFile {
        prefix_hash: prefix_hash.clone(),
        local_public_key: local_public_key.clone(),
        remote_public_key: remote_public_key.clone(),
        inconsistency_counter: *inconsistency_counter,
        move_token_counter: *move_token_counter,
        balance: *balance,
        local_pending_debt: *local_pending_debt,
        remote_pending_debt: *remote_pending_debt,
        rand_nonce: rand_nonce.clone(),
        new_token: new_token.clone(),
    };

    let data = toml::to_string(&token_file)?;

    let mut file = File::create(path)?;
    file.write_all(&data.as_bytes())?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    use app::{
        HashResult, PublicKey, RandValue, Signature, HASH_RESULT_LEN, PUBLIC_KEY_LEN,
        RAND_VALUE_LEN, SIGNATURE_LEN,
    };

    /*
    #[test]
    fn test_token_file_basic() {
        let token_file: TokenFile = toml::from_str(
            r#"
            prefix_hash = 'prefix_hash'
            local_public_key = 'local_public_key'
            remote_public_key = 'remote_public_key'
            inconsistency_counter = 'inconsistency_counter'
            move_token_counter = 'move_token_counter'
            balance = 'balance'
            local_pending_debt = 'local_pending_debt'
            remote_pending_debt = 'remote_pending_debt'
            rand_nonce = 'rand_nonce'
            new_token = 'new_token'
        "#,
        )
        .unwrap();

        assert_eq!(token_file.prefix_hash, "prefix_hash");
        assert_eq!(token_file.local_public_key, "local_public_key");
        assert_eq!(token_file.remote_public_key, "remote_public_key");
        assert_eq!(token_file.inconsistency_counter, "inconsistency_counter");
        assert_eq!(token_file.move_token_counter, "move_token_counter");
        assert_eq!(token_file.balance, "balance");
        assert_eq!(token_file.local_pending_debt, "local_pending_debt");
        assert_eq!(token_file.remote_pending_debt, "remote_pending_debt");
        assert_eq!(token_file.rand_nonce, "rand_nonce");
        assert_eq!(token_file.new_token, "new_token");
    }
    */

    #[test]
    fn test_store_load_token() {
        // Create a temporary directory:
        let dir = tempdir().unwrap();
        let file_path = dir.path().join("token_file");

        let token = MoveTokenHashedReport {
            prefix_hash: HashResult::from(&[0; HASH_RESULT_LEN]),
            local_public_key: PublicKey::from(&[1; PUBLIC_KEY_LEN]),
            remote_public_key: PublicKey::from(&[2; PUBLIC_KEY_LEN]),
            inconsistency_counter: 8u64,
            move_token_counter: 15u128,
            balance: -1500i128,
            local_pending_debt: 10u128,
            remote_pending_debt: 20u128,
            rand_nonce: RandValue::from(&[3; RAND_VALUE_LEN]),
            new_token: Signature::from(&[4; SIGNATURE_LEN]),
        };

        store_token_to_file(&token, &file_path).unwrap();
        let token2 = load_token_from_file(&file_path).unwrap();

        assert_eq!(token, token2);
    }
}
