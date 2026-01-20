//! SSRF (Server-Side Request Forgery) protection.
//!
//! Validates that URLs and resolved IP addresses are not pointing to
//! private, internal, or reserved addresses.
use std::net::IpAddr;

/// Denied URL schemes that should never be fetched.
pub const DENIED_SCHEMES: &[&str] = &[
    "file",
    "ftp",
    "data",
    "javascript",
    "chrome",
    "about",
    "blob",
    "ws",
    "wss",
];

/// Error type for SSRF validation failures.
#[derive(Debug, Clone, thiserror::Error)]
pub enum SsrfError {
    #[error("blocked scheme: {0}")]
    BlockedScheme(String),

    #[error("blocked IP: {0} (private/reserved)")]
    BlockedIp(IpAddr),

    #[error("DNS resolution failed: {0}")]
    DnsError(String),
}

/// Check if an IP address is private, reserved, or otherwise blocked.
///
/// This covers:
/// - Loopback addresses (127.0.0.0/8, ::1)
/// - RFC 1918 private ranges (10/8, 172.16/12, 192.168/16)
/// - Link-local addresses (169.254/16, fe80::/10)
/// - Multicast addresses (224/4, ff00::/8)
/// - Unspecified addresses (0.0.0.0/8, ::)
/// - IPv6 unique local (fc00::/7)
pub fn is_private_or_reserved(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => {
            v4.is_loopback()
                || v4.is_private()
                || v4.is_link_local()
                || v4.is_multicast()
                || v4.is_broadcast()
                || v4.is_unspecified()
                || v4.octets()[0] == 0
        }
        IpAddr::V6(v6) => {
            v6.is_loopback()
                || v6.is_multicast()
                || v6.is_unspecified()
                || (v6.segments()[0] & 0xfe00) == 0xfc00
                || (v6.segments()[0] & 0xffc0) == 0xfe80
        }
    }
}

/// Validate that an IP address is not private or reserved.
///
/// Returns an error if the IP is blocked.
pub fn validate_ip(ip: IpAddr) -> Result<(), SsrfError> {
    if is_private_or_reserved(ip) { Err(SsrfError::BlockedIp(ip)) } else { Ok(()) }
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

    #[test]
    fn test_is_private_or_reserved_loopback_v4() {
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(127, 255, 255, 255))));
    }

    #[test]
    fn test_is_private_or_reserved_private_v4() {
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(172, 16, 0, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(172, 31, 255, 255))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(192, 168, 0, 1))));
    }

    #[test]
    fn test_is_private_or_reserved_link_local_v4() {
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(169, 254, 0, 1))));
    }

    #[test]
    fn test_is_private_or_reserved_multicast_v4() {
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(224, 0, 0, 1))));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(239, 255, 255, 255))));
    }

    #[test]
    fn test_is_private_or_reserved_unspecified_v4() {
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::UNSPECIFIED)));
        assert!(is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(0, 0, 0, 1))));
    }

    #[test]
    fn test_is_private_or_reserved_loopback_v6() {
        assert!(is_private_or_reserved(IpAddr::V6(Ipv6Addr::LOCALHOST)));
    }

    #[test]
    fn test_is_private_or_reserved_unique_local_v6() {
        assert!(is_private_or_reserved(IpAddr::V6(Ipv6Addr::new(
            0xfc00, 0, 0, 0, 0, 0, 0, 1
        ))));
        assert!(is_private_or_reserved(IpAddr::V6(Ipv6Addr::new(
            0xfdff, 0, 0, 0, 0, 0, 0, 1
        ))));
    }

    #[test]
    fn test_is_private_or_reserved_link_local_v6() {
        assert!(is_private_or_reserved(IpAddr::V6(Ipv6Addr::new(
            0xfe80, 0, 0, 0, 0, 0, 0, 1
        ))));
    }

    #[test]
    fn test_is_private_or_reserved_multicast_v6() {
        assert!(is_private_or_reserved(IpAddr::V6(Ipv6Addr::new(
            0xff00, 0, 0, 0, 0, 0, 0, 1
        ))));
    }

    #[test]
    fn test_is_private_or_reserved_unspecified_v6() {
        assert!(is_private_or_reserved(IpAddr::V6(Ipv6Addr::UNSPECIFIED)));
    }

    #[test]
    fn test_is_private_or_reserved_public_v4() {
        assert!(!is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))));
        assert!(!is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(1, 1, 1, 1))));
        assert!(!is_private_or_reserved(IpAddr::V4(Ipv4Addr::new(93, 184, 216, 34))));
    }

    #[test]
    fn test_is_private_or_reserved_public_v6() {
        assert!(!is_private_or_reserved(IpAddr::V6(Ipv6Addr::new(
            0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 1
        ))));
    }

    #[test]
    fn test_validate_ip_public() {
        assert!(validate_ip(IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))).is_ok());
    }

    #[test]
    fn test_validate_ip_blocked() {
        assert!(validate_ip(IpAddr::V4(Ipv4Addr::new(127, 0, 0, 1))).is_err());
        assert!(validate_ip(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 1))).is_err());
    }
}
