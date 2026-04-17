# Delphi

GTD-менеджер задач — часть экосистемы Kosmos. Три реализации: macOS (SwiftUI), Web/Desktop (Electron + Vue с shared `DesktopChrome`/`DesktopContentSurface`), Android (Kotlin + Compose).

## РџР»Р°С‚С„РѕСЂРјС‹

| РџР»Р°С‚С„РѕСЂРјР° | РџСѓС‚СЊ | РЎС‚РµРє |
|-----------|------|------|
| **macOS** | `swift/` | SwiftUI + SwiftData, macOS 15+ |
| **Web/Desktop** | `ts/` | Electron + Vue 3 + Vite + Pinia + reka-ui + Tailwind CSS 4; desktop shell uses `@kepler/visuals` |
| **Android** | `kotlin/` | Kotlin + Jetpack Compose + Material 3 + Room + Hilt, minSdk 28 |

РЎРёРЅС…СЂРѕРЅРёР·Р°С†РёСЏ: **P2P mesh (equal peers)** вЂ” РІСЃРµ РїР»Р°С‚С„РѕСЂРјС‹ Р·Р°РїСѓСЃРєР°СЋС‚ WS-СЃРµСЂРІРµСЂ Р РєР»РёРµРЅС‚. РќРµС‚ С…Р°Р±Р°.

## P2P Sync вЂ” СЂР°РІРЅРѕРїСЂР°РІРЅР°СЏ СЃРёРЅС…СЂРѕРЅРёР·Р°С†РёСЏ

### РђСЂС…РёС‚РµРєС‚СѓСЂР°

Р’СЃРµ РїР»Р°С‚С„РѕСЂРјС‹ (Electron, Android, macOS) вЂ” **СЂР°РІРЅРѕРїСЂР°РІРЅС‹Рµ РїРёСЂС‹**. РљР°Р¶РґС‹Р№ Р·Р°РїСѓСЃРєР°РµС‚ WebSocket-СЃРµСЂРІРµСЂ Р РїРѕРґРєР»СЋС‡Р°РµС‚СЃСЏ РєР°Рє РєР»РёРµРЅС‚ Рє РґСЂСѓРіРёРј РїРёСЂР°Рј. РќРµС‚ РІС‹РґРµР»РµРЅРЅРѕРіРѕ С…Р°Р±Р°.

| РџР»Р°С‚С„РѕСЂРјР° | WS-СЃРµСЂРІРµСЂ | WS-РєР»РёРµРЅС‚ | РџРѕСЂС‚ |
|-----------|-----------|-----------|------|
| **Electron** | `ws` library | `ws` library | 21531 |
| **Android** | Ktor CIO embedded | OkHttp | 21531 (fallback 21531-21541) |
| **macOS** | NWListener | URLSession | 21531 |

### Space Code

12-СЃРёРјРІРѕР»СЊРЅС‹Р№ Base32-Crockford РєРѕРґ (С„РѕСЂРјР°С‚ `XXXX-XXXX-XXXX`), РіРµРЅРµСЂРёСЂСѓРµРјС‹Р№ СЃР»СѓС‡Р°Р№РЅРѕ. РќРµ РєРѕРґРёСЂСѓРµС‚ IP.

- **Р›СЋР±Р°СЏ РїР»Р°С‚С„РѕСЂРјР°** РјРѕР¶РµС‚ СЃРѕР·РґР°С‚СЊ РїСЂРѕСЃС‚СЂР°РЅСЃС‚РІРѕ в†’ РіРµРЅРµСЂРёСЂСѓРµС‚ СЃР»СѓС‡Р°Р№РЅС‹Р№ РєРѕРґ
- **HMAC secret** = `normalizeCode(code)` (uppercase, Р±РµР· С‚РёСЂРµ) вЂ” РґР»СЏ Р°СѓС‚РµРЅС‚РёС„РёРєР°С†РёРё РїРёСЂРѕРІ
- **Space ID** = `SHA-256(normalized_code)[:16 hex]` = per-space DB identifier
- **РћРґРёРЅ Р°РєС‚РёРІРЅС‹Р№ Space** РЅР° СѓСЃС‚СЂРѕР№СЃС‚РІРѕ
- **Р‘РµР· РїСЂРѕСЃС‚СЂР°РЅСЃС‚РІР°** в†’ РїРѕРєР°Р·С‹РІР°РµС‚СЃСЏ СЌРєСЂР°РЅ РЅР°СЃС‚СЂРѕР№РєРё (SpaceSetupScreen/SpaceSetupView)

### QR-payload

```
ark://join?code=XXXX-XXXX-XXXX&addrs=192.168.1.70:21531,10.0.0.5:21531
```

РЎРѕРґРµСЂР¶РёС‚ Space code + РІСЃРµ РёР·РІРµСЃС‚РЅС‹Рµ Р°РґСЂРµСЃР° СЃРѕР·РґР°СЋС‰РµРіРѕ РїРёСЂР° (LAN, WAN, IPv6).

### Multi-address peer records

РљР°Р¶РґС‹Р№ РїРёСЂ С…СЂР°РЅРёС‚ СЃРїРёСЃРѕРє Р°РґСЂРµСЃРѕРІ (LAN, WAN, IPv6) РґР»СЏ РєР°Р¶РґРѕРіРѕ РёР·РІРµСЃС‚РЅРѕРіРѕ РїРёСЂР°. РџСЂРё РїРѕРґРєР»СЋС‡РµРЅРёРё РїСЂРѕР±СѓРµС‚ РІСЃРµ Р°РґСЂРµСЃР° **РїР°СЂР°Р»Р»РµР»СЊРЅРѕ**, Р±РµСЂС‘С‚ РїРµСЂРІС‹Р№ СѓСЃРїРµС€РЅС‹Р№.

### Peer list exchange (mesh discovery)

РџРѕСЃР»Рµ `hello` РїРёСЂС‹ РѕР±РјРµРЅРёРІР°СЋС‚СЃСЏ СЃРїРёСЃРєР°РјРё РёР·РІРµСЃС‚РЅС‹С… РїРёСЂРѕРІ СЃ РёС… Р°РґСЂРµСЃР°РјРё. Р­С‚Рѕ РїРѕР·РІРѕР»СЏРµС‚ РѕР±РЅР°СЂСѓР¶РёРІР°С‚СЊ РїРёСЂС‹ С‚СЂР°РЅР·РёС‚РёРІРЅРѕ Р±РµР· mDNS.

