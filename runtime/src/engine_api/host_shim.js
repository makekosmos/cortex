// Engine host shim for browser-opened .kspkg apps (KOS-299). Injected into
// every page served through the launch asset channel; it performs the
// one-time bootstrap exchange, then exposes the same `window.kosmosApp`
// page API the Electron host's preload defined. Nothing here trusts page
// JS: every request is re-authorized server-side against the launch grant.
//
// Credential flow:
//   launch_url#launch=<id>&code=<code>  (fragment — never sent to the Engine)
//   POST /v1/apps/launch/<id>/bootstrap {code}  →  broker_token + data_api
//   history.replaceState strips the fragment immediately.
// The session survives in-tab reloads via sessionStorage (per-tab storage),
// and is revoked on pagehide via sendBeacon; the lease TTL stays the backstop.
(function () {
  "use strict";
  var LAUNCH_PREFIX = "/v1/apps/launch/";
  var SESSION_KEY = "kosmos.launch";

  function persist(state) {
    try {
      sessionStorage.setItem(SESSION_KEY, JSON.stringify(state));
    } catch (_) {
      /* private-mode storage — the tab still works until reload */
    }
  }

  function restore() {
    try {
      var raw = sessionStorage.getItem(SESSION_KEY);
      if (!raw) return null;
      var state = JSON.parse(raw);
      if (!state || !state.launch_id || !state.broker_token || !state.expires_at) return null;
      if (Date.parse(state.expires_at) - Date.now() <= 1000) return null;
      return state;
    } catch (_) {
      return null;
    }
  }

  function arkUrl(launchId, op) {
    return LAUNCH_PREFIX + launchId + "/" + op;
  }

  // Mirrors the deleted host's launchRenewalDelayMs: renew ~30 s before
  // expiry, and on failure retry sooner but never past the deadline.
  function renewDelayMs(expiresAt, retry) {
    var until = Date.parse(expiresAt) - Date.now();
    if (!isFinite(until) || until <= 1000) return null;
    return retry ? Math.max(1000, Math.min(30000, until - 1000)) : Math.max(1000, until - 30000);
  }

  function install(state) {
    var ready = Promise.resolve(state);

    var api = {
      identity: { id: state.id, version: state.version },
      ark: {
        request: function (operation, params) {
          return ready.then(function (session) {
            return fetch(session.data_api, {
              method: "POST",
              headers: {
                "content-type": "application/json",
                "x-kosmos-launch-token": session.broker_token,
              },
              body: JSON.stringify({ operation: operation, params: params || {} }),
            }).then(function (r) {
              return r.json();
            });
          });
        },
        subscribe: function (handler) {
          var controller = new AbortController();
          ready.then(function (session) {
            return fetch(arkUrl(session.launch_id, "events"), {
              headers: { "x-kosmos-launch-token": session.broker_token },
              signal: controller.signal,
            }).then(function (resp) {
              if (!resp.ok || !resp.body) return;
              var reader = resp.body.getReader();
              var decoder = new TextDecoder();
              var buf = "";
              (function pump() {
                reader
                  .read()
                  .then(function (r) {
                    if (r.done) return;
                    buf += decoder.decode(r.value, { stream: true });
                    var cut;
                    while ((cut = buf.indexOf("\n\n")) >= 0) {
                      var block = buf.slice(0, cut);
                      buf = buf.slice(cut + 2);
                      var data = "";
                      block.split("\n").forEach(function (line) {
                        if (line.indexOf("data:") === 0) data += line.slice(5).trimStart();
                      });
                      if (data) {
                        try {
                          handler(JSON.parse(data));
                        } catch (_) {
                          /* a bad handler must not kill the stream */
                        }
                      }
                    }
                    pump();
                  })
                  .catch(function () {
                    /* aborted or connection dropped — the lease may be gone */
                  });
              })();
            });
          });
          return function () {
            controller.abort();
          };
        },
      },
    };

    var timer = null;
    function scheduleRenew(retry) {
      if (timer) clearTimeout(timer);
      var delay = renewDelayMs(state.expires_at, retry);
      if (delay === null) return; // lease is dead — the next request 403s
      timer = setTimeout(function () {
        fetch(arkUrl(state.launch_id, "renew"), {
          method: "POST",
          headers: { "x-kosmos-launch-token": state.broker_token },
        })
          .then(function (r) {
            return r.json();
          })
          .then(function (resp) {
            if (resp && resp.ok && resp.data && resp.data.expires_at) {
              state.expires_at = resp.data.expires_at;
              persist(state);
              scheduleRenew(false);
            } else {
              scheduleRenew(true);
            }
          })
          .catch(function () {
            scheduleRenew(true);
          });
      }, delay);
    }
    scheduleRenew(false);

    window.addEventListener("pagehide", function () {
      if (timer) clearTimeout(timer);
      try {
        navigator.sendBeacon(
          arkUrl(state.launch_id, "revoke"),
          JSON.stringify({ token: state.broker_token }),
        );
      } catch (_) {
        /* best effort — the TTL expiry is the backstop */
      }
      try {
        sessionStorage.removeItem(SESSION_KEY);
      } catch (_) {}
    });

    window.kosmosApp = api;
    // The deleted preload exposed the same object on both names; packages
    // built against the legacy `kepler` global keep working.
    window.kepler = api;
  }

  var match = /[#&]launch=([0-9a-fA-F-]+)&code=([0-9a-f]+)/.exec(location.hash);
  if (match) {
    var launchId = match[1];
    var code = match[2];
    // The fragment is spent state: strip it before it can leak into logs,
    // clipboard shares or a Referer on an embedded link.
    history.replaceState(null, "", location.pathname + location.search);
    fetch(arkUrl(launchId, "bootstrap"), {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ code: code }),
    })
      .then(function (r) {
        return r.json();
      })
      .then(function (resp) {
        if (resp && resp.ok && resp.data) {
          var state = {
            launch_id: resp.data.launch_id,
            id: resp.data.id,
            version: resp.data.version,
            broker_token: resp.data.broker_token,
            data_api: resp.data.data_api,
            expires_at: resp.data.expires_at,
          };
          persist(state);
          install(state);
        }
      })
      .catch(function () {
        /* bootstrap failed — the app sees no bridge and reports its own error */
      });
    return;
  }

  var session = restore();
  if (session) install(session);
})();
