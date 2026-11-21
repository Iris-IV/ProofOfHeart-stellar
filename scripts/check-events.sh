#!/usr/bin/env bash
# Fails when an event topic published by the contract is missing from EVENT_PAYLOADS.md (#1467).
# Looks at `publish(("name", ...` and `emit_event!(env, ("name", ...` in src/ (tests excluded).
set -euo pipefail
cd "$(dirname "$0")/.."

topics=$(grep -rhoE --include='*.rs' --exclude-dir=tests \
  -e 'publish\(\s*\(\s*"[a-z_0-9]+"' -e 'emit_event!\(\s*[a-z_]+,\s*\(\s*"[a-z_0-9]+"' src \
  | grep -oE '"[a-z_0-9]+"' | tr -d '"' | sort -u)
# Multi-line `.publish(\n ("name"` calls.
multi=$(grep -rhzoE --include='*.rs' --exclude-dir=tests 'publish\(\s*\(\s*"[a-z_0-9]+"' src \
  | tr '\0' '\n' | grep -oE '"[a-z_0-9]+"' | tr -d '"' | sort -u || true)
topics=$(printf '%s\n%s\n' "$topics" "$multi" | sort -u | sed '/^$/d')

missing=0
for t in $topics; do
  if ! grep -qF "\`(\"$t\"" EVENT_PAYLOADS.md && ! grep -qE "^### \`$t( |\`)" EVENT_PAYLOADS.md; then
    echo "EVENT_PAYLOADS.md is missing event: $t"
    missing=1
  fi
done
if [ "$missing" -ne 0 ]; then
  echo "Document the event(s) above (topics, data, emitting entrypoint)." >&2
  exit 1
fi
echo "EVENT_PAYLOADS.md covers all $(echo "$topics" | wc -l | tr -d ' ') published event topics."
