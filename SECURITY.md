# Security

This crate talks to TypeSafe over HTTPS and holds an API key.

- Report vulnerabilities privately via [GitHub Security Advisories](https://github.com/AbdelStark/typesafe-rs/security/advisories/new) on this repository.
- Do not open a public issue for a credential leak or a way to extract keys from logs.

`Error` display redacts `Bearer` tokens and never prints request bodies. `SecretString` redacts itself in `Debug`. Do not log `ClientConfig` with a custom `Debug` that dumps the key.
