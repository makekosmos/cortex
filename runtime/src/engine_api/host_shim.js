// Host bridge for a .kspkg app served by the Engine: delivers the launch
// credentials a browser page cannot otherwise hold, then exposes the
// window.kosmosApp / window.kepler API the removed package host provided.
(function () {
  "use strict";

  // Session survives in-tab reloads: the fragment is gone after the first
  // load, so the credentials ride in this tab's sessionStorage until the
  // tab closes — the same boundary Electron gave the old host.
  var SESSION_KEY = "mundus.launch";

  function parseHash() {
    var hash = location.hash || "";
    if (hash.charAt(0) === "#") {
      hash = hash.slice(1);
    }
    var out = {};
    hash.split("&").forEach(function (pair) {
      var at = pair.indexOf("=");
      if (at > 0) {
        out[pair.slice(0, at)] = pair.slice(at + 1);
      }
    });
    return out.launch && out.code ? out : null;
  }

  function persist(state) {
    try {
      sessionStorage.setItem(SESSION_KEY, JSON.stringify(state));
    } catch (e) {
      // Storage may be disabled — the page works until it reloads.
    }
  }

  function restore() {
    var raw;
    try {
      raw = sessionStorage.getItem(SESSION_KEY);
    } catch (e) {
      return null;
    }
    if (!raw) {
      return null;
    }
    var state = JSON.parse(raw);
    if (!state || !state.launch_id || !state.broker_token) {
      return null;
    }
    if (new Date(state.expires_at).getTime() <= Date.now()) {
      try {
        sessionStorage.removeItem(SESSION_KEY);
      } catch (e) {}
      return null;
    }
    return state;
  }

  // POST /v1/apps/launch/<id>/bootstrap — the one-time code rides in the
  // URL fragment (never sent over HTTP) and burns on first exchange.
  function bootstrap(launchId, code) {
    return fetch("/v1/apps/launch/" + launchId + "/bootstrap", {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({ code: code }),
    })
      .then(function (res) {
        return res.json();
      })
      .then(function (body) {
        if (!body || !body.ok || !body.data || !body.data.broker_token) {
          return null;
        }
        var state = {
          launch_id: body.data.launch_id,
          id: body.data.id,
          version: body.data.version,
          broker_token: body.data.broker_token,
          data_api: body.data.data_api,
          expires_at: body.data.expires_at,
        };
        persist(state);
        return state;
      });
  }

  function renew(session) {
    return fetch("/v1/apps/launch/" + session.launch_id + "/renew", {
      method: "POST",
      headers: { "X-Kosmos-Launch-Token": session.broker_token },
    })
      .then(function (res) {
        return res.ok ? res.json() : null;
      })
      .then(function (body) {
        if (body && body.ok && body.data && body.data.expires_at) {
          session.expires_at = body.data.expires_at;
          persist(session);
        }
        return session.expires_at;
      });
  }

  // Same cadence the deleted host used: renew ~30 s before expiry.
  function scheduleRenew(session) {
    renew(session)
      .catch(function () {
        return session.expires_at;
      })
      .then(function (expiresAt) {
        var ms = Math.max(new Date(expiresAt).getTime() - Date.now() - 30 * 1000, 5 * 1000);
        setTimeout(function () {
          scheduleRenew(session);
        }, ms);
      });
  }

  function startEventStream(session, handler, signal) {
    fetch("/v1/apps/launch/" + session.launch_id + "/events", {
      headers: { "X-Kosmos-Launch-Token": session.broker_token },
      signal: signal,
    })
      .then(function (res) {
        if (!res.ok || !res.body) {
          return;
        }
        var reader = res.body.getReader();
        var decoder = new TextDecoder();
        var buffer = "";
        function pump() {
          return reader.read().then(function (step) {
            if (step.done) {
              return;
            }
            buffer += decoder.decode(step.value, { stream: true });
            var at;
            while ((at = buffer.indexOf("\n\n")) >= 0) {
              var chunk = buffer.slice(0, at);
              buffer = buffer.slice(at + 2);
              chunk.split("\n").forEach(function (line) {
                if (line.indexOf("data:") === 0) {
                  handler(JSON.parse(line.slice(5)));
                }
              });
            }
            return pump();
          });
        }
        return pump();
      })
      .catch(function () {
        // Stream end or abort — unsubscribe needs no callback.
      });
  }

  // Install the API synchronously, the moment the shim parses — app
  // scripts that read window.kosmosApp at startup must find it. ark.*
  // waits on `ready`, which resolves once the credentials exist.
  function install(identity, ready) {
    var session = null;
    var wired = ready.then(function (next) {
      session = next;
      if (session) {
        // An immediate renew doubles as the release cancel: a pagehide from
        // the previous load marked this lease released, and only a renew
        // inside the grace window keeps it alive.
        scheduleRenew(session);
        addEventListener("pagehide", function (event) {
          // `persisted` = back/forward cache, the page keeps its session —
          // releasing there would kill a tab that is still usable.
          if (event.persisted) {
            return;
          }
          navigator.sendBeacon(
            "/v1/apps/launch/" + session.launch_id + "/release",
            JSON.stringify({ token: session.broker_token }),
          );
        });
      }
      return session;
    });

    function arkRequest(operation, params) {
      return wired.then(function () {
        if (!session) {
          return { ok: false, error: "launch session unavailable" };
        }
        return fetch(session.data_api, {
          method: "POST",
          headers: {
            "Content-Type": "application/json",
            "X-Kosmos-Launch-Token": session.broker_token,
          },
          body: JSON.stringify({ operation: operation, params: params || {} }),
        })
          .then(function (res) {
            return res.json();
          })
          .catch(function () {
            return { ok: false, error: "request failed" };
          });
      });
    }

    function arkSubscribe(handler) {
      var done = new AbortController();
      wired.then(function () {
        if (session && !done.signal.aborted) {
          startEventStream(session, handler, done.signal);
        }
      });
      return function () {
        done.abort();
      };
    }

    window.kosmosApp = {
      identity: identity,
      ark: { request: arkRequest, subscribe: arkSubscribe },
    };
    window.kepler = window.kosmosApp;
  }

  var hash = parseHash();
  if (hash) {
    // Strip the fragment before any app code or history entry can copy it.
    history.replaceState(null, "", location.pathname + location.search);
    install({ id: hash.pkg || null, version: hash.v || null }, bootstrap(hash.launch, hash.code));
  } else {
    var stored = restore();
    if (stored) {
      install({ id: stored.id, version: stored.version }, Promise.resolve(stored));
    }
    // No fragment, no stored session: this page was never launched through
    // packages.open — install nothing rather than a dead API.
  }
})();
