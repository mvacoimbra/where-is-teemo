use rcgen::{
    BasicConstraints, CertificateParams, DnType, ExtendedKeyUsagePurpose, IsCa, Issuer, KeyPair,
    KeyUsagePurpose,
};
use std::fs;
use std::path::{Path, PathBuf};

pub struct CaCert {
    pub cert_pem: String,
    pub key_pem: String,
}

pub struct ServerCert {
    pub cert_pem: String,
    pub key_pem: String,
}

fn certs_dir(app_data_dir: &Path) -> PathBuf {
    app_data_dir.join("certs")
}

fn ca_cert_path(app_data_dir: &Path) -> PathBuf {
    certs_dir(app_data_dir).join("ca.pem")
}

fn ca_key_path(app_data_dir: &Path) -> PathBuf {
    certs_dir(app_data_dir).join("ca-key.pem")
}

fn server_cert_path(app_data_dir: &Path) -> PathBuf {
    certs_dir(app_data_dir).join("server.pem")
}

fn server_key_path(app_data_dir: &Path) -> PathBuf {
    certs_dir(app_data_dir).join("server-key.pem")
}

/// Load existing CA from disk or generate a new one.
pub fn ensure_ca(app_data_dir: &Path) -> Result<CaCert, String> {
    let cert_path = ca_cert_path(app_data_dir);
    let key_path = ca_key_path(app_data_dir);

    if cert_path.exists() && key_path.exists() {
        log::info!("Loading existing CA from {:?}", certs_dir(app_data_dir));
        let cert_pem =
            fs::read_to_string(&cert_path).map_err(|e| format!("Failed to read CA cert: {e}"))?;
        let key_pem =
            fs::read_to_string(&key_path).map_err(|e| format!("Failed to read CA key: {e}"))?;
        return Ok(CaCert { cert_pem, key_pem });
    }

    log::info!("Generating new CA certificate");
    let ca = generate_ca()?;
    let dir = certs_dir(app_data_dir);
    fs::create_dir_all(&dir).map_err(|e| format!("Failed to create certs dir: {e}"))?;
    fs::write(&cert_path, &ca.cert_pem).map_err(|e| format!("Failed to write CA cert: {e}"))?;
    fs::write(&key_path, &ca.key_pem).map_err(|e| format!("Failed to write CA key: {e}"))?;

    Ok(ca)
}

fn generate_ca() -> Result<CaCert, String> {
    let mut params = CertificateParams::default();
    params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
    params
        .distinguished_name
        .push(DnType::CommonName, "Where Is Teemo CA");
    params
        .distinguished_name
        .push(DnType::OrganizationName, "Where Is Teemo");
    params.key_usages.push(KeyUsagePurpose::KeyCertSign);
    params.key_usages.push(KeyUsagePurpose::CrlSign);

    let key_pair = KeyPair::generate().map_err(|e| format!("Failed to generate CA key: {e}"))?;
    let cert = params
        .self_signed(&key_pair)
        .map_err(|e| format!("Failed to self-sign CA: {e}"))?;

    Ok(CaCert {
        cert_pem: cert.pem(),
        key_pem: key_pair.serialize_pem(),
    })
}

/// Generate a server certificate signed by the CA, for localhost proxy use.
pub fn generate_server_cert(ca: &CaCert, app_data_dir: &Path) -> Result<ServerCert, String> {
    let cert_path = server_cert_path(app_data_dir);
    let key_path = server_key_path(app_data_dir);

    // CertificateParams::new() auto-detects IP vs DNS SANs from strings
    let mut params = CertificateParams::new(vec![
        "127.0.0.1".to_string(),
        "localhost".to_string(),
    ])
    .map_err(|e| format!("Failed to create server cert params: {e}"))?;

    params
        .distinguished_name
        .push(DnType::CommonName, "Where Is Teemo Proxy");
    params
        .extended_key_usages
        .push(ExtendedKeyUsagePurpose::ServerAuth);

    let server_key =
        KeyPair::generate().map_err(|e| format!("Failed to generate server key: {e}"))?;

    let ca_key = KeyPair::from_pem(&ca.key_pem)
        .map_err(|e| format!("Failed to parse CA key: {e}"))?;

    let issuer = Issuer::from_ca_cert_pem(&ca.cert_pem, ca_key)
        .map_err(|e| format!("Failed to create issuer from CA: {e}"))?;

    let server_cert = params
        .signed_by(&server_key, &issuer)
        .map_err(|e| format!("Failed to sign server cert: {e}"))?;

    let server = ServerCert {
        cert_pem: server_cert.pem(),
        key_pem: server_key.serialize_pem(),
    };

    fs::write(&cert_path, &server.cert_pem)
        .map_err(|e| format!("Failed to write server cert: {e}"))?;
    fs::write(&key_path, &server.key_pem)
        .map_err(|e| format!("Failed to write server key: {e}"))?;

    log::info!("Server certificate generated for 127.0.0.1/localhost");
    Ok(server)
}

/// Check if the CA is already installed in the system trust store.
pub fn is_ca_installed(app_data_dir: &Path) -> bool {
    let cert_path = ca_cert_path(app_data_dir);
    if !cert_path.exists() {
        return false;
    }

    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("security")
            .args([
                "find-certificate",
                "-c",
                "Where Is Teemo CA",
                "/Library/Keychains/System.keychain",
            ])
            .output();

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    #[cfg(target_os = "windows")]
    {
        let output = std::process::Command::new("certutil")
            .args(["-user", "-verifystore", "Root", "Where Is Teemo CA"])
            .output();

        match output {
            Ok(o) => o.status.success(),
            Err(_) => false,
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        false
    }
}

/// Install the CA certificate in the OS trust store.
pub fn install_ca_system(app_data_dir: &Path) -> Result<(), String> {
    let cert_path = ca_cert_path(app_data_dir);
    if !cert_path.exists() {
        return Err("CA certificate not found. Run ensure_ca() first.".to_string());
    }

    if is_ca_installed(app_data_dir) {
        log::info!("CA already installed in system trust store");
        return Ok(());
    }

    let cert_path_str = cert_path.to_str().ok_or("Invalid cert path encoding")?;

    #[cfg(target_os = "macos")]
    {
        log::info!("Installing CA in macOS System Keychain (will prompt for admin)");
        let script = format!(
            r#"do shell script "security add-trusted-cert -d -r trustRoot -k /Library/Keychains/System.keychain '{}'" with administrator privileges"#,
            cert_path_str
        );
        let output = std::process::Command::new("osascript")
            .args(["-e", &script])
            .output()
            .map_err(|e| format!("Failed to run osascript: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to install CA: {stderr}"));
        }
    }

    #[cfg(target_os = "windows")]
    {
        log::info!("Installing CA in Windows user certificate store");
        let output = std::process::Command::new("certutil")
            .args(["-addstore", "-user", "Root", cert_path_str])
            .output()
            .map_err(|e| format!("Failed to run certutil: {e}"))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("Failed to install CA: {stderr}"));
        }
    }

    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    {
        return Err("Unsupported OS for CA installation".to_string());
    }

    log::info!("CA certificate installed successfully");
    Ok(())
}
