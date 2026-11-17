# 0003. Release artifacts and the Arch repository

Date: 2026-11-17
Status: Accepted

## Context

The project needs one place for the family to install Zendo and receive
updates on Arch/CachyOS and Fedora. A tag was already used for `v0.1.0`
(2026-11-16), but the Arch `zendo-git` package builds `main` and the Fedora
spec versions a snapshot of whatever ref it gets. The AUR would be the
natural Arch home; its registration is temporarily closed ("a wave of
automated account creation"), and the project has no AUR account.

## Decision

A `v*` tag builds and publishes everything: a GitHub Release with the source
tarball, `SHA256SUMS`, the Arch package and the Fedora SRPM; the Arch package
and a `repo-add` database on a `gh-pages` branch served at
`https://leandrop-dev.github.io/zendo/x86_64`; and a COPR build from the
tag's SRPM. Arch users add the repository once with
`SigLevel = Optional TrustAll` and update with `pacman -Syu`. `zendo-git`
keeps tracking `main`. The AUR package is prepared and published when
registration reopens.

## Alternatives Considered

- **AUR only.** The natural place, but registration is closed and there is
  no account; nothing can be published today.
- **GitHub Releases only, `pacman -U` per release.** No automatic updates,
  and the "family can update" goal fails.
- **Open Build Service's Arch repository.** It builds PKGBUILDs but without a
  network, so the `git+https` source and crate fetching need source services
  and a vendored tree: more moving parts and another account for the same
  result.
- **A signed repository from the start.** Requires a project key, its
  distribution and a keyring package; deferred, with `SigLevel = Optional
  TrustAll` for now (HTTPS and the repository's access control are the only
  guarantees).

## Consequences

- The repository is unsigned: anyone able to write the `gh-pages` branch can
  publish a package that Arch machines trust. The workflow and branch
  protection are the only guard until a key exists; signing is a follow-up.
- GitHub Pages holds about 18 MB per release plus the database; old packages
  stay (the workflow keeps files), so the site grows with each release. A
  cleanup policy is needed eventually.
- COPR automation depends on the `COPR_LOGIN` and `COPR_API_TOKEN` secrets;
  a rotated token breaks releases until the secret is updated.
- The Fedora spec and SRPM are generated from the same tag with the vendored
  crates, so the build stays offline; the vendoring step runs in the release
  job, which has network.
- The release package and `zendo-git` conflict, as both own `/usr/bin/zendo`;
  switching between them is a normal pacman replacement.
- A tag is not moved: a failed release is fixed forward with a new version.