### UDP Beacon discovery (Syncthing-style)

Primary discovery вЂ” UDP broadcast РЅР° РїРѕСЂС‚ `LAN_SYNC_PORT + 1` (21532). РљР°Р¶РґС‹Р№ РїРёСЂ РєР°Р¶РґС‹Рµ 5 СЃРµРє С€Р»С‘С‚ beacon `{t, s=space_id, d=device_id, n=device_name, p=ws_port, a=[routable_addresses]}` РІРѕ РІСЃРµ С€РёСЂРѕРєРѕРІРµС‰Р°С‚РµР»СЊРЅС‹Рµ Р°РґСЂРµСЃР° IPv4-РїРѕРґСЃРµС‚РµР№. mDNS РЅРµ РёСЃРїРѕР»СЊР·СѓРµС‚СЃСЏ вЂ” Р±Р»РѕРєРёСЂСѓРµС‚СЃСЏ AP isolation РЅР° РјРЅРѕРіРёС… СЂРѕСѓС‚РµСЂР°С….

**Р”РµРґСѓРїР»РёРєР°С†РёСЏ РІС…РѕРґСЏС‰РёС… beacon'РѕРІ вЂ” РѕР±СЏР·Р°С‚РµР»СЊРЅР°.** Receiver РґРµСЂР¶РёС‚ `Map<device_id, SeenPeer>` Рё РІС‹Р·С‹РІР°РµС‚ `onPeerDiscovered` С‚РѕР»СЊРєРѕ РєРѕРіРґР° (Р°) device_id РЅРѕРІС‹Р№, РёР»Рё (Р±) СЃРїРёСЃРѕРє Р°РґСЂРµСЃРѕРІ РёР·РјРµРЅРёР»СЃСЏ. TTL 30 СЃ (2Г— beacon interval) вЂ” stale-Р·Р°РїРёСЃРё РІС‹С‡РёС‰Р°СЋС‚СЃСЏ, С‡С‚РѕР±С‹ peer РјРѕРі РїРµСЂРµР°РЅРѕРЅСЃРёСЂРѕРІР°С‚СЊСЃСЏ. **Р‘РµР· РґРµРґСѓРїР°** РєР°Р¶РґС‹Р№ beacon (РєР°Р¶РґС‹Рµ 5 СЃ) С‚СЂРёРіРіРµСЂРёР» reconnect в†’ Р±РµСЃРєРѕРЅРµС‡РЅС‹Р№ СЃРїР°Рј `[SyncClient] All addresses failed`.

### Р¤РёР»СЊС‚СЂР°С†РёСЏ Р°РґСЂРµСЃРѕРІ (Syncthing-style)

Beacon'С‹ Рё `ownAddresses` (РІ `hello`/`peer_list`) **MUST** СЃРѕРґРµСЂР¶Р°С‚СЊ С‚РѕР»СЊРєРѕ РјР°СЂС€СЂСѓС‚РёР·РёСЂСѓРµРјС‹Рµ Р°РґСЂРµСЃР°. Р¤РёР»СЊС‚СЂС‹:

- loopback (`internal=true`, `127.0.0.0/8`, `::1`)
- IPv4 link-local `169.254.0.0/16`
- IPv6 link-local `fe80::/10` Рё unique-local `fc00::/7`
- РІРёСЂС‚СѓР°Р»СЊРЅС‹Рµ РёРЅС‚РµСЂС„РµР№СЃС‹ РїРѕ РїСЂРµС„РёРєСЃСѓ РёРјРµРЅРё: `utun*`, `awdl*`, `llw*`, `bridge*`, `anpi*`, `docker*`, `br-*`, `veth*`, `virbr*`, `vboxnet*`, `vmnet*`, `tun*`, `tap*`, `wg*`, `tailscale*`, `vEthernet*`, `VMware*`, `VirtualBox*`, `rmnet*`, `dummy*`

Р РµР°Р»РёР·Р°С†РёСЏ: `packages/ark-core/rust/src/beacon.rs` (Rust beacon), `kotlin/.../BroadcastDiscovery.kt` (`sendBeacon`). **Р•СЃР»Рё РґРѕР±Р°РІР»СЏРµС€СЊ РЅРѕРІС‹Р№ СЃРїРѕСЃРѕР± Р°РЅРѕРЅСЃРёСЂРѕРІР°РЅРёСЏ Р°РґСЂРµСЃРѕРІ вЂ” С„РёР»СЊС‚СЂСѓР№ С‚Р°Рј Р¶Рµ.**

### Device name = host name, РЅРµ process name

Р’СЃРµ РїР»Р°С‚С„РѕСЂРјС‹ Р°РЅРѕРЅСЃРёСЂСѓСЋС‚ **СЂРµР°Р»СЊРЅРѕРµ РёРјСЏ СѓСЃС‚СЂРѕР№СЃС‚РІР° РћРЎ**, Р° РЅРµ РёРјСЏ РїСЂРёР»РѕР¶РµРЅРёСЏ:

| РџР»Р°С‚С„РѕСЂРјР° | РСЃС‚РѕС‡РЅРёРє | РџСЂРёРјРµСЂ |
|-----------|----------|--------|
| **Electron** | `os.hostname()` СЃ trim `.local` | `Kirill-MacBook-Pro-437` |
| **Android** | `${Build.MANUFACTURER} ${Build.MODEL}` | `Nothing A063` |
| **macOS** | `Host.current().localizedName` | `Kirill's MacBook Pro` |

РҐРµР»РїРµСЂ РІ Electron: `getHostDeviceName()` РІ `ts/electron/main.ts`. Renderer-РїСЂРѕС†РµСЃСЃ РїРµСЂРµРґР°С‘С‚ РїСѓСЃС‚СѓСЋ СЃС‚СЂРѕРєСѓ РІ `lan-sync:start`, main-РїСЂРѕС†РµСЃСЃ РІСЃРµРіРґР° РїРѕРґСЃС‚Р°РІР»СЏРµС‚ host name. **РќРёРєРѕРіРґР°** РЅРµ Р·Р°С…Р°СЂРґРєРѕР¶РёРІР°Р№ `"Delphi Electron"` РёР»Рё РёРјСЏ РїСЂРѕС†РµСЃСЃР°.

### Single-session-per-device РЅР° SyncServer

