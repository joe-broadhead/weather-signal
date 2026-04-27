#!/usr/bin/env bash
set -euo pipefail

REPO_SLUG="${WEATHER_SIGNAL_REPO:-joe-broadhead/weather-signal}"
VERSION="${WEATHER_SIGNAL_VERSION:-latest}"
INSTALL_DIR="${WEATHER_SIGNAL_INSTALL_DIR:-$HOME/.local/bin}"
INSTALL_SKILLS="${WEATHER_SIGNAL_INSTALL_SKILLS:-0}"
SKILLS_DIR="${WEATHER_SIGNAL_SKILLS_DIR:-$HOME/.agents/skills}"
SKILL_NAME="${WEATHER_SIGNAL_SKILL_NAME:-}"
NON_INTERACTIVE="${WEATHER_SIGNAL_INSTALL_NONINTERACTIVE:-0}"
VERIFY_CHECKSUM="${WEATHER_SIGNAL_VERIFY_CHECKSUM:-1}"
DOWNLOAD_TOKEN="${WEATHER_SIGNAL_GITHUB_TOKEN:-${GITHUB_TOKEN:-${GH_TOKEN:-}}}"

usage() {
  cat <<'EOF'
Usage: install.sh [--install-skills [--skill <name>]] [--skills-dir <path>] [--non-interactive|-y] [--install-dir <path>]

Downloads and installs weather-signal from GitHub releases.

Defaults:
  - install dir: $HOME/.local/bin
  - skills dir: $HOME/.agents/skills
  - version: latest release
  - checksum verification: enabled

Environment overrides:
  WEATHER_SIGNAL_REPO                    GitHub repo slug (default: joe-broadhead/weather-signal)
  WEATHER_SIGNAL_GITHUB_TOKEN            Optional token for private repos or rate limits
  WEATHER_SIGNAL_VERSION                 Release tag, such as v0.0.0 (default: latest)
  WEATHER_SIGNAL_INSTALL_DIR             Install directory for weather-signal
  WEATHER_SIGNAL_INSTALL_SKILLS          1 to install Agent Skills (default: 0)
  WEATHER_SIGNAL_SKILLS_DIR              Skills destination (default: $HOME/.agents/skills)
  WEATHER_SIGNAL_SKILL_NAME              Optional single skill to install
  WEATHER_SIGNAL_INSTALL_NONINTERACTIVE  1 to skip prompts
  WEATHER_SIGNAL_VERIFY_CHECKSUM         1 to verify artifact checksum (default: 1)
EOF
}

validate_skill_name_segment() {
  local skill_name="$1"
  if [[ -z "${skill_name}" ]]; then
    echo "Skill name cannot be empty." >&2
    return 1
  fi
  if ! [[ "${skill_name}" =~ ^[A-Za-z0-9][A-Za-z0-9._-]*$ ]]; then
    echo "Invalid skill name '${skill_name}'. Use a single safe path segment." >&2
    return 1
  fi
}

resolve_skill_install_selection() {
  if [[ -n "${SKILL_NAME}" ]]; then
    validate_skill_name_segment "${SKILL_NAME}"
  fi
  if [[ -n "${SKILL_NAME}" && "${INSTALL_SKILLS}" != "1" ]]; then
    echo "--skill requires --install-skills." >&2
    return 1
  fi
}

compute_sha256() {
  local file="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file" | awk '{print $1}'
  else
    echo "Could not verify artifact hash: no sha256sum or shasum available." >&2
    return 1
  fi
}

expected_checksum() {
  local checksum_file="$1"
  local filename="$2"
  while read -r hash path _; do
    [[ -z "${hash}" || -z "${path}" ]] && continue
    path="${path#\*}"
    local candidate="${path##*/}"
    candidate="${candidate##*\\}"
    if [[ "${candidate}" == "${filename}" ]]; then
      printf '%s\n' "${hash}"
      return 0
    fi
  done < "${checksum_file}"
}

