//! One case table for the compiler loader and the published runtime pack.
//!
//! Keeping the values here makes an accepted origin a two-consumer contract: adding a case without
//! teaching either side about it turns one of the parity tests red. This module contains data only;
//! neither implementation may call the other across the compiler/host dependency fence.

#[derive(Debug, Clone, Copy)]
pub struct OriginCase {
    pub value: &'static str,
    pub accepted: bool,
}

pub const ORIGIN_CASES: &[OriginCase] = &[
    OriginCase {
        value: "https://gitlab.com",
        accepted: true,
    },
    OriginCase {
        value: "https://gitlab.company.example:8443",
        accepted: true,
    },
    OriginCase {
        value: "https://localhost",
        accepted: true,
    },
    OriginCase {
        value: "https://127.0.0.1:443",
        accepted: true,
    },
    OriginCase {
        value: "https://[2001:db8::1]:8443",
        accepted: true,
    },
    OriginCase {
        value: "http://gitlab.company.example",
        accepted: false,
    },
    OriginCase {
        value: "https://",
        accepted: false,
    },
    OriginCase {
        value: "https://user@gitlab.company.example",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.company.example/",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.company.example/api/v4",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.company.example?path=/api/v4",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.company.example#api-v4",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.company.example@egress.example",
        accepted: false,
    },
    OriginCase {
        value: "https://{origin}.example",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab company.example",
        accepted: false,
    },
    OriginCase {
        value: "https://-gitlab.example",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab..example",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.example:",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.example:0",
        accepted: false,
    },
    OriginCase {
        value: "https://gitlab.example:65536",
        accepted: false,
    },
    OriginCase {
        value: "https://2001:db8::1",
        accepted: false,
    },
    OriginCase {
        value: "https://[2001:db8::1",
        accepted: false,
    },
    OriginCase {
        value: "https://[not-ipv6]",
        accepted: false,
    },
];