`SyncServer.peers` РІРЅСѓС‚СЂРё С…СЂР°РЅРёС‚ СЃРµСЃСЃРёРё РєР»СЋС‡РѕРј РїРѕ WS-СЃРѕРµРґРёРЅРµРЅРёСЋ, РЅРѕ **РІРЅРµС€РЅРµ РІРёРґРёРјРѕ** РґРѕР»Р¶РЅРѕ Р±С‹С‚СЊ **РѕРґРЅРѕ СѓСЃС‚СЂРѕР№СЃС‚РІРѕ = РѕРґРЅР° Р·Р°РїРёСЃСЊ**:

1. Р’ `handleHello`: РїРѕСЃР»Рµ Р°СѓС‚РµРЅС‚РёС„РёРєР°С†РёРё РЅРѕРІРѕР№ СЃРµСЃСЃРёРё вЂ” Р·Р°РєСЂС‹С‚СЊ РІСЃРµ РїСЂРѕС‡РёРµ authenticated-СЃРµСЃСЃРёРё СЃ С‚РµРј Р¶Рµ `device_id` (`CloseReason.NORMAL`, reason `superseded`). РџСЂРµРґРІР°СЂРёС‚РµР»СЊРЅРѕ СЃРЅСЏС‚СЊ С„Р»Р°Рі `authenticated` РЅР° stale-СЃРµСЃСЃРёСЏС…, С‡С‚РѕР±С‹ РёС… `close` handler РЅРµ РґС‘СЂРЅСѓР» Р»РёС€РЅРёР№ `onPeerDisconnected`.
2. `getConnectedPeers()` **MUST** РґРµРґСѓРїРёС‚СЊ РїРѕ `device_id` (`LinkedHashMap<device_id, name>`) вЂ” Р·Р°С‰РёС‚Р° РЅР° СЃР»СѓС‡Р°Р№ РіРѕРЅРєРё РјРµР¶РґСѓ handshake Рё eviction.
3. Р’ РєРѕРѕСЂРґРёРЅР°С‚РѕСЂРµ (`PeerManager.updatePeerCounts`): РјРµСЂР¶ inbound-СЃРµСЃСЃРёР№ (`SyncServer`) Рё outbound-РєР»РёРµРЅС‚Р° (`LanSyncClient`) РґРµРґСѓРїРёС‚СЃСЏ РїРѕ `device_id`. Р”Р»СЏ СЌС‚РѕРіРѕ `LanSyncClient.ServerInfo` С…СЂР°РЅРёС‚ `deviceId` СЃРµСЂРІРµСЂР°. Р¤РёР·РёС‡РµСЃРєРѕРµ СѓСЃС‚СЂРѕР№СЃС‚РІРѕ, РїРѕРґРєР»СЋС‡С‘РЅРЅРѕРµ РІ РѕР±Рµ СЃС‚РѕСЂРѕРЅС‹, = РѕРґРЅР° Р·Р°РїРёСЃСЊ.

### РџСЂРѕС‚РѕРєРѕР» СЃРёРЅС…СЂРѕРЅРёР·Р°С†РёРё

1. **hello** вЂ” РєР»РёРµРЅС‚ РѕС‚РїСЂР°РІР»СЏРµС‚ РїСЂРё РїРѕРґРєР»СЋС‡РµРЅРёРё, СЃРµСЂРІРµСЂ РѕС‚РІРµС‡Р°РµС‚ `hello_ack`
2. **peer_list** вЂ” РѕР±РјРµРЅ РёР·РІРµСЃС‚РЅС‹РјРё РїРёСЂР°РјРё Рё РёС… Р°РґСЂРµСЃР°РјРё РґР»СЏ mesh discovery
3. **version_vector** вЂ” РѕР±РјРµРЅ version vectors, РІС‹С‡РёСЃР»РµРЅРёРµ diff
4. **batch sync** вЂ” РїР°С‡РєРё РґРѕ 100 РёР·РјРµРЅРµРЅРёР№, РєР°Р¶РґР°СЏ СЃ ACK (`batch_ack`)
5. **live mode** вЂ” РїРѕСЃР»Рµ Р·Р°РІРµСЂС€РµРЅРёСЏ sync, РјСѓС‚Р°С†РёРё РёРґСѓС‚ РєР°Рє `live_change` СЃ `live_ack`

- **РљРѕРЅС„Р»РёРєС‚-СЂРµР·РѕР»СЋС†РёСЏ**: HLC-based Last-Writer-Wins (Hybrid Logical Clock)
- **Version vector**: РїРµСЂСЃРёСЃС‚РёС‚СЃСЏ РІ `sync_kv` (Electron/sidecar), DataStore (Android), UserDefaults (macOS)
- **РЎСѓС‰РЅРѕСЃС‚Рё**: todo, project, area, tag, heading

### РљР»СЋС‡РµРІС‹Рµ С„Р°Р№Р»С‹

| Р¤Р°Р№Р» | Р РѕР»СЊ |
|------|------|
| `packages/ark-core/rust/src/sync_server.rs` | Rust WS-СЃРµСЂРІРµСЂ (РІСЃРµ РїР»Р°С‚С„РѕСЂРјС‹ С‡РµСЂРµР· UniFFI / sidecar) |
| `packages/ark-core/rust/src/sync_client.rs` | Rust WS-РєР»РёРµРЅС‚ СЃ address racing |
| `packages/ark-core/rust/src/beacon.rs` | UDP Beacon discovery (РїРѕСЂС‚ 21532) |
| `packages/ark-core/rust/src/relay_transport.rs` | Outbound relay WebSocket РєР»РёРµРЅС‚ (backoff, offline outbox) |
| `packages/ark-core/rust/src/mesh.rs` | MeshCoordinator: LAN + relay, РґРµРґСѓРїР»РёРєР°С†РёСЏ РёР·РјРµРЅРµРЅРёР№ |
| `packages/ark-core/rust/src/ffi.rs` | UniFFI facade (ArkCore, FfiSyncConfig, ArkEventListener) |
| `packages/arksync-node/src/ark-client.ts` | `@arksync/node` ArkClient вЂ” TypeScript РѕР±С‘СЂС‚РєР° РЅР°Рґ sidecar IPC |
| `ts/electron/main.ts` | Electron main: ArkClient РёР· @arksync/node, stale-sync reset, macOS application menu |
| `ts/electron/sidecar.ts` | SidecarClient: С‚РѕР»СЊРєРѕ DB ops |
| `ts/src/services/sync/lan-protocol.ts` | РћР±С‰РёРµ С‚РёРїС‹, HLC, diff, batch splitting (standalone) |
| `ts/src/store/todos.ts` | CRUD + `broadcastToLanSync()` РЅР° РєР°Р¶РґРѕР№ РјСѓС‚Р°С†РёРё |
| `kotlin/.../data/sync/PeerManager.kt` | Android РєРѕРѕСЂРґРёРЅР°С‚РѕСЂ: UniFFI `ArkCore.startSync()` |

