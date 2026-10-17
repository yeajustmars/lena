#!/usr/bin/env bash
# -------------------------------------------------------------------------------
# Leña Release script
#         Usage: ./release.sh <new-version>
#       Example: ./release.sh 0.1.0
#   Description: Automates the release process for Leña workspace crates.
# -------------------------------------------------------------------------------

# The exact order matters here! Dependencies must be published before dependents.
CRATES_TO_PUBLISH=("lena-protocol" "lena-core" "lena")

LENA_LOGO=$(cat <<'EOF'
  _          _    __
 | |   ___  (_)_ |  |
 | |__/ -_) | ' \| .` |
 |____\___| |_||_\__,_|

EOF
)

BOLD='\033[1m'
BLUE='\033[0;34m'
RED='\033[0;31m'
GREEN='\033[0;32m'
ORANGE='\033[0;33m'
NC='\033[0m'

ERROR="${RED}[ERROR]${NC}"
INFO="${BLUE}[INFO]${NC}"
SUCCESS="${GREEN}[OK]${NC}"

echo -e "${BLUE}${LENA_LOGO}${NC}"

show_spinner() {
    local pid=$1
    local msg=$2
    local delay=0.1
    local spinstr='|/-\'

    tput civis
    trap "tput cnorm; echo; exit 1" INT TERM

    while kill -0 "$pid" 2>/dev/null; do
        local temp=${spinstr#?}
        printf "\r ${BLUE}[%c]${NC} %b" "$spinstr" "$msg"
        local spinstr=$temp${spinstr%"$temp"}
        sleep $delay
    done

    printf "\r\033[K"
    tput cnorm
    trap - INT TERM
}

publish_crate() {
    local CRATE_NAME=$1
    local NEW_VERSION=$2
    local STEP=$3

    echo -e "\n$INFO ${STEP}. ${BOLD}${CRATE_NAME}:${NC} Dry run"
    cargo publish -p "$CRATE_NAME" --dry-run

    echo -e "$INFO ${STEP}. ${BOLD}${CRATE_NAME}:${NC} Publishing"
    cargo publish -p "$CRATE_NAME"

    echo -e "$INFO ${STEP}. ${BOLD}Waiting for '${CRATE_NAME}' on crates.io${NC}"
    local MAX_RETRIES=30

    while true; do
        for ((i=1; i<=MAX_RETRIES; i++)); do
            HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "https://crates.io/api/v1/crates/${CRATE_NAME}/${NEW_VERSION}")
            if [ "$HTTP_CODE" -eq 200 ]; then
                echo -e "${SUCCESS} ${BLUE}${CRATE_NAME} v${NEW_VERSION}${NC} is live!"

                echo -e "$INFO ${STEP}. ${BOLD}Syncing local Cargo registry${NC}"
                cargo update -p "$CRATE_NAME" || cargo search "$CRATE_NAME" --limit 1 > /dev/null
                return 0
            fi
            STATUS_MSG="Attempt ${BOLD}${i}/${MAX_RETRIES}${NC} | Status: ${RED}${HTTP_CODE} (Not found)${NC} | Waiting 10s..."
            sleep 10 &
            show_spinner $! "$STATUS_MSG"
        done

        # If we exit the loop, we hit MAX_RETRIES without success
        echo -e "\n$ERROR Timed out waiting for crates.io to index ${CRATE_NAME}."
        read -p "Do you want to [K]eep waiting, [S]kip to next crate, or [A]bort? (k/s/a): " -n 1 -r choice
        echo
        case "$choice" in
            [Kk]* ) echo -e "${INFO} Resuming wait loop...";;
            [Ss]* ) echo -e "${ORANGE}Skipping wait check. Local registry may fail to sync.${NC}"; return 0;;
            * ) exit 1;;
        esac
    done
}

#### ________________________________________________________ CONFIGURE RELEASE

script_dir=$(cd -- "$( dirname -- "${BASH_SOURCE[0]}" )" &> /dev/null && pwd)
cd "$script_dir"

# Fail fast on errors, undefined vars, or pipeline failures
set -euo pipefail

if [ "$#" -ne 1 ]; then
    echo -e "      ${BOLD}Usage:${NC} $0 <new-version>"
    echo -e "    ${BOLD}Example:${NC} $0 0.1.0"
    echo -e "\n$ERROR Missing required version argument."
    exit 1
fi

if [[ ! "$1" =~ ^(v)?[0-9]+\.[0-9]+\.[0-9]+(-.*)?$ ]]; then
    echo -e "$ERROR Version must be in the format X.Y.Z (e.g., 0.1.0)"
    exit 1
fi

if ! git diff --quiet || ! git diff --cached --quiet; then
    echo -e "$ERROR Git working directory is not clean. Commit or stash changes first."
    exit 1
fi

OLD_VERSION=$(grep '^version = ' Cargo.toml | head -n 1 | sed -E 's/version = "(.*)"/\1/')
NEW_VERSION="${1#v}"
TAG="v$NEW_VERSION"

echo -e "${INFO} Found the following:"
echo -e "  ${BLUE}OLD_VERSION:${NC}$OLD_VERSION"
echo -e "  ${BLUE}NEW_VERSION:${NC}$NEW_VERSION"
echo -e "          ${BLUE}TAG:${NC}$TAG"

CONFIG_MSG=$(echo -e "\n${ORANGE}Continue?${NC}(${GREEN}y${NC}/${RED}n${NC}): ")
read -p "$CONFIG_MSG " -n 1 -r; echo; [[ $REPLY =~ ^[Yy]$ ]] || exit 1

#### ________________________________________________________ START RELEASE

echo -e "\n${INFO} 0. ${BOLD}Starting release:${NC} ${BLUE}${NEW_VERSION}${NC}"

echo -e "$INFO 1. ${BOLD}Cargo.toml:${NC} ${BLUE}${OLD_VERSION}${NC} --> ${BLUE}${NEW_VERSION}${NC}"
sed -i.bak -e "s/^version = \".*\"/version = \"$NEW_VERSION\"/" Cargo.toml
for crate in "${CRATES_TO_PUBLISH[@]}"; do
    sed -i.bak -E "s/^(${crate}[[:space:]]*=.*version[[:space:]]*=[[:space:]]*\")[^\"]+(\".*)$/\1$NEW_VERSION\2/" Cargo.toml
done
rm Cargo.toml.bak

echo -e "$INFO 2. ${BOLD}Cargo.lock:${NC} syncing with Cargo.toml"
cargo check --quiet

echo -e "$INFO 3. ${BOLD}Committing version bump${NC}"
git add Cargo.toml Cargo.lock
git commit -m "chore: bump version to $NEW_VERSION"

echo -e "$INFO 4. ${BOLD}Creating tag $TAG${NC}"
git tag "$TAG"

echo -e "$INFO 5. ${BOLD}Pushing to GitHub (Triggering CI/CD)${NC}"
CURRENT_BRANCH=$(git rev-parse --abbrev-ref HEAD)
git push origin "$CURRENT_BRANCH"
git push origin "$TAG"

# --- PUBLISH CRATES ---
STEP_NUM=6
for crate in "${CRATES_TO_PUBLISH[@]}"; do
    publish_crate "$crate" "$NEW_VERSION" "$STEP_NUM"
    ((STEP_NUM++))
done

echo -e "\n${SUCCESS} Successfully deployed and published version $NEW_VERSION!"
