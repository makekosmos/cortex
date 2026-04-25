import { strict as assert } from 'node:assert'
import fs from 'node:fs'

const ffi = fs.readFileSync('packages/ark-core/rust/src/ffi.rs', 'utf8')
const normalizedFfi = ffi.replace(/\s+/g, ' ')
const readme = fs.readFileSync('packages/ark-core/README.md', 'utf8')

const requiredSnippets = [
  'use crate::relay_sync::{RelaySync, RelaySyncConfig};',
  'relay: Option<Arc<RelaySync>>',
  'let relay = if let Some(relay_url) = config.relay_url.clone()',
  'RelaySync::new(',
  'relay_api_key: config.relay_api_key.clone()',
  'auth_secret: config.auth_secret.clone()',
  'self_arc.install_relay_callbacks(&relay_sync).await;',
  'relay_sync.start().await.map_err(ArkCoreError::from)?;',
  'relay .broadcast_live_change(entity.clone())',
  'relay.get_connected_peer_entries().await',
  'relay.stop();',
  'relay: self.relay.clone()',
]

for (const snippet of requiredSnippets) {
  const haystack = snippet.includes(' ') ? normalizedFfi : ffi
  assert(
    haystack.includes(snippet),
    `ffi.rs must include relay lifecycle wiring snippet: ${snippet}`,
  )
}

assert(
  readme.includes('Relay sync is wired into both `ark-core-rpc` and the UniFFI `ArkCore::start_sync` facade.'),
  'ARK core README must document UniFFI relay support',
)

console.log('verify-ffi-relay-wiring PASS')