### Р’Р°Р¶РЅС‹Рµ РїСЂР°РІРёР»Р° СЂРµР°Р»РёР·Р°С†РёРё

- Vue 3 reactive proxies **MUST** Р±С‹С‚СЊ deep-cloned С‡РµСЂРµР· `JSON.parse(JSON.stringify())` РїРµСЂРµРґ Electron IPC (structured clone РЅРµ РјРѕР¶РµС‚ СЃРµСЂРёР°Р»РёР·РѕРІР°С‚СЊ Proxy-РѕР±СЉРµРєС‚С‹)
- Electron main **MUST** СЃР±СЂР°СЃС‹РІР°С‚СЊ Р»РѕРєР°Р»СЊРЅРѕРµ sync-state (`syncActive`, peer cache, С‚РµРєСѓС‰РёР№ runtime), РµСЃР»Рё sidecar РІРѕР·РІСЂР°С‰Р°РµС‚ `Sync not running`; РёРЅР°С‡Рµ UI РїСЂРѕРґРѕР»Р¶РёС‚ СЃР»Р°С‚СЊ `broadcast_change` РІ РјС‘СЂС‚РІС‹Р№ runtime Рё СЃРїР°РјРёС‚СЊ warnings
- macOS/Electron: **РќР•** СЃС‚Р°РІРёС‚СЊ `Menu.setApplicationMenu(null)` РІ Delphi; РёСЃРїРѕР»СЊР·СѓР№ РЅРѕСЂРјР°Р»СЊРЅС‹Р№ application menu template, РёРЅР°С‡Рµ РІРѕР·РјРѕР¶РµРЅ Cocoa warning `representedObject is not a WeakPtrToElectronMenuModelAsNSObject`
- Android version vector **MUST** РїРµСЂСЃРёСЃС‚РёС‚СЊСЃСЏ РІ DataStore, **РќР•** СЂРµРіРµРЅРµСЂРёСЂРѕРІР°С‚СЊСЃСЏ СЃ `Instant.now()` РїСЂРё reconnect
- OkHttp WebSocket: Р±РµР· `pingInterval` (СЃРµСЂРІРµСЂ С€Р»С‘С‚ WS-level pings), `readTimeout=0`
- **РќРµС‚ РєРЅРѕРїРєРё "РћС‡РёСЃС‚РёС‚СЊ РґР°РЅРЅС‹Рµ"** вЂ” РґР°РЅРЅС‹Рµ СѓРґР°Р»СЏСЋС‚СЃСЏ С‚РѕР»СЊРєРѕ С‡РµСЂРµР· СЃРёСЃС‚РµРјРЅС‹Рµ РЅР°СЃС‚СЂРѕР№РєРё (Settings в†’ Apps)

### РџРѕРІРµРґРµРЅРёРµ СЃРёРЅС…СЂРѕРЅРёР·Р°С†РёРё

- **Initial sync**: version vectors РѕР±РјРµРЅРёРІР°СЋС‚СЃСЏ, diff РІС‹С‡РёСЃР»СЏРµС‚СЃСЏ, Р±Р°С‚С‡Рё РѕС‚РїСЂР°РІР»СЏСЋС‚СЃСЏ СЃ ACK
- **Live mode**: РјСѓС‚Р°С†РёРё С‚СЂР°РЅСЃР»РёСЂСѓСЋС‚СЃСЏ РєР°Рє `live_change` СЃ `live_ack`
- **Reconnection**: Р°РІС‚РѕРјР°С‚РёС‡РµСЃРєРёР№ СЃ exponential backoff
- **Persistence**: РґР°РЅРЅС‹Рµ СЃРѕС…СЂР°РЅСЏСЋС‚СЃСЏ РјРµР¶РґСѓ reconnect вЂ” space С…СЂР°РЅРёС‚ РІСЃРµС… РїРёСЂРѕРІ Рё Р·Р°РґР°С‡Рё

### Р¤Р°Р№Р»С‹ РїРѕ РїР»Р°С‚С„РѕСЂРјР°Рј (Space UI)

| РџР»Р°С‚С„РѕСЂРјР° | SpaceManager | SpaceSetupUI |
|-----------|-------------|--------------|
| **TS/Electron** | `src/services/space/space-manager.ts` | `src/components/SpaceSetup.vue` |
| **Swift** | `Delphi/Space/SpaceManager.swift` | `Delphi/Space/SpaceSetupView.swift` |
| **Kotlin** | `data/space/SpaceManager.kt` | `ui/screens/space/SpaceSetupScreen.kt` |

> РџРѕРґСЂРѕР±РЅР°СЏ РґРѕРєСѓРјРµРЅС‚Р°С†РёСЏ РїРѕ РєР°Р¶РґРѕР№ РїР»Р°С‚С„РѕСЂРјРµ:
> - `swift/AGENTS.md` вЂ” macOS SwiftUI
> - `kotlin/AGENTS.md` вЂ” Android Kotlin
> РђСЂС…РёС‚РµРєС‚СѓСЂР° TS/Electron РѕРїРёСЃР°РЅР° РЅРёР¶Рµ.

## РќР°РІРёРіР°С†РёСЏ РїРѕ СѓРјРѕР»С‡Р°РЅРёСЋ

- **Android**: РіР»Р°РІРЅС‹Р№ СЌРєСЂР°РЅ вЂ” **РЎРµРіРѕРґРЅСЏ**
- **macOS/Electron**: РіР»Р°РІРЅС‹Р№ СЌРєСЂР°РЅ вЂ” **Р’С…РѕРґСЏС‰РёРµ** (Inbox)

## РњРѕРґРµР»Рё РґР°РЅРЅС‹С…

