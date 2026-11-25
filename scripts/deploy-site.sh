#!/bin/sh
# Builds the website and publishes it to browser.bebitterbebetter.com.br.
#
# Usage: scripts/deploy-site.sh [--dry-run]
# Reads SITE_SSH_HOST, SITE_SSH_PORT, SITE_SSH_USER, SITE_SSH_KEY_PATH and
# SITE_REMOTE_DIR from the environment or from .env. The server's host key
# must already be in ~/.ssh/known_hosts (or SITE_SSH_KNOWN_HOSTS).
set -eu

root=$(CDPATH='' cd -- "$(dirname -- "$0")/.." && pwd)
if [ -f "$root/.env" ]; then
    set -a
    # shellcheck disable=SC1091
    . "$root/.env"
    set +a
fi
: "${SITE_SSH_HOST:?}" "${SITE_SSH_PORT:?}" "${SITE_SSH_USER:?}" "${SITE_SSH_KEY_PATH:?}" "${SITE_REMOTE_DIR:?}"
dry=
[ "${1:-}" = --dry-run ] && dry=--dry-run

build=$(mktemp -d)
trap 'rm -rf "$build"' EXIT
"$root/scripts/build-site.sh" "$build/site"

known=${SITE_SSH_KNOWN_HOSTS:-$HOME/.ssh/known_hosts}
ssh_cmd="ssh -p $SITE_SSH_PORT -i $SITE_SSH_KEY_PATH -o IdentitiesOnly=yes -o StrictHostKeyChecking=yes -o UserKnownHostsFile=$known"
# --delete keeps the server identical to the build; .well-known holds the
# Flathub token, which the repository also carries.
rsync -rlt --checksum --delete --delay-updates $dry --itemize-changes \
    -e "$ssh_cmd" "$build/site/" "$SITE_SSH_USER@$SITE_SSH_HOST:$SITE_REMOTE_DIR/"
echo "Published https://browser.bebitterbebetter.com.br/"