verify_checksum_file() {
  local artifact="$1"
  local checksum_file="$2"
  local expected
  local actual

  expected="$(expected_checksum "$checksum_file" "$(basename "$artifact")")"
  if [[ -z "$expected" ]]; then
    echo "Checksum file is missing entry for $(basename "$artifact")." >&2
    return 1
  fi

  actual="$(compute_sha256 "$artifact" | tr '[:upper:]' '[:lower:]')"
  expected="$(echo "$expected" | tr '[:upper:]' '[:lower:]')"

  if [[ "$actual" != "$expected" ]]; then
    echo "Checksum mismatch for $(basename "$artifact")." >&2
    echo "  Expected: $expected" >&2
    echo "  Actual:   $actual" >&2
    return 1
  fi
}

download_file() {
  local file_name="$1"
  local url="$2"
  local out="$3"

  if [[ -n "${DOWNLOAD_TOKEN}" ]]; then
    if curl -fsSL -H "Authorization: Bearer ${DOWNLOAD_TOKEN}" "${url}" -o "${out}"; then
      return 0
    fi
    echo "Authenticated download failed for ${file_name}; retrying without token." >&2
  fi

  if curl -fsSL "${url}" -o "${out}"; then
    return 0
  fi

  if command -v gh >/dev/null 2>&1; then
    echo "Direct download failed for ${file_name}; trying gh release download"
    if [[ "${VERSION}" == "latest" ]]; then
      gh release download --repo "${REPO_SLUG}" --pattern "${file_name}" --output "${out}"
    else
      gh release download "${VERSION}" --repo "${REPO_SLUG}" --pattern "${file_name}" --output "${out}"
    fi
    return 0
  fi

  echo "Download failed for ${file_name} and gh CLI is not available for fallback." >&2
  return 1
}

download_repo_archive() {
  local ref="$1"
  local out="$2"
  local archive_url="https://api.github.com/repos/${REPO_SLUG}/tarball/${ref}"

  if [[ -n "${DOWNLOAD_TOKEN}" ]]; then
    if curl -fsSL \
      -H "Authorization: Bearer ${DOWNLOAD_TOKEN}" \
      -H "Accept: application/vnd.github+json" \
      "${archive_url}" \
      -o "${out}"; then
      return 0
    fi
    echo "Authenticated archive download failed for ref '${ref}'; retrying without token." >&2
  fi

  curl -fsSL \
    -H "Accept: application/vnd.github+json" \
    "${archive_url}" \
    -o "${out}"
}

repo_default_branch() {
  local api_url="https://api.github.com/repos/${REPO_SLUG}"
  local response=""

  if [[ -n "${DOWNLOAD_TOKEN}" ]]; then
    response="$(curl -fsSL -H "Authorization: Bearer ${DOWNLOAD_TOKEN}" "${api_url}" 2>/dev/null || true)"
  else
    response="$(curl -fsSL "${api_url}" 2>/dev/null || true)"
  fi

  if [[ -z "${response}" ]]; then
    return 0
  fi

  printf '%s' "${response}" \
    | tr -d '\n' \
    | sed -n 's/.*"default_branch"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
}

latest_release_tag() {
  local api_url="https://api.github.com/repos/${REPO_SLUG}/releases/latest"
  local response=""

  if [[ -n "${DOWNLOAD_TOKEN}" ]]; then
    response="$(curl -fsSL -H "Authorization: Bearer ${DOWNLOAD_TOKEN}" "${api_url}" 2>/dev/null || true)"
  fi
  if [[ -z "${response}" ]]; then
    response="$(curl -fsSL "${api_url}" 2>/dev/null || true)"
  fi
  if [[ -z "${response}" ]]; then
    return 0
  fi

  printf '%s' "${response}" \
    | tr -d '\n' \
    | sed -n 's/.*"tag_name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p'
}