### TodoItem (Р·Р°РґР°С‡Р°)
```
id            UUID
title         String
notes         String?
priority      none | low | medium | high
scheduledDate Date?          вЂ” РєРѕРіРґР° Р·Р°РїР»Р°РЅРёСЂРѕРІР°РЅР°
deadline      Date?          вЂ” РґРµРґР»Р°Р№РЅ
reminderDate  Date?          вЂ” РЅР°РїРѕРјРёРЅР°РЅРёРµ
isToday       Bool           вЂ” РїРѕРјРµС‡РµРЅР° РєР°Рє "СЃРµРіРѕРґРЅСЏ"
isEvening     Bool           вЂ” РїРѕРјРµС‡РµРЅР° РєР°Рє "РІРµС‡РµСЂ"
isSomeday     Bool           вЂ” РѕС‚Р»РѕР¶РµРЅР° РЅР° "РїРѕС‚РѕРј"
isCompleted   Bool
completedAt   Date?
isCancelled   Bool
cancelledAt   Date?
isTrashed     Bool
sortOrder     Int            вЂ” РїРѕСЂСЏРґРѕРє РІ СЃРїРёСЃРєРµ
headingID     UUID?          вЂ” Р·Р°РіРѕР»РѕРІРѕРє РІРЅСѓС‚СЂРё РїСЂРѕРµРєС‚Р°
createdAt     Date

РЎРІСЏР·Рё:
  в†’ Project?                 вЂ” РїСЂРѕРµРєС‚ (РѕРїС†РёРѕРЅР°Р»СЊРЅРѕ)
  в†’ Area?                    вЂ” РѕР±Р»Р°СЃС‚СЊ (РѕРїС†РёРѕРЅР°Р»СЊРЅРѕ)
  в†’ [Tag]                    вЂ” С‚РµРіРё (many-to-many)
  в†’ [ChecklistItem]          вЂ” С‡РµРєР»РёСЃС‚ (cascade delete)
  в†’ RecurrenceData?          вЂ” РїРѕРІС‚РѕСЂРµРЅРёРµ
```

### Project (РїСЂРѕРµРєС‚)
```
id            UUID
title         String
notes         String?
status        active | someday | completed
scheduledDate Date?
deadline      Date?
sortOrder     Int
colorTag      String?
createdAt     Date

РЎРІСЏР·Рё:
  в†’ [TodoItem]               вЂ” Р·Р°РґР°С‡Рё
  в†’ [Heading]                вЂ” Р·Р°РіРѕР»РѕРІРєРё-СЃРµРєС†РёРё (cascade delete)
  в†’ Area?                    вЂ” РѕР±Р»Р°СЃС‚СЊ
```

### Area (РѕР±Р»Р°СЃС‚СЊ)
```
id            UUID
title         String
sortOrder     Int
createdAt     Date

РЎРІСЏР·Рё:
  в†’ [Project]                вЂ” РїСЂРѕРµРєС‚С‹
  в†’ [TodoItem]               вЂ” Р·Р°РґР°С‡Рё РІРЅРµ РїСЂРѕРµРєС‚РѕРІ
```

### Tag
```
id            UUID
title         String
color         String?
createdAt     Date

РЎРІСЏР·Рё:
  в†’ [TodoItem]               вЂ” many-to-many
```

### Heading (Р·Р°РіРѕР»РѕРІРѕРє РІРЅСѓС‚СЂРё РїСЂРѕРµРєС‚Р°)
```
id            UUID
title         String
sortOrder     Int

РЎРІСЏР·Рё:
  в†’ Project                  вЂ” СЂРѕРґРёС‚РµР»СЊСЃРєРёР№ РїСЂРѕРµРєС‚
```

### ChecklistItem (СЌР»РµРјРµРЅС‚ С‡РµРєР»РёСЃС‚Р°)
```
id            UUID
title         String
isCompleted   Bool
sortOrder     Int

РЎРІСЏР·Рё:
  в†’ TodoItem                 вЂ” СЂРѕРґРёС‚РµР»СЊСЃРєР°СЏ Р·Р°РґР°С‡Р°
```

### RecurrenceData (РїРѕРІС‚РѕСЂРµРЅРёРµ)
```
frequency       daily | weekly | monthly | yearly
interval        Int (РєР°Р¶РґС‹Рµ N РµРґРёРЅРёС†)
recurrenceType  fixed | afterCompletion
daysOfWeek      [Int]?       вЂ” РґР»СЏ weekly
endDate         Date?
```

## Smart Lists (СѓРјРЅС‹Рµ СЃРїРёСЃРєРё)

| РЎРїРёСЃРѕРє | Р¤РёР»СЊС‚СЂ |
|--------|--------|
| **Р’С…РѕРґСЏС‰РёРµ** | РќРµС‚ РїСЂРѕРµРєС‚Р°, РЅРµ today/evening/someday, РЅРµ Р·Р°РІРµСЂС€РµРЅР°, РЅРµ РІ РєРѕСЂР·РёРЅРµ |
| **РЎРµРіРѕРґРЅСЏ** | isToday=true РР›Р scheduledDate=СЃРµРіРѕРґРЅСЏ, РЅРµ Р·Р°РІРµСЂС€РµРЅР° |
| **РџР»Р°РЅС‹** | scheduledDate РІ Р±СѓРґСѓС‰РµРј, РЅРµ today/someday, РЅРµ Р·Р°РІРµСЂС€РµРЅР° |
| **РљРѕРіРґР° СѓРіРѕРґРЅРѕ** | РќРµ someday, РЅРµ Р·Р°РІРµСЂС€РµРЅР°, РЅРµ РІ РєРѕСЂР·РёРЅРµ, РЅРµС‚ scheduledDate РІ Р±СѓРґСѓС‰РµРј |
| **РџРѕС‚РѕРј** | isSomeday=true, РЅРµ Р·Р°РІРµСЂС€РµРЅР° |
| **Р–СѓСЂРЅР°Р»** | isCompleted=true РР›Р isCancelled=true |
| **РљРѕСЂР·РёРЅР°** | isTrashed=true |

## РљР»Р°РІРёС€Рё (macOS)

