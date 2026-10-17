#!/usr/bin/env bash
# -------------------------------------------------------------------------------
# Leña Release script
#         Usage: ./release.sh <new-version>
#       Example: ./release.sh 0.1.0
# -------------------------------------------------------------------------------

# Dependencies MUST be published before dependents.
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
        printf "\r ${BLUE}[\%c]${NC} %b" "$spinstr" "$msg"
        local spinstr=$temp${spinstr\%"$temp"}
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

    echo -e "$INFO ${STEP}.${BOLD}${CRATE_NAME}:${NC} Publishing"
    cargo publish -p "$CRATE_NAME"

    echo -e "$INFO${STEP}. ${BOLD}Waiting for '${CRATE_NAME}' on crates.io${NC}"
    local MAX_RETRIES=30

    while true; do
        for ((i=1; i<=MAX_RETRIES; i++)); do
            HTTP_CODE=$(curl -s -o /dev/null -w "%{http_code}" "https://crates.io/api/v1/crates/${CRATE_NAME}/${NEW_VERSION}")
            if [ "$HTTP_CODE" -eq 200 ]; then
                echo -e "${SUCCESS} ${BLUE}${CRATE_NAME} v${NEW_VERSION}${NC} is live!"

                echo -e "$INFO${STEP}. ${BOLD}Syncing local Cargo registry${NC}"
                cargo update -p "$CRATE_NAME" || cargo search "$CRATE_NAME" --limit 1 > /dev/null
                return 0
            fi
            STATUS_MSG="Attempt ${BOLD}${i}/${MAX_RETRIES}${NC} \vert{} Status:${RED}${HTTP_CODE} (Not found)${NC} | Waiting 10s..."
            sleep 10 &
            show_spinner $! "$STATUS_MSG"
        done

        # If we exit the loop, we hit MAX_RETRIES without success
        echo -e "\n$ERROR Timed out waiting for crates.io to index${CRATE_NAME}."
        read -p "Do you want to [K]eep waiting, [S]kip to next crate, or [A]b
