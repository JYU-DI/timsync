#! /bin/bash

set -eu

log() {
  echo -e "$1" >&2
}

_INSTALL_DIR=${HOME}/bin
CMD_NAME=timsync
COMMAND="${_INSTALL_DIR}/${CMD_NAME}"

TIM_HOST=${TIM_HOST:-""}
TIM_FOLDER_ROOT=${TIM_FOLDER_ROOT:-""}
TIM_USERNAME=${TIM_USERNAME:-""}
TIM_PASSWORD=${TIM_PASSWORD:-""}
ROOT_FOLDER=${ROOT_FOLDER:-"."}
TIMSYNC_VERSION=${TIMSYNC_VERSION:-"ci"}

if [ -z "${TIM_HOST}" ]; then
  log "ERROR: No TIM host is specified"
  exit 1
fi
if [ -z "${TIM_FOLDER_ROOT}" ]; then
  log "ERROR: No TIM folder root is specified"
  exit 1
fi
if [ -z "${TIM_USERNAME}" ]; then
  log "ERROR: No TIM username is specified"
  exit 1
fi
if [ -z "${TIM_PASSWORD}" ]; then
  log "ERROR: No TIM password is specified"
  exit 1
fi

if [[ ! -x "${COMMAND}" ]]; then
  log "Downloading timsync version '${TIMSYNC_VERSION}'..."
  wget --progress=dot:mega "https://github.com/JYU-DI/timsync/releases/download/${TIMSYNC_VERSION}/timsync.linux.x86_64"
  mkdir -p "${_INSTALL_DIR}"
  mv timsync.linux.x86_64 "${COMMAND}"
  chmod +x "${COMMAND}"
fi

log "Initializing timsync..."

cd "${ROOT_FOLDER}"
"${COMMAND}" init --no-prompt

echo "
[targets.default]
host = \"${TIM_HOST}\"
folder_root = \"${TIM_FOLDER_ROOT}\"
username = \"${TIM_USERNAME}\"
password = \"${TIM_PASSWORD}\"
" >"${ROOT_FOLDER}/.timsync/config.toml"

log "Syncing with TIM..."

"${COMMAND}" sync
