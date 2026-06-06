# ark-relay-server

- **Path**: `services/relay-reference`

WebSocket-relay сервер для p2p-синхронизации ARK-пиров **через NAT**. Когда устройства не могут увидеть друг друга в LAN (разные подсети, NAT, мобильная сеть), relay-сервер выступает посредником.

## Что делает

Раздаёт sync-фреймы между подключёнными пирами:

- `hello` (включая `auth_nonce` + `auth_hmac` если space использует `auth_secret`)
- `version_vector`
- `sync_changes`
- `live_change`

Relay не хранит данные — он только пересылает фреймы между пирами одного `spaceId`. Сами данные остаются на устройствах.

## Как используется

Если `start_sync` в `ark-core-rpc` получает `relay_url`, sidecar поднимает relay-bridge **параллельно** LAN sync. Bridge соединяется с relay по WSS и обменивается фреймами с другими пирами того же space'а.

```ts
const ark = new ArkClient({
  spaceId: "default",
  deviceId: "device-1",
  authSecret: "shared-secret",
  relayUrl: "wss://relay.example.com/space/default",
});
await ark.start();
```

## Безопасность

::: warning
Relay видит фреймы в открытом виде на WebSocket-уровне (если без TLS). Используй **WSS** в продакшене.

HMAC-auth (`auth_secret`) защищает от джойна пиров без секрета, но **не** шифрует трафик.
:::

## Связанные документы

- [Синхронизация](/concepts/sync).
- [ark-core](/packages/ark-core).
