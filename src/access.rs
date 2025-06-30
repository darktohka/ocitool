use nix::unistd::{execvp, getuid};
use std::env::args;
use std::ffi::CString;
use std::fmt::{Display, Formatter};
use std::io::ErrorKind;
use std::os::unix::net::UnixStream;
use std::process::exit;

#[derive(Debug)]
pub enum SocketAccessError {
    /// Socket file doesn't exist
    NotFound,
    /// Permission denied when trying to connect
    PermissionDenied,
    /// Other connection error
    ConnectionError(std::io::Error),
}

impl Display for SocketAccessError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            SocketAccessError::NotFound => write!(f, "Socket file not found"),
            SocketAccessError::PermissionDenied => write!(f, "Permission denied"),
            SocketAccessError::ConnectionError(e) => write!(f, "Connection error: {}", e),
        }
    }
}

impl std::error::Error for SocketAccessError {}

/// Checks if the current user has access to connect to the Unix socket
pub fn can_connect_to_socket(socket_path: &str) -> Result<(), SocketAccessError> {
    // Try to connect to the socket
    match UnixStream::connect(socket_path) {
        Ok(_) => Ok(()),
        Err(e) => match e.kind() {
            ErrorKind::PermissionDenied => Err(SocketAccessError::PermissionDenied),
            ErrorKind::NotFound => Err(SocketAccessError::NotFound),
            _ => Err(SocketAccessError::ConnectionError(e)),
        },
    }
}

pub fn ensure_socket_access(socket_path: &str) {
    let uid = getuid().as_raw();

    match can_connect_to_socket(socket_path) {
        Ok(_) => return, // Access is fine
        Err(e) => {
            if uid == 0 {
                eprintln!("Error: {}", e);
                exit(1);
            }

            // Re-execute the program with sudo
            let args: Vec<CString> = args()
                .map(|arg| CString::new(arg).expect("Argument contains null bytes"))
                .collect();

            let sudo_path = which::which("sudo").unwrap_or_else(|_| {
                eprintln!("Error: 'sudo' command not found");
                exit(1);
            });

            let actual_args = std::iter::once(CString::new(sudo_path.to_str().unwrap()).unwrap())
                .chain(args.into_iter())
                .collect::<Vec<CString>>();

            let path = CString::new(sudo_path.to_str().unwrap()).unwrap();
            execvp(&path, &actual_args).expect("Failed to re-execute with sudo");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        fs::{set_permissions, File, Permissions},
        io::Write,
        os::unix::{fs::PermissionsExt, net::UnixListener},
    };
    use tempfile::tempdir;

    #[test]
    fn test_can_connect_to_socket_success() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("test_socket.sock");
        let listener = UnixListener::bind(&socket_path).unwrap();

        // Spawn a thread to accept connections
        std::thread::spawn(move || {
            let _ = listener.accept();
        });

        let result = can_connect_to_socket(socket_path.to_str().unwrap());
        assert!(result.is_ok());
    }

    #[test]
    fn test_can_connect_to_socket_not_found() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("non_existent_socket.sock");

        let result = can_connect_to_socket(socket_path.to_str().unwrap());
        assert!(matches!(result, Err(SocketAccessError::NotFound)));
    }

    #[test]
    fn test_can_connect_to_socket_permission_denied() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("permission_denied_socket.sock");

        // Create a socket file with restricted permissions
        let _ = File::create(&socket_path).unwrap();
        set_permissions(&socket_path, Permissions::from_mode(0o000)).unwrap();

        let result = can_connect_to_socket(socket_path.to_str().unwrap());
        assert!(matches!(
            result,
            Err(SocketAccessError::PermissionDenied) | Err(SocketAccessError::ConnectionError(_))
        ));
    }

    #[test]
    fn test_can_connect_to_socket_connection_error() {
        let dir = tempdir().unwrap();
        let socket_path = dir.path().join("invalid_socket.sock");

        // Create a file that is not a socket
        let mut file = File::create(&socket_path).unwrap();
        file.write_all(b"This is not a socket").unwrap();

        let result = can_connect_to_socket(socket_path.to_str().unwrap());
        assert!(matches!(result, Err(SocketAccessError::ConnectionError(_))));
    }
}