list_standalone_skills() {
  local skills_source="$1"
  find "${skills_source}" -mindepth 1 -maxdepth 1 -type d \
    -exec test -f "{}/SKILL.md" ';' -print | sed 's#.*/##' | sort
}

install_standalone_skill_from_source() {
  local skills_source="$1"
  local skills_dest="$2"
  local skill_name="$3"
  local skill_source="${skills_source}/${skill_name}"
  local installed_dir="${skills_dest}/${skill_name}"

  validate_skill_name_segment "${skill_name}" || return 1
  if [[ ! -f "${skill_source}/SKILL.md" ]]; then
    echo "Skill '${skill_name}' not found in repository archive." >&2
    return 1
  fi

  mkdir -p "${skills_dest}" || return 1
  rm -rf "${installed_dir}" || return 1
  cp -R "${skill_source}" "${installed_dir}" || return 1
  echo "Installed skill '${skill_name}' to ${skills_dest}"
}

install_all_standalone_skills_from_source() {
  local skills_source="$1"
  local skills_dest="$2"
  local skill_count=0
  local skill_name=""

  while IFS= read -r skill_name; do
    [[ -n "${skill_name}" ]] || continue
    install_standalone_skill_from_source "${skills_source}" "${skills_dest}" "${skill_name}" || return 1
    skill_count=$((skill_count + 1))
  done < <(list_standalone_skills "${skills_source}")

  if (( skill_count < 1 )); then
    echo "No standalone skills found in repository archive." >&2
    return 1
  fi

  echo "Installed ${skill_count} skill(s) to ${skills_dest}"
}

install_skills_from_ref() {
  local ref="$1"
  local skills_dest="$2"
  local requested_skill="$3"
  local archive_ref="${ref//\//-}"
  local archive_path="${tmp_dir}/repo-${archive_ref}.tar.gz"
  local extract_dir="${tmp_dir}/repo-${archive_ref}"
  local skills_source=""

  download_repo_archive "${ref}" "${archive_path}" || return 1
  mkdir -p "${extract_dir}" || return 1
  tar -xzf "${archive_path}" -C "${extract_dir}" || return 1
  skills_source="$(find "${extract_dir}" -type d -path "*/.github/skills" | head -n 1)"
  if [[ -z "${skills_source}" ]]; then
    echo "Skills directory not found in repository archive for ref '${ref}'." >&2
    return 1
  fi

  if [[ -n "${requested_skill}" ]]; then
    install_standalone_skill_from_source "${skills_source}" "${skills_dest}" "${requested_skill}" || return 1
  else
    install_all_standalone_skills_from_source "${skills_source}" "${skills_dest}" || return 1
  fi
}

resolve_skills_ref() {
  local detected_latest_tag=""

  if [[ "${VERSION}" != "latest" ]]; then
    printf '%s\n' "${VERSION}"
    return 0
  fi

  detected_latest_tag="$(latest_release_tag)"
  if [[ -z "${detected_latest_tag}" ]]; then
    echo "Could not resolve latest release tag for skills installation." >&2
    echo "Set WEATHER_SIGNAL_VERSION to an explicit release tag, or retry after GitHub is reachable." >&2
    return 1
  fi

  printf '%s\n' "${detected_latest_tag}"
}

