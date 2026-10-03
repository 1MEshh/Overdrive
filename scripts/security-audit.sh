#!/usr/bin/env bash
set -euo pipefail

echo "[AUDIT] Starting zero-leak and security gate for Overdrive..."

TRACKED_FILES=$(git ls-files 2>/dev/null || echo "")
if [ -z "$TRACKED_FILES" ]; then
    echo "[AUDIT] Notice: No tracked files in git index yet; scanning repository tree..."
    TRACKED_FILES=$(find . -maxdepth 4 -not -path '*/.*' -not -path './target*' -type f)
fi

LEAK_FOUND=0

# 1. Scan for local filesystem paths (/home/<user>, /Users/<user>, C:\Users\<user>)
if grep -rnE '(/home/[a-zA-Z0-9_-]+|/Users/[a-zA-Z0-9_-]+)' $TRACKED_FILES --exclude="security-audit.sh" 2>/dev/null; then
    echo "[FAIL] Local absolute filesystem path detected in repository files."
    LEAK_FOUND=1
fi

# 2. Scan for current OS user identity if set
CURRENT_USER="${USER:-$(whoami 2>/dev/null || echo "")}"
if [ -n "$CURRENT_USER" ] && [ "$CURRENT_USER" != "root" ] && [ "$CURRENT_USER" != "runner" ]; then
    if grep -rniF "$CURRENT_USER" $TRACKED_FILES --exclude="security-audit.sh" 2>/dev/null; then
        echo "[FAIL] OS username leaked in repository files."
        LEAK_FOUND=1
    fi
fi

# 3. Scan for system hostname if available
CURRENT_HOST="$(hostname 2>/dev/null || echo "")"
if [ -n "$CURRENT_HOST" ] && [ "$CURRENT_HOST" != "localhost" ] && [ ${#CURRENT_HOST} -ge 4 ]; then
    if grep -rniF "$CURRENT_HOST" $TRACKED_FILES --exclude="security-audit.sh" 2>/dev/null; then
        echo "[FAIL] Hostname leaked in repository files."
        LEAK_FOUND=1
    fi
fi

# 4. Scan for API key or token prefixes
if grep -rnE '(ghp_[A-Za-z0-9]{20,}|sk-[A-Za-z0-9]{20,}|vercel_[A-Za-z0-9]{16,}|AKIA[0-9A-Z]{16})' $TRACKED_FILES --exclude="security-audit.sh" 2>/dev/null; then
    echo "[FAIL] Potential secret or API token detected in repository files."
    LEAK_FOUND=1
fi

# 5. Scan for emojis in code, comments, and documentation
EMOJI_DETECTED=$(python3 -c '
import sys, re

emoji_pattern = re.compile(
    r"[\U0001F600-\U0001F64F"  # emoticons
    r"\U0001F300-\U0001F5FF"  # symbols & pictographs
    r"\U0001F680-\U0001F6FF"  # transport & map
    r"\U0001F1E0-\U0001F1FF"  # flags (iOS)
    r"\U00002702-\U000027B0"
    r"\U000024C2-\U0001F251"
    r"\U0001F900-\U0001F9FF"  # supplemental symbols
    r"\U0001FA70-\U0001FAFF"  # symbols and pictographs extended-a
    r"]+", flags=re.UNICODE
)

files = sys.argv[1:]
has_emoji = False
for f in files:
    if f.endswith((".png", ".gif", ".jpg", ".jpeg", ".ico", ".lock")):
        continue
    try:
        with open(f, "r", encoding="utf-8", errors="ignore") as fh:
            for line_no, line in enumerate(fh, 1):
                if emoji_pattern.search(line):
                    print(f"[FAIL] Emoji detected in {f}:{line_no}: {line.strip()[:60]}")
                    has_emoji = True
    except Exception:
        pass

if has_emoji:
    sys.exit(1)
sys.exit(0)
' $TRACKED_FILES 2>&1 || echo "EMOJI_LEAK")

if [[ "$EMOJI_DETECTED" == *"EMOJI_LEAK"* ]] || [[ "$EMOJI_DETECTED" == *"[FAIL]"* ]]; then
    echo "$EMOJI_DETECTED"
    echo "[FAIL] Emojis are strictly disallowed in code, comments, or documentation."
    LEAK_FOUND=1
fi

# 6. Verify git author/committer configuration does not leak personal identity
GIT_EMAIL=$(git config user.email || echo "")
if [[ "$GIT_EMAIL" != *"noreply.github.com"* ]]; then
    echo "[FAIL] Git user.email ('$GIT_EMAIL') is not a sanitized noreply address."
    LEAK_FOUND=1
fi

if [ "$LEAK_FOUND" -ne 0 ]; then
    echo "[AUDIT] FAILED: Security or privacy leak detected."
    exit 1
fi

echo "[AUDIT] PASSED: Zero PII, path, secret, or emoji leaks detected."
