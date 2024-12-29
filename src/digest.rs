use sha256::digest;

pub fn sha256_digest(data: &Vec<u8>) -> String {
    format!("sha256:{}", digest(data))
}
