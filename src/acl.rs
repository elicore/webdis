use crate::config::AclConfig;
use std::net::IpAddr;

pub struct Acl {
    rules: Vec<AclRule>,
}

struct AclRule {
    ip_subnet: Option<ipnet::IpNet>,
    // basic_auth: Option<String>, // TODO: Implement Basic Auth
    enabled: Vec<String>,
    disabled: Vec<String>,
}

impl Acl {
    pub fn new(config: Option<Vec<AclConfig>>) -> Self {
        let mut rules = Vec::new();
        if let Some(configs) = config {
            for c in configs {
                let ip_subnet = c.ip.and_then(|ip| ip.parse().ok());
                rules.push(AclRule {
                    ip_subnet,
                    enabled: c.enabled.unwrap_or_default(),
                    disabled: c.disabled.unwrap_or_default(),
                });
            }
        }
        Self { rules }
    }

    pub fn check(&self, ip: IpAddr, command: &str) -> bool {
        if self.rules.is_empty() {
            return true; // No ACLs means everything is allowed (default)
        }

        let mut allowed = true; // Default to allowed if no rules match? Or deny?
                                // Webdis logic: ACLs are interpreted in order, later authorizations superseding earlier ones.
                                // "All commands being enabled by default"

        for rule in &self.rules {
            let ip_match = rule.ip_subnet.map(|net| net.contains(&ip)).unwrap_or(true);

            if ip_match {
                // Check disabled first
                for disabled_cmd in &rule.disabled {
                    if disabled_cmd == "*" || disabled_cmd.eq_ignore_ascii_case(command) {
                        allowed = false;
                    }
                }

                // Check enabled (supersedes disabled)
                for enabled_cmd in &rule.enabled {
                    if enabled_cmd == "*" || enabled_cmd.eq_ignore_ascii_case(command) {
                        allowed = true;
                    }
                }
            }
        }

        allowed
    }
}
