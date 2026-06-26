# IT Security Policy - Lumen Analytics GmbH

This policy is binding for all employees and describes the technical and
organizational measures that protect company and customer data. It is maintained
by the IT team and is classified as confidential. In any conflict between
convenience and security, security wins; exceptions require written approval
from the IT team.

## Passwords and Multi-Factor Authentication

Passwords must be at least twelve characters long and contain upper- and
lowercase letters, digits, and special characters. Reusing old passwords is not
allowed, and passwords must never be shared between accounts or colleagues.
Multi-factor authentication (MFA) is mandatory for all business accounts; an
authenticator app is preferred over SMS codes, because SMS can be intercepted.
Credentials may only be stored in the approved password manager, never in plain
text files, spreadsheets, or chat messages.

If you suspect that a password has been exposed - for example after a phishing
attempt or a leaked third-party service - change it immediately and report the
incident. Service accounts and API keys follow the same rules and must be
rotated at least once a year, or immediately when someone with access leaves the
team.

## Data Classification

We distinguish three protection levels. **Public** covers content that can be
published without risk, such as marketing material and the public product FAQ.
**Internal** covers documents that are only shared within the company, for
example the employee handbook and onboarding materials. **Confidential** covers
especially sensitive data such as customer data, source code, financial records,
and security documentation like this policy.

Confidential data may only be transmitted in encrypted form and must not be
stored on private devices. When in doubt about the classification of a document,
treat it as confidential until the owner confirms otherwise. Sharing
confidential data with external parties always requires a signed agreement and
approval from the data owner.

## Remote Work and Secure Mobile Working

When working outside the office, the company VPN is mandatory as soon as you
access internal systems. Public Wi-Fi networks - for example in cafes, hotels,
or train stations - may only be used through the VPN. Screens must be protected
with a privacy filter when working in public spaces, and confidential calls
should not be held where they can be overheard.

These rules complement the organizational remote-work guidance in the employee
handbook and take precedence in all security matters. Devices must never be left
unattended in public; a laptop in a car, for instance, must be locked in the
trunk and out of sight, and ideally not left there at all.

## Device Encryption and Updates

All company laptops are equipped with full-disk encryption, and removable media
must be encrypted before any company data is copied onto it. Operating-system
security updates must be installed within seven days of release, and the IT team
may enforce critical patches sooner. The screen must be locked whenever you
leave your workplace; an automatic lock activates after five minutes of
inactivity.

Personal devices used for work ("bring your own device") must be enrolled in the
mobile device management system and meet the same encryption and update
standards. Jailbroken or rooted devices are not permitted to access company
resources under any circumstances.

## Incident Response

Any suspicion of a security incident - such as a phishing email, a lost device,
or an unusual login - must be reported immediately to
security@lumen-analytics.example. The IT team confirms receipt within two hours
during business hours. Affected accounts are locked right away, and the incident
is documented in the incident log.

Employees must not try to "clean up" compromised systems on their own, so that
evidence is preserved for analysis. After an incident is resolved, the IT team
runs a blameless post-mortem: the goal is to fix the underlying weakness, not to
assign blame, because punishing honest reporting only drives incidents
underground.

## Third-Party and Vendor Access

New software-as-a-service tools that process company or customer data must be
reviewed by the IT team before adoption. Vendors with access to confidential
data must provide evidence of appropriate security controls, and their access is
reviewed at least once a year. Access for any external party is granted on a
least-privilege basis and revoked the moment it is no longer needed.
