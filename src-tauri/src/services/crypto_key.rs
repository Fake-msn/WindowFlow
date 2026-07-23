use keyring::Entry;

/// 凭据管理器中的服务名与账户名
const SERVICE: &str = "WindowFlow";
const ACCOUNT: &str = "db_encryption_key";

/// [T1] 从 Windows 凭据管理器（Credential Manager）获取数据库加密密钥。
/// 若不存在，则生成一个 256-bit 随机密钥并安全存入凭据管理器。
/// 返回 64 个十六进制字符（32 字节），供 SQLCipher `PRAGMA key = x'...'` 使用。
pub fn get_or_create_db_key() -> Result<String, String> {
    let entry = Entry::new(SERVICE, ACCOUNT)
        .map_err(|e| format!("failed to open keyring entry: {}", e))?;

    match entry.get_password() {
        Ok(key) if key.len() == 64 && key.chars().all(|c| c.is_ascii_hexdigit()) => Ok(key),
        _ => {
            let key = generate_hex_key();
            entry
                .set_password(&key)
                .map_err(|e| format!("failed to store db key in credential manager: {}", e))?;
            log::info!("[T1] Generated new 256-bit DB key and stored in Credential Manager");
            Ok(key)
        }
    }
}

/// 生成 32 字节随机密钥并编码为十六进制字符串
fn generate_hex_key() -> String {
    let mut bytes = [0u8; 32];
    getrandom::getrandom(&mut bytes).expect("OS RNG failed");
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_hex_key_format() {
        let k = generate_hex_key();
        assert_eq!(k.len(), 64);
        assert!(k.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_hex_key_unique() {
        // 两次生成的密钥应不同（极高概率）
        assert_ne!(generate_hex_key(), generate_hex_key());
    }
}
