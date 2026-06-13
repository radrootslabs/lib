#!/usr/bin/env bash
set -euo pipefail

scan_forbidden() {
  local label="$1"
  local pattern="$2"
  shift 2

  local matches
  matches="$(
    rg -nI \
      --glob '!AGENTS.md' \
      --glob '!scripts/ci/guard_no_legacy_identifiers.sh' \
      -- "$pattern" "$@" ||
      true
  )"

  if [[ -n $matches ]]; then
    echo "$label is forbidden in oss source files"
    echo "$matches"
    exit 1
  fi
}

scan_forbidden "legacy identifier 'tangle'" "tangle" .

scan_forbidden \
  "legacy broad trade event identifier" \
  "RadrootsTradeMessageType|RadrootsTradeEnvelope|RadrootsTradeMessagePayload|RadrootsTradeQuestion|RadrootsTradeAnswer|RadrootsTradeDiscount|RadrootsTradeOrder|RadrootsActiveOrder|RadrootsActiveTrade|RadrootsTradeListingParseError|RadrootsTradeDomain|TradeListingParseError|TradeListingEnvelope|TradeListingMessage|KIND_TRADE_ORDER|TRADE_LISTING_KINDS|build_envelope_draft|parse_envelope|public_trade|events::trade::|events_codec::trade::|radroots_sdk::trade::|trade_order_economics_digest|trade_revision|trade_lifecycle|reduce_active_order|canonicalize_active_order|active_trade_|ActiveOrder|active_order|active order|active trade|RADROOTS_TRADE_(LISTING_DOMAIN|ENVELOPE_VERSION)" \
  crates spec scripts

scan_forbidden \
  "legacy broad trade listing kind constant" \
  "KIND_TRADE_LISTING_(ORDER|QUESTION|ANSWER|DISCOUNT|CANCEL|FULFILLMENT|RECEIPT)" \
  crates spec scripts

echo "no legacy identifiers found in oss source files"
