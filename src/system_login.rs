use std::collections::HashMap;

use crate::client::LoginCredentials;
use std::fs;

/// Parses the kernel command line and extracts login credentials.
pub fn parse_kernel_cmdline(cmdline: &str) -> HashMap<String, LoginCredentials> {
    let mut credentials = HashMap::new();

    // Find the dockerlogin=... part
    for part in cmdline.split_whitespace() {
        if let Some(rest) = part.strip_prefix("dockerlogin=") {
            // Split by ';' to get multiple entries
            for entry in rest.trim_matches('"').split(';') {
                if entry.trim().is_empty() {
                    continue;
                }

                let fields: Vec<&str> = entry.split(',').collect();
                if fields.len() == 3 {
                    let mut hostname = fields[0].trim().trim_matches('"').to_string();

                    if !hostname.starts_with("https://") && !hostname.starts_with("http://") {
                        hostname = format!("https://{}", hostname);
                    }

                    let username = fields[1].trim().to_string();
                    let password = fields[2].trim().to_string();
                    credentials.insert(hostname, LoginCredentials { username, password });
                } else if fields.len() == 2 {
                    // If only username and password are provided, use registry-1.docker.io as default hostname
                    let hostname = "https://registry-1.docker.io".to_string();
                    let username = fields[0].trim().to_string();
                    let password = fields[1].trim().to_string();
                    credentials.insert(hostname, LoginCredentials { username, password });
                }
            }
        }
    }

    credentials
}

pub fn get_system_login() -> HashMap<String, LoginCredentials> {
    let cmdline = fs::read_to_string("/proc/cmdline").unwrap_or_default();
    parse_kernel_cmdline(&cmdline)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_kernel_cmdline() {
        let cmdline = "quiet splash dockerlogin=registry.tohka.us,pirates,pass;registry2.example.com,user2,pass2;";
        let creds = parse_kernel_cmdline(cmdline);
        assert_eq!(creds.len(), 2);
        assert_eq!(creds["https://registry.tohka.us"].username, "pirates");
        assert_eq!(creds["https://registry2.example.com"].password, "pass2");
    }
}
