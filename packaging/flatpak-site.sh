#!/usr/bin/env bash
# The Flatpak repository of polykrom.app: flatpak-site.sh SITE_DIR adds SITE_DIR/flatpak/ (the
# OSTree repository under repo/, polykrom.flatpakrepo and an index page) to the assembled site,
# from the .flatpak bundles of the latest published release (or RELEASE_TAG). The repository
# already served at SITE_BASE_URL (https://polykrom.app) is mirrored first, so refs this release
# does not replace stay and `flatpak update` keeps working; it serves the latest release only. Every site deployment (.github/workflows/site.yml) runs
# it, and no other workflow replaces the site. Objects over Cloudflare's per-file limit are split
# afterwards by site/parts.py and joined again by site/worker.js.
set -euo pipefail
trap 'echo "flatpak-site.sh: line $LINENO: \"$BASH_COMMAND\" failed (exit $?)" >&2' ERR

site="${1:?site directory required}"
tag="${RELEASE_TAG:-}"
app_id=app.polykrom.Polykrom
if [ -z "$tag" ]; then
  tag="$(gh api "repos/$GITHUB_REPOSITORY/releases?per_page=100" \
    --jq '[.[] | select(.draft == false and .prerelease == false)][0].tag_name // empty')"
fi
# Before the first publication the docs and browser app can still deploy.
if [ -z "$tag" ]; then exit 0; fi
if ! printf '%s' "$tag" | grep -Eq '^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-((0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*)(\.(0|[1-9][0-9]*|[0-9]*[A-Za-z-][0-9A-Za-z-]*))*))?(\+[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$'; then
  echo "invalid release tag: $tag" >&2
  exit 1
fi
published="$(gh api "repos/$GITHUB_REPOSITORY/releases/tags/$tag" \
  --jq 'select(.draft == false and .prerelease == false and .published_at != null) | .tag_name')"
if [ "$published" != "$tag" ]; then
  echo "error: $tag is not a published stable release (draft or prerelease)" >&2
  exit 1
fi
test -s "$site/index.html"
test -s "$site/app/index.html"

version="${tag#v}"
bundles="$(mktemp -d)"
gh release download "$tag" --repo "$GITHUB_REPOSITORY" --pattern SHA256SUMS.txt --dir "$bundles"
repo_dir="$site/flatpak/repo"
mkdir -p "$repo_dir"
ostree --repo="$repo_dir" init --mode=archive-z2
base_url="${SITE_BASE_URL:-https://polykrom.app}"
base_url="${base_url%/}"
previous_url="$base_url/flatpak/repo"
# Only a 404 means nothing is served yet: a transient failure must not drop refs this release
# does not replace.
status="$(curl --silent --location --output /dev/null --write-out '%{http_code}' "$previous_url/summary" || true)"
case "$status" in
  200)
    ostree --repo="$repo_dir" remote add --no-gpg-verify previous "$previous_url"
    ostree --repo="$repo_dir" pull --mirror previous
    ostree --repo="$repo_dir" remote delete previous
    ;;
  404) ;;
  *)
    echo "error: $previous_url/summary answered HTTP ${status:-none}; not replacing the served repository" >&2
    exit 1
    ;;
esac
for arch in x86_64 aarch64; do
  asset="polykrom-$version-linux-$arch.flatpak"
  gh release download "$tag" --repo "$GITHUB_REPOSITORY" --pattern "$asset" --dir "$bundles"
  (
    cd "$bundles"
    awk -v file="$asset" '$2 == file || $2 == "*" file { print; found=1 } END { exit !found }' \
      SHA256SUMS.txt | sha256sum --check -
  )
  validation_repo="$(mktemp -d)"
  ostree --repo="$validation_repo" init --mode=archive-z2
  flatpak build-import-bundle "$validation_repo" "$bundles/$asset"
  refs="$(ostree --repo="$validation_repo" refs)"
  if ! printf '%s\n' "$refs" | grep -Fxq "app/$app_id/$arch/stable"; then
    echo "error: $asset does not hold app/$app_id/$arch/stable; refs:" >&2
    printf '%s\n' "$refs" >&2
    exit 1
  fi
  echo "$asset: app/$app_id/$arch/stable"
  flatpak build-import-bundle "$repo_dir" "$bundles/$asset"
done
flatpak build-update-repo --title="Polykrom" --comment='Official Polykrom releases' \
  --description='Official Flatpak repository for Polykrom' \
  --homepage="$base_url" --default-branch=stable "$repo_dir"
test -s "$repo_dir/summary"
cat >"$site/flatpak/polykrom.flatpakrepo" <<META
[Flatpak Repo]
Title=Polykrom
Comment=Official Polykrom releases
Description=Official Flatpak repository for Polykrom
Url=$base_url/flatpak/repo/
Homepage=$base_url
META
cat >"$site/flatpak/index.html" <<HTML
<!doctype html><html lang="en"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1"><title>Polykrom Flatpak repository</title></head><body><main><h1>Polykrom Flatpak repository</h1><p>Latest published release: <strong>$tag</strong></p><p><a href="polykrom.flatpakrepo">Add the Polykrom repository</a></p><p>Flathub provides the required Freedesktop runtime. Add both remotes to the same user installation:</p><pre>flatpak remote-add --user --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
flatpak remote-add --user --if-not-exists polykrom $base_url/flatpak/polykrom.flatpakrepo
flatpak install --user polykrom $app_id
flatpak run $app_id</pre></main></body></html>
HTML
