#!/bin/sh
# Makes the release SRPM for a tag: the tag's tarball and a vendored copy of
# its crates, since COPR builds without a network.
#   make-srpm.sh <version> [outdir]
set -eu

version=${1:?usage: make-srpm.sh <tag version> [outdir]}
out="${2:-.}"
mkdir -p "$out"
out=$(cd "$out" && pwd)
# RPM has no hyphen inside Version: a pre-release becomes 1.0.0~rc.1.
rpmversion=$(printf '%s' "$version" | sed 's/-/~/' )
tag="v$version"
url="https://github.com/leandrop-dev/zendo/archive/refs/tags/$tag.tar.gz"
root=$(mktemp -d)
trap 'rm -rf "$root"' EXIT

curl -fsSL "$url" -o "$root/zendo-$version.tar.gz"
tar xzf "$root/zendo-$version.tar.gz" -C "$root"
# The frontend's built files travel with the sources: a build machine has no
# network for npm. Needs Node and npm.
(cd "$root/zendo-$version/crates/zendo/ui" \
    && npm ci --ignore-scripts --no-audit --no-fund && npm run build)
tar -C "$root/zendo-$version/crates/zendo/ui" -czf "$root/zendo-$version-ui.tar.gz" build
(cd "$root/zendo-$version" && cargo vendor --locked "$root/vendor" >/dev/null)
mkdir "$root/vendor-tar"
mv "$root/vendor" "$root/vendor-tar/vendor"
tar czf "$root/zendo-$version-vendor.tar.gz" -C "$root/vendor-tar" .

cp "$root/zendo-$version.tar.gz" "$root/zendo-$version-vendor.tar.gz" \
    "$root/zendo-$version-ui.tar.gz" "$out/"
# The SRPM keeps the spec unexpanded, and COPR rebuilds from it without our
# --define values, so write them into a copy of the spec instead.
sed -e "s/^%{!?zendover:.*/%global zendover $rpmversion/" \
    -e "s/^%{!?tagversion:.*/%global tagversion $version/" \
    "$(dirname "$0")/zendo-release.spec" > "$root/zendo-release.spec"
rpmbuild -bs "$root/zendo-release.spec" \
  --define "_sourcedir $out" \
  --define "_srcrpmdir $out"
