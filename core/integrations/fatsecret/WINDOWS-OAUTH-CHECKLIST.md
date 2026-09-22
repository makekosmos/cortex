# FatSecret Windows OAuth manual checklist

Status for this secret-free PR: **NOT_RUN**.

Perform on a clean Windows 10/11 checkout only after a FatSecret developer
consumer is provisioned and the callback URI is approved. Never place the
consumer secret, access token, or verifier in git, issue comments, screenshots,
or logs.

1. Set the public provider config (provider=fatsecret, clientId, API base URL);
   confirm no secret/token/password field is present.
2. Put the consumer secret in the OS credential manager through the production
   keyring adapter. Confirm integrations.json contains no secret.
3. Start the desktop OAuth connect flow and verify the system browser opens the
   FatSecret authorization URL with a short-lived request token.
4. Approve access in the existing FatSecret account; return through the
   registered loopback callback and complete the verifier exchange.
5. Restart Kosmos and confirm status remains connected using only keyring state.
6. Create one control food entry in FatSecret, run manual sync_now, and locate
   one ARK object with typeId=nutrition_entry_obj, provider fatsecret, the
   external ID, date/meal/food/serving identifiers, and available kcal/БЖУ.
7. Run the same sync twice and confirm the deterministic
   fatsecret-food-entry:<external_id> object is updated, not duplicated.
8. Run startup and scheduled sync; start two manual syncs concurrently and
   confirm only one provider request is in flight.
9. Disconnect; verify credentials/config connection state are removed while the
   imported ARK object remains.
10. Exercise invalid credentials, HTTP 429, malformed response, and offline
    cases; verify user-safe localized errors contain no token, secret, key,
    authorization header, or full payload.
11. Inspect application config, renderer state, ARK objects, and redacted logs
    for secret absence; capture evidence without copying sensitive values.