| Р”РµР№СЃС‚РІРёРµ | РЎРѕС‡РµС‚Р°РЅРёРµ |
|----------|-----------|
| РќРѕРІР°СЏ Р·Р°РґР°С‡Р° | Cmd+N |
| РќРѕРІС‹Р№ РїСЂРѕРµРєС‚ | Cmd+Option+N |
| РќРѕРІС‹Р№ Р·Р°РіРѕР»РѕРІРѕРє | Cmd+Shift+N |
| Р—Р°РІРµСЂС€РёС‚СЊ | Cmd+K |
| РћС‚РјРµРЅРёС‚СЊ | Cmd+Option+K |
| Р”СѓР±Р»РёСЂРѕРІР°С‚СЊ | Cmd+D |
| РџРµСЂРµРјРµСЃС‚РёС‚СЊ | Cmd+Shift+M |
| РЎРµРіРѕРґРЅСЏ | Cmd+T |
| Р’РµС‡РµСЂ | Cmd+E |
| РљРѕРіРґР° СѓРіРѕРґРЅРѕ | Cmd+R |
| РџРѕС‚РѕРј | Cmd+O |
| Р”РµРґР»Р°Р№РЅ | Cmd+Shift+D |
| РўРµРіРё | Cmd+Shift+T |
| РџРѕРёСЃРє | Cmd+F |
| РЎР°Р№РґР±Р°СЂ | Cmd+/ |
| Smart List 1-6 | Cmd+1..6 |

## РђСЂС…РёС‚РµРєС‚СѓСЂР° TS-РІРµСЂСЃРёРё (`ts/`)

### РЎС‚СЂСѓРєС‚СѓСЂР°

```
ts/
в”њв”Ђв”Ђ sidecar/               вЂ” Rust sidecar (delphi-db)
в”‚   в”њв”Ђв”Ђ Cargo.toml         вЂ” rusqlite (bundled), serde_json
в”‚   в””в”Ђв”Ђ src/main.rs        вЂ” stdin/stdout JSON RPC + SQLite (WAL)
в”њв”Ђв”Ђ electron/              вЂ” Electron main process
│   ├── main.ts            — точка входа, IPC-хендлеры (db:*, fs:*, lan-sync:*) и native window chrome bootstrap для macOS/Windows
в”‚   в””в”Ђв”Ђ sidecar.ts         вЂ” SidecarClient: spawn delphi-db, JSON queue, dbLoadAll/upsertTodo/вЂ¦
в”њв”Ђв”Ђ src/
в”‚   в”њв”Ђв”Ђ App.vue            вЂ” РєРѕСЂРЅРµРІРѕР№ layout, connection bootstrap, P2P sync bridge
в”‚   в”њв”Ђв”Ђ main.ts            вЂ” createApp, router, Pinia
│   ├── components/        — app-level composition/components (SideBar adapter, QuickEntry wrapper, QuickOpen, …); shared visuals come from `@kepler/visuals`, desktop shell должен собираться из `DesktopChrome` + `DesktopContentSurface`
в”‚   в”њв”Ђв”Ђ pages/             вЂ” route views (TodayPage, AllTaskPage, WeekPage, ProjectPage, вЂ¦)
в”‚   в”њв”Ђв”Ђ composables/       вЂ” useSmartList, useQuickEntry, useTheme, useSidebarState
в”‚   в”њв”Ђв”Ђ store/
в”‚   в”‚   в”њв”Ђв”Ђ todos.ts       вЂ” Pinia store: Р·Р°РґР°С‡Рё, РїСЂРѕРµРєС‚С‹, CRUD в†’ localDb + lanSync
в”‚   в”‚   в””в”Ђв”Ђ tasks.ts       вЂ” РІСЃРїРѕРјРѕРіР°С‚РµР»СЊРЅС‹Рµ СѓС‚РёР»РёС‚С‹ РґР»СЏ Р·Р°РґР°С‡
в”‚   в”њв”Ђв”Ђ services/
в”‚   в”‚   в”њв”Ђв”Ђ sync/          вЂ” lan-protocol, hlc, ark-types
в”‚   в”‚   в”њв”Ђв”Ђ api/           вЂ” HTTP helpers
в”‚   в”‚   в”њв”Ђв”Ђ filters/       вЂ” smart list С„РёР»СЊС‚СЂС‹
в”‚   в”‚   в”њв”Ђв”Ђ gemini/        вЂ” РіРѕР»РѕСЃРѕРІРѕР№ РІРІРѕРґ (Gemini Live API)
в”‚   в”‚   в”њв”Ђв”Ђ recurrence/    вЂ” РїРѕРІС‚РѕСЂСЏСЋС‰РёРµСЃСЏ Р·Р°РґР°С‡Рё
в”‚   в”‚   в”њв”Ђв”Ђ runtime/       вЂ” runtime utilities
в”‚   в”‚   в””в”Ђв”Ђ storage/       вЂ” local-db.ts (sidecar bridge), localStorage wrappers
в”‚   в”њв”Ђв”Ђ router/            вЂ” vue-router РєРѕРЅС„РёРі
в”‚   в””в”Ђв”Ђ types/             вЂ” TypeScript С‚РёРїС‹ (Task, Project, Priority, вЂ¦)
в””в”Ђв”Ђ vite.config.ts
```

### Rust Sidecar (Р»РѕРєР°Р»СЊРЅР°СЏ Р‘Р”)

`sidecar/` вЂ” Р±РёРЅР°СЂРЅРёРє `delphi-db` РЅР° Rust (РїР°С‚С‚РµСЂРЅ РёР· Eden):
- **РџСЂРѕС‚РѕРєРѕР»**: stdin/stdout, РѕРґРЅР° СЃС‚СЂРѕРєР° = РѕРґРёРЅ JSON-Р·Р°РїСЂРѕСЃ/РѕС‚РІРµС‚
- **Р‘Р”**: `<userData>/delphi.db` (SQLite WAL, rusqlite bundled)
- **РўР°Р±Р»РёС†С‹**: `todos`, `projects`, `areas`, `tags`, `headings`, `sync_kv`
- **IPC**: Electron main в†’ `electron/sidecar.ts` в†’ `SidecarClient` в†’ spawn РїСЂРѕС†РµСЃСЃ

**РћРїРµСЂР°С†РёРё**: `init` (РѕС‚РєСЂС‹С‚СЊ Р‘Р”), `load_all`, `upsert_todo`, `delete_todo`, `batch_upsert_todos`, `upsert_project`, `delete_project`, `upsert_area`, `upsert_tag`, `upsert_heading`, `delete_heading`, `get_sync_kv`, `set_sync_kv`, `clear_all`

**РџРѕС‚РѕРє РґР°РЅРЅС‹С… РїСЂРё СЃС‚Р°СЂС‚Рµ (Electron)**:
```
App.vue в†’ isLocalDbAvailable()
  true  в†’ loadAllFromLocalDb() в†’ IPC db:loadAll в†’ sidecar в†’ SQLite  (РјРіРЅРѕРІРµРЅРЅРѕ, offline)
```

