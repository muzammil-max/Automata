#!/usr/bin/env bash
# ============================================================
# AUTOMATA WEBSITE — VERCEL DEPLOY
# ------------------------------------------------------------
# Deploys website/ to production and keeps the short domain
# automata-app.vercel.app pointed at the newest build.
#
# First time only:  npx vercel login
# Then:             ./deploy.sh
# ============================================================

set -euo pipefail

cd "$(dirname "${BASH_SOURCE[0]}")"

echo "Deploying website/ to Vercel (production) ..."
OUT="$(npx --yes vercel@latest deploy --prod --yes 2>&1)"
echo "$OUT"

# Grab the deployment-specific URL from the CLI output.
DEPLOY_URL="$(printf '%s\n' "$OUT" | grep -oE 'https://[a-z0-9-]+-[a-z0-9]+-[a-z0-9-]+\.vercel\.app' | head -1)"

if [ -n "$DEPLOY_URL" ]; then
  echo ""
  echo "Pointing automata-app.vercel.app at $DEPLOY_URL ..."
  npx --yes vercel@latest alias set "$DEPLOY_URL" automata-app.vercel.app
  # Make the alias follow future production deploys (no manual step needed).
  npx --yes vercel@latest domains add automata-app.vercel.app automata || true
else
  echo "NOTE: deployment URL not found in output; alias not refreshed."
fi

echo ""
echo "Live: https://automata-app.vercel.app"