install_selected_skills() {
  local requested_skill="$1"
  local skills_ref=""

  skills_ref="$(resolve_skills_ref)"
  if [[ -n "${requested_skill}" ]]; then
    echo "Installing skill '${requested_skill}' from ref '${skills_ref}' into ${SKILLS_DIR}"
  else
    echo "Installing skills from ref '${skills_ref}' into ${SKILLS_DIR}"
  fi

  install_skills_from_ref "${skills_ref}" "${SKILLS_DIR}" "${requested_skill}"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --non-interactive|-y)
      NON_INTERACTIVE="1"
      ;;
    --install-skills)
      INSTALL_SKILLS="1"
      ;;
    --skills-dir)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --skills-dir" >&2
        exit 1
      fi
      SKILLS_DIR="$2"
      shift
      ;;
    --skill)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --skill" >&2
        exit 1
      fi
      SKILL_NAME="$2"
      shift
      ;;
    --install-dir)
      if [[ $# -lt 2 ]]; then
        echo "Missing value for --install-dir" >&2
        exit 1
      fi
      INSTALL_DIR="$2"
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
  shift
done

resolve_skill_install_selection

OS="$(uname -s)"
ARCH="$(uname -m)"

asset_os=""
case "${OS}" in
  Linux) asset_os="linux" ;;
  Darwin) asset_os="macos" ;;
  MINGW*|MSYS*|CYGWIN*) asset_os="windows" ;;
  *) echo "Unsupported OS: ${OS}" >&2; exit 1 ;;
esac

case "${ARCH}" in
  x86_64|amd64) asset_arch="x86_64" ;;
  arm64|aarch64)
    if [[ "${asset_os}" == "macos" ]]; then
      asset_arch="arm64"
    else
      echo "Unsupported arch for release assets: ${ARCH}" >&2
      exit 1
    fi
    ;;
  *) echo "Unsupported arch: ${ARCH}" >&2; exit 1 ;;
esac

if [[ "${asset_os}" == "windows" ]]; then
  ext=".exe"
else
  ext=""
fi

asset="weather-signal-${asset_os}-${asset_arch}.tar.gz"
checksum_file="weather-signal-${asset_os}-${asset_arch}.sha256"
if [[ "${VERSION}" == "latest" ]]; then
  url="https://github.com/${REPO_SLUG}/releases/latest/download/${asset}"
  checksum_url="https://github.com/${REPO_SLUG}/releases/latest/download/${checksum_file}"
else
  url="https://github.com/${REPO_SLUG}/releases/download/${VERSION}/${asset}"
  checksum_url="https://github.com/${REPO_SLUG}/releases/download/${VERSION}/${checksum_file}"
fi

tmp_dir="$(mktemp -d)"
trap 'rm -rf "${tmp_dir}"' EXIT

echo "Downloading ${url}"
download_file "${asset}" "${url}" "${tmp_dir}/${asset}"

if [[ "${VERIFY_CHECKSUM}" == "1" ]]; then
  echo "Downloading ${checksum_url}"
  download_file "${checksum_file}" "${checksum_url}" "${tmp_dir}/${checksum_file}"
  echo "Verifying SHA-256 checksum"
  verify_checksum_file "${tmp_dir}/${asset}" "${tmp_dir}/${checksum_file}"
fi

tar -xzf "${tmp_dir}/${asset}" -C "${tmp_dir}"
binary_path="$(find "${tmp_dir}" -type f -name "weather-signal${ext}" | head -n 1)"
if [[ -z "${binary_path}" ]]; then
  echo "weather-signal binary was not found in ${asset}." >&2
  exit 1
fi

mkdir -p "${INSTALL_DIR}"
cp "${binary_path}" "${INSTALL_DIR}/weather-signal${ext}"
chmod +x "${INSTALL_DIR}/weather-signal${ext}"

if [[ "${INSTALL_SKILLS}" == "1" ]]; then
  install_selected_skills "${SKILL_NAME}"
elif [[ "${NON_INTERACTIVE}" != "1" && -t 0 ]]; then
  read -r -p "Install Weather Signal agent skills? [y/N]: " choice
  choice="${choice:-N}"
  case "${choice}" in
  [Yy])
    INSTALL_SKILLS="1"
    SKILL_NAME=""
    install_selected_skills ""
    ;;
  esac
fi

echo "Installed weather-signal to ${INSTALL_DIR}/weather-signal${ext}"
echo "Add ${INSTALL_DIR} to your PATH if needed."