**РљР°Р¶РґР°СЏ РјСѓС‚Р°С†РёСЏ (store/todos.ts)**:
```
addTodo/updateTodo/вЂ¦ в†’ localDbUpsertTodo (fire & forget)
                     в†’ broadcastToLanSync() (live_change to connected peers)
```

**РЎР±РѕСЂРєР°**:
```bash
bun run build:sidecar:dev   # cargo build (debug)
bun run build:sidecar       # cargo build --release
bun run dev                 # build sidecar:dev + vite
```

### UI-Р±РёР±Р»РёРѕС‚РµРєР°

reka-ui (headless Vue 3 components): Tooltip, Dialog Рё С‚.Рґ. РЎС‚РёР»Рё вЂ” Tailwind CSS 4 СЃ CSS-РїРµСЂРµРјРµРЅРЅС‹РјРё (`--background`, `--foreground`, `--border`, `--popover`, `--muted-foreground`).

**Shared UI single source of truth:** Delphi TS **MUST** брать общие визуальные компоненты из workspace-пакета `@kepler/visuals` через его public API. Desktop app shell uses `DesktopChrome` for the window frame and `DesktopContentSurface` for the inner content area. Не держи локальные копии вроде `src/components/SideBarButton.vue`; app-level компоненты в `src/components/` должны быть только адаптерами/композицией над shared package.
Titlebar navigation for routed desktop pages should use shared `TitlebarHistoryControls` from `@kepler/visuals`; disabled-state must come from the current Vue Router history state instead of local guessed counters.

### Sidebar zen-mode width

`src/composables/useSidebarState.ts` С…СЂР°РЅРёС‚ module-level `sidebarHidden` Рё РѕС‚РґР°С‘С‚ `wrapClass`/`wrapStyle` РґР»СЏ СЃС‚СЂР°РЅРёС† РєРѕРЅС‚РµРЅС‚Р°. **Smart-list СЃС‚СЂР°РЅРёС†С‹, ProjectPage Рё SettingsPage MUST РёСЃРїРѕР»СЊР·РѕРІР°С‚СЊ СЌС‚РѕС‚ composable** РґР»СЏ РѕР±С‰РµРіРѕ РєРѕРЅС‚РµР№РЅРµСЂР° РєРѕРЅС‚РµРЅС‚Р°, С‡С‚РѕР±С‹ РїСЂРё СЃРєСЂС‹С‚РёРё СЃР°Р№РґР±Р°СЂР° layout РїР»Р°РІРЅРѕ РїРµСЂРµРєР»СЋС‡Р°Р»СЃСЏ РјРµР¶РґСѓ `100%` Рё `var(--bringhurst-wide)` РІРјРµСЃС‚Рѕ СЂРµР·РєРѕРіРѕ reflow.

### РЎРѕСЃС‚РѕСЏРЅРёРµ РїРѕРґРєР»СЋС‡РµРЅРёСЏ (App.vue)

РРЅРґРёРєР°С‚РѕСЂ-С‚РѕС‡РєР° РІ РїСЂР°РІРѕРј РІРµСЂС…РЅРµРј СѓРіР»Сѓ:
- **Р—РµР»С‘РЅС‹Р№** (`online`) вЂ” LAN sync Р°РєС‚РёРІРµРЅ, live mode СЂР°Р±РѕС‚Р°РµС‚
- **Р–С‘Р»С‚С‹Р№ РїСѓР»СЊСЃРёСЂСѓСЋС‰РёР№** (`syncing`) вЂ” СѓСЃС‚Р°РЅР°РІР»РёРІР°РµС‚СЃСЏ СЃРѕРµРґРёРЅРµРЅРёРµ / batch sync
- **РљСЂР°СЃРЅС‹Р№** (`offline`) вЂ” РЅРµС‚ РїРѕРґРєР»СЋС‡С‘РЅРЅС‹С… РєР»РёРµРЅС‚РѕРІ РёР»Рё РїСЂРѕСЃС‚СЂР°РЅСЃС‚РІРѕ РЅРµ СЃРѕР·РґР°РЅРѕ

РџСЂРё РЅР°РІРµРґРµРЅРёРё вЂ” С‚СѓР»С‚РёРї (reka-ui Tooltip) СЃ РѕРїРёСЃР°РЅРёРµРј С‚РµРєСѓС‰РµРіРѕ СЃРѕСЃС‚РѕСЏРЅРёСЏ.

### P2P Sync (Electron main process)

Electron РёСЃРїРѕР»СЊР·СѓРµС‚ `@arksync/node` в†’ `ArkClient` в†’ IPC Рє Rust sidecar `ark-core-rpc`. TS-СѓСЂРѕРІРµРЅСЊ РЅРµ СЃРѕРґРµСЂР¶РёС‚ WebSocket-РєРѕРґР° вЂ” РІРµСЃСЊ P2P РІ Rust:

- **`packages/arksync-node/src/ark-client.ts`** вЂ” `ArkClient`: `start()`, `stop()`, `broadcastChange()`, `onPeerConnected`, `onEntityChanged`
- **Sidecar IPC** С‡РµСЂРµР· `electron/main.ts` в†’ `lan-sync:start`, `lan-sync:change`, `lan-sync:broadcast`
- **Version vector**: РїРµСЂСЃРёСЃС‚РёСЂСѓРµС‚СЃСЏ РІ sidecar `sync_kv` (SQLite)
- **Incoming changes**: IPC `lan-sync:change` в†’ renderer в†’ Pinia store

## РЎРµСЂРІРёСЃС‹

### TodoFilterService
Р¦РµРЅС‚СЂР°Р»РёР·РѕРІР°РЅРЅР°СЏ С„РёР»СЊС‚СЂР°С†РёСЏ РїРѕ smart lists. РљРµС€РёСЂСѓРµС‚ СЃС‡С‘С‚С‡РёРєРё.

### LemmaSearchService
РњРѕСЂС„РѕР»РѕРіРёС‡РµСЃРєРёР№ РїРѕРёСЃРє С‡РµСЂРµР· NLTagger. Р СѓСЃСЃРєРёРµ СЃРєР»РѕРЅРµРЅРёСЏ, Р°РЅРіР»РёР№СЃРєРёРµ С„РѕСЂРјС‹.

