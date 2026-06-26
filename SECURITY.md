# Security Policy

## Supported Versions

| Version | Supported |
| --- | --- |
| latest release tag | yes |
| main branch | best effort |

## Reporting a Vulnerability

Please report security issues privately:

1. Open a [GitHub Security Advisory](https://github.com/TheHenkelmann/ragdoll/security/advisories/new) if you have access, or
2. Email the maintainer listed in the repository profile.

Do not open public issues for undisclosed vulnerabilities.

We aim to acknowledge reports within 3 business days and provide a remediation timeline when possible.

## Scope

In scope:

- Rust gateway authentication and authorization
- Python worker ingestion pipeline
- Container image and default credentials
- Data exposure through the HTTP API

Out of scope:

- Third-party model artifacts downloaded from Hugging Face
- Misconfiguration of cloud deployments outside the provided templates
