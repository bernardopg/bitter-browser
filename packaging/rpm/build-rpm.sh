#!/bin/sh
# Builds the Fedora package from the release source archive. Run as root in
# a Fedora container.
#
# Usage: packaging/rpm/build-rpm.sh SOURCE_ARCHIVE OUT_DIR
set -eu

source_archive=${1:?usage: build-rpm.sh SOURCE_ARCHIVE OUT_DIR}
out=${2:?}
here=$(CDPATH='' cd -- "$(dirname -- "$0")" && pwd)
spec=$here/bitter-browser.spec
top=$(mktemp -d)

dnf install -y --setopt=install_weak_deps=False rpm-build rpmlint 'dnf-command(builddep)'
dnf builddep -y "$spec"
mkdir -p "$top/SOURCES" "$out"
cp "$source_archive" "$top/SOURCES/"
rpmbuild --define "_topdir $top" -ba "$spec"
find "$top/RPMS" "$top/SRPMS" -name '*.rpm' -exec cp {} "$out/" \;
rpmlint --config "$here/rpmlint.toml" "$out"/*.rpm
ls -la "$out"
