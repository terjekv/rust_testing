use ldap3::{result::Result, LdapConn, LdapConnSettings, Scope, SearchEntry};
use native_tls::TlsConnector;
use std::env;

fn main() -> Result<()> {
    let args: Vec<String> = env::args().skip(1).collect();

    if args.is_empty() {
        println!("Usage: cargo run -- <username1>:<password1> <username2>:<password2>");
        return Ok(());
    }

    for arg in args {
        if let Some((username, password)) = arg.split_once(':') {
            // Try LDAP
            match test_ldap_credentials("ldap://localhost:3893", username, password) {
                Ok(groups) => println!("ldap : {} [OK] ({})", username, groups.join(", ")),
                Err(err) => println!("ldap : {} [Failed: {}]", username, err),
            }

            // Try LDAPS
            match test_ldap_credentials("ldaps://localhost:3894", username, password) {
                Ok(groups) => println!("ldaps: {} [OK] ({})", username, groups.join(", ")),
                Err(err) => println!("ldaps: {} [Failed: {}]", username, err),
            }
        } else {
            println!(
                "Invalid argument format: '{}'. Expected format: <username>:<password>",
                arg
            );
        }
    }

    Ok(())
}

fn test_ldap_credentials(addr: &str, username: &str, password: &str) -> Result<Vec<String>> {
    let mut ldap = if addr.starts_with("ldaps://") {
        // LDAPS - Secure connection
        let tls_connector = TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        LdapConn::with_settings(LdapConnSettings::new().set_connector(tls_connector), addr)?
    } else {
        // LDAP - Standard connection
        LdapConn::with_settings(LdapConnSettings::new(), addr)?
    };

    let bind_dn = format!("{},dc=example,dc=org", username);
    ldap.simple_bind(&bind_dn, password)?.success()?;

    // Search for groups with the service user
    let bind_dn = format!("{},dc=example,dc=org", "serviceuser");
    ldap.simple_bind(&bind_dn, "mysecret")?.success()?;

    // Search for groups
    let (rs, _res) = ldap
        .search(
            "dc=example,dc=org",          // Base DN for the search
            Scope::Subtree,               // Scope of the search
            &format!("uid={}", username), // Search filter
            vec!["memberOf"],             // Attributes to return (e.g., common name of the group)
        )?
        .success()?;

    let groups: Vec<String> = rs
        .into_iter()
        .filter_map(|entry| SearchEntry::construct(entry).attrs.get("memberOf").cloned())
        .flatten()
        .filter_map(|dn| parse_ou_from_dn(&dn))
        .collect();

    Ok(groups)
}

fn parse_ou_from_dn(dn: &str) -> Option<String> {
    dn.split(',')
        .find(|component| component.starts_with("ou=") && !component.contains("ou=groups"))
        .map(|ou_component| ou_component.replace("ou=", ""))
}