### NaturalDateParser
РџР°СЂСЃРёРЅРі РґР°С‚ РёР· С‚РµРєСЃС‚Р°:
- "СЃРµРіРѕРґРЅСЏ", "Р·Р°РІС‚СЂР°", "РїРѕСЃР»РµР·Р°РІС‚СЂР°"
- "3d", "2w", "3mo", "1y"
- "in 5 days", "next monday"
- Р СѓСЃСЃРєРёРµ РґРЅРё РЅРµРґРµР»Рё, РѕС‚РЅРѕСЃРёС‚РµР»СЊРЅС‹Рµ С„СЂР°Р·С‹

## РЎРёРЅС…СЂРѕРЅРёР·Р°С†РёСЏ

### P2P Sync (РѕСЃРЅРѕРІРЅРѕР№ СЂРµР¶РёРј)

РћРїРёСЃР°РЅ РІС‹С€Рµ. РџСЂРё РїРµСЂРІРѕРј Р·Р°РїСѓСЃРєРµ Р±РµР· РїСЂРѕСЃС‚СЂР°РЅСЃС‚РІР° в†’ РїРѕРєР°Р·Р°С‚СЊ SpaceSetupScreen/View.

### Р“РґРµ Р¶РёРІС‘С‚ РєРѕРґ (TS/Electron)

| Р¤Р°Р№Р» | Р РѕР»СЊ |
|------|------|
| `packages/arksync-node/src/ark-client.ts` | `@arksync/node` ArkClient вЂ” TypeScript РѕР±С‘СЂС‚РєР° РЅР°Рґ sidecar IPC |
| `electron/main.ts` | ArkClient РёР· @arksync/node, IPC-С…РµРЅРґР»РµСЂС‹ |
| `electron/sidecar.ts` | SidecarClient: С‚РѕР»СЊРєРѕ DB ops |
| `src/services/sync/ark-types.ts` | РўРёРїС‹ ArkChange, РјР°РїРїРёРЅРі СЃСѓС‰РЅРѕСЃС‚РµР№, settings helpers |
| `src/services/sync/lan-protocol.ts` | РћР±С‰РёРµ С‚РёРїС‹, HLC, diff, batch splitting |
| `store/todos.ts` | CRUD + `broadcastToLanSync()` РЅР° РєР°Р¶РґРѕР№ РјСѓС‚Р°С†РёРё |
| `App.vue` | Bootstrap sync, РѕР±СЂР°Р±РѕС‚РєР° РІС…РѕРґСЏС‰РёС… РёР·РјРµРЅРµРЅРёР№ |

### РСЃС…РѕРґСЏС‰РёРµ РёР·РјРµРЅРµРЅРёСЏ (store/todos.ts)

РљР°Р¶РґР°СЏ РјСѓС‚Р°С†РёСЏ РІС‹Р·С‹РІР°РµС‚ `broadcastToLanSync()` вЂ” РѕС‚РїСЂР°РІР»СЏРµС‚ `live_change` РІСЃРµРј РїРѕРґРєР»СЋС‡С‘РЅРЅС‹Рј РїРёСЂР°Рј.

### UUID normalization

Р’СЃРµ UUID MUST be lowercase РЅР° РІСЃРµС… РїР»Р°С‚С„РѕСЂРјР°С…. Mac UUID РїРѕ СѓРјРѕР»С‡Р°РЅРёСЋ uppercase.
- **Swift**: `uuidString.lowercased()`
- **Kotlin**: `.lowercase()`
- **TS/Electron**: `.toLowerCase()`

### Sidecar: clear_all

РћРїРµСЂР°С†РёСЏ `clear_all` РІ Rust sidecar delphi-db вЂ” СѓРґР°Р»СЏРµС‚ РІСЃРµ СЃС‚СЂРѕРєРё РёР· С‚Р°Р±Р»РёС† `todos`, `projects`, `areas`, `tags`, `headings`, `sync_kv`.

### Relay С‚СЂР°РЅСЃРїРѕСЂС‚ (@arksync/node / Rust)

Relay WebSocket С‚СЂР°РЅСЃРїРѕСЂС‚ СЂРµР°Р»РёР·РѕРІР°РЅ РІ `packages/ark-core/rust/src/relay_transport.rs` Рё РєРѕРѕСЂРґРёРЅРёСЂСѓРµС‚СЃСЏ С‡РµСЂРµР· `mesh.rs`. `@arksync/node` ArkClient РїСЂРёРЅРёРјР°РµС‚ РѕРїС†РёРѕРЅР°Р»СЊРЅС‹Рµ `relayUrl` Рё `relayApiKey` вЂ” Р±РµР· РЅРёС… СЂР°Р±РѕС‚Р°РµС‚ С‚РѕР»СЊРєРѕ LAN.

## Р“РѕР»РѕСЃРѕРІРѕР№ РІРІРѕРґ (Web)

Google Gemini 2.5 Live API:
- РњРёРєСЂРѕС„РѕРЅ в†’ PCM 16kHz в†’ Gemini
- Function calling: create_task, update_last_task, delete_last_task
- РЇР·С‹Рє: СЂСѓСЃСЃРєРёР№ (РїСЂРёРѕСЂРёС‚РµС‚)

## Conventions

- РЇР·С‹Рє UI: СЂСѓСЃСЃРєРёР№
- Package manager: bun (workspace)
- РЁСЂРёС„С‚: Zed Mono Extended (web), СЃРёСЃС‚РµРјРЅС‹Р№ (macOS)
- РўРµРјР°: С‚С‘РјРЅР°СЏ РїРѕ СѓРјРѕР»С‡Р°РЅРёСЋ, РїРѕРґРґРµСЂР¶РєР° СЃРІРµС‚Р»РѕР№
- Path alias: `@/` в†’ `src/`
- Vue: Composition API + `<script setup lang="ts">`, Р±РµР· Options API
- State: Pinia stores (`defineStore`), `shallowRef` РґР»СЏ РїСЂРёРјРёС‚РёРІРѕРІ
- UI-РєРѕРјРїРѕРЅРµРЅС‚С‹: reka-ui (headless) + Tailwind CSS 4
- Р›РёРЅС‚РµСЂ: oxlint, С„РѕСЂРјР°С‚С‚РµСЂ: oxfmt (СЃ sortImports)
- РўРµСЃС‚С‹: vitest (unit), playwright (e2e)
