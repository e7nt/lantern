# Security policy

## Reporting a vulnerability

Do not open a public issue for a suspected vulnerability. Use GitHub's private
vulnerability reporting for `e7nt/lantern`:

<https://github.com/e7nt/lantern/security/advisories/new>

Include the affected revision, impact, prerequisites, and the smallest safe
reproduction you can provide. Do not include real credentials, private source
code, prompts, provider output, or data from somebody else's workbench.

The project will acknowledge a report when a maintainer is available, validate
the affected boundary, and coordinate disclosure after a fix. Lantern is an
early open-source project and does not yet promise a response-time SLA.

## Supported versions

Security fixes target the current `main` branch until versioned releases begin.
Historical protocol snapshots document old contracts; they are not maintained
runtime versions.

## Security boundaries

Lantern currently treats an explicitly launched repository as a trusted
workbench. The agent can read, edit, and execute commands in that repository.
Provider authentication belongs to Pi and the user's normal process
environment. Review these contracts before reporting expected behavior as a
vulnerability:

- `docs/CREDENTIALS.md`
- `docs/DIAGNOSTICS.md`
- `docs/decisions/003-trusted-workspace-default.md`

Credential disclosure, repository escape, command execution outside the opened
workbench, unsafe handling of untrusted Git content, and diagnostic leakage are
in scope. Social-engineering requests for a user to run Lantern on an untrusted
repository are important product-safety reports but should clearly identify
that prerequisite.
