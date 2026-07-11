# Security policy

## Supported versions

| Version | Supported |
|---|:---:|
| 0.1.x | Yes |
| Older | No |

## Reporting a vulnerability

Do not open a public issue for vulnerabilities that could cause unintended deletion, path traversal, symlink abuse, command injection, or exposure of private filesystem information.

Use [GitHub private vulnerability reporting](https://github.com/tuanle96/devclean/security/advisories/new). Include:

- affected version and platform;
- exact command and arguments;
- minimal reproduction using disposable fixture data;
- expected and observed safety behavior;
- potential impact.

The maintainer will acknowledge reports on a best-effort basis, investigate privately, and coordinate disclosure with the reporter. Avoid testing against data you do not own or have permission to modify.
