"""Temporary, current-user callback receiver for the APK's fixed hms redirect.

Run --install before browser authorization; --remove after the experiment.
The callback is encrypted with Windows DPAPI, never printed.
"""
import ctypes
from ctypes import wintypes
import json
import os
import pathlib
import secrets
import subprocess
import sys
import time
import urllib.parse
import urllib.request
import winreg

ROOT = pathlib.Path(__file__).resolve().parent / "login-probe"
KEY = r"Software\Classes\hms"
COMMAND = subprocess.list2cmdline([sys.executable.replace("python.exe", "pythonw.exe"), str(pathlib.Path(__file__).resolve())]) + ' "%1"'


def validate(url, state, now):
    u = urllib.parse.urlsplit(url)
    if u.scheme != "hms" or u.netloc != "redirect_url" or u.path or u.fragment:
        raise ValueError("unexpected callback")
    q = urllib.parse.parse_qs(u.query, keep_blank_values=True, strict_parsing=True)
    if any(len(v) != 1 for v in q.values()):
        raise ValueError("duplicate parameter")
    if now > state["expires"] or not secrets.compare_digest(q.get("state", [""])[0], state["state"]):
        raise ValueError("invalid or expired state")
    if not q.get("code", [""])[0] or "error" in q:
        raise ValueError("no authorization code")
    return {k: v[0] for k, v in q.items()}


def protect(data, decrypt=False):
    class Blob(ctypes.Structure):
        _fields_ = [("size", wintypes.DWORD), ("data", ctypes.POINTER(ctypes.c_ubyte))]
    buf = ctypes.create_string_buffer(data)
    source = Blob(len(data), ctypes.cast(buf, ctypes.POINTER(ctypes.c_ubyte)))
    target = Blob()
    api = ctypes.WinDLL("crypt32", use_last_error=True)
    operation = api.CryptUnprotectData if decrypt else api.CryptProtectData
    operation.argtypes = [ctypes.POINTER(Blob), ctypes.c_void_p if decrypt else wintypes.LPCWSTR, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, wintypes.DWORD, ctypes.POINTER(Blob)]
    operation.restype = wintypes.BOOL
    if not operation(ctypes.byref(source), None if decrypt else "Huawei login probe", None, None, None, 1, ctypes.byref(target)):
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        return ctypes.string_at(target.data, target.size)
    finally:
        kernel = ctypes.WinDLL("kernel32")
        kernel.LocalFree.argtypes = [ctypes.c_void_p]
        kernel.LocalFree.restype = ctypes.c_void_p
        kernel.LocalFree(target.data)


def install():
    for hive in (winreg.HKEY_CURRENT_USER, winreg.HKEY_LOCAL_MACHINE):
        try:
            with winreg.OpenKey(hive, KEY):
                raise RuntimeError("Existing hms registration; left unchanged")
        except FileNotFoundError:
            pass
    ROOT.mkdir(exist_ok=True)
    if (ROOT / "callback.dpapi").exists():
        raise RuntimeError("Previous callback exists; left unchanged")
    state = {"state": secrets.token_urlsafe(32), "nonce": secrets.token_urlsafe(32), "expires": time.time() + 1200}
    (ROOT / "challenge.json").write_text(json.dumps(state), encoding="utf-8")
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, KEY) as key:
        winreg.SetValueEx(key, "", 0, winreg.REG_SZ, "URL:Huawei personal archive probe")
        winreg.SetValueEx(key, "URL Protocol", 0, winreg.REG_SZ, "")
    with winreg.CreateKey(winreg.HKEY_CURRENT_USER, KEY + r"\shell\open\command") as key:
        winreg.SetValueEx(key, "", 0, winreg.REG_SZ, COMMAND)
    query = dict(access_type="offline", response_type="code", client_id="10414141", redirect_uri="hms://redirect_url", scope="openid https://www.huawei.com/auth/account/base.profile", display="touch", ui_locales="en-us", state=state["state"], nonce=state["nonce"])
    print("https://oauth-login.cloud.huawei.com/oauth2/v3/authorize?" + urllib.parse.urlencode(query))


def remove():
    with winreg.OpenKey(winreg.HKEY_CURRENT_USER, KEY + r"\shell\open\command") as key:
        if winreg.QueryValueEx(key, "")[0] != COMMAND:
            raise RuntimeError("Registration changed; left unchanged")
    for suffix in (r"\shell\open\command", r"\shell\open", r"\shell", ""):
        winreg.DeleteKey(winreg.HKEY_CURRENT_USER, KEY + suffix)


def receive(url):
    from health_session import NoRedirect
    state = json.loads((ROOT / "challenge.json").read_text(encoding="utf-8"))
    callback = validate(url, state, time.time())
    if (ROOT / 'session.dpapi').exists():
        raise ValueError('Existing session left unchanged')
    # Exclusive capture prevents a replay from exchanging the same code twice.
    with (ROOT / 'callback.dpapi').open('xb') as output:
        output.write(protect(json.dumps(callback).encode()))
        output.flush()
        os.fsync(output.fileno())
    request = urllib.request.Request(
        'https://healthsession-drru.things.dbankcloud.ru/commonAbility/userAccessToken/obtain',
        data=json.dumps(dict(authorizationCode=callback['code'], appId='10414141')).encode(),
        headers={'Content-Type': 'application/json', 'x-ts': str(int(time.time()*1000)),
                 'x-version': 'and_health_16.1.6.320'})
    with urllib.request.build_opener(NoRedirect()).open(request, timeout=30) as response:
        raw = response.read()
    # Keep the encrypted server reply even if validation or final publication fails.
    encrypted = protect(raw)
    with (ROOT / 'exchange-response.dpapi').open('xb') as output:
        output.write(encrypted)
        output.flush()
        os.fsync(output.fileno())
    token = json.loads(raw)
    if (token.get('resultCode') != 0
            or not all(isinstance(token.get(k), str) and token[k] for k in ('uid', 'accessToken', 'refreshToken'))
            or int(token.get('accessTokenExpireTime', 0)) <= int(time.time()*1000)):
        raise ValueError('Authorization exchange failed')
    with (ROOT / 'session.dpapi').open('xb') as output:
        output.write(encrypted)
        output.flush()
        os.fsync(output.fileno())
    (ROOT / 'status.txt').write_text('session-ready', encoding='utf-8')


def check():
    state = {"state": "test-state", "expires": 100}
    good = "hms://redirect_url?state=test-state&code=test-code"
    assert validate(good, state, 99)["code"] == "test-code"
    for url, now in [(good, 101), (good + "&code=second", 99), (good.replace("test-state", "wrong"), 99), (good.replace("redirect_url", "other"), 99), (good + "#fragment", 99), (good + "&error=1201", 99)]:
        try:
            validate(url, state, now)
        except ValueError:
            continue
        raise AssertionError("accepted invalid callback")
    assert protect(protect(b"test-code"), decrypt=True) == b"test-code"
    print("PASS: callback validation and DPAPI encryption")


if __name__ == "__main__":
    if len(sys.argv) != 2:
        raise SystemExit(2)
    if sys.argv[1] == "--self-test":
        check()
    elif sys.argv[1] == "--install":
        install()
    elif sys.argv[1] == "--remove":
        remove()
    else:
        try:
            receive(sys.argv[1])
        except Exception:
            # Never write the URL, credentials, or exception text to a log.
            if ROOT.exists():
                (ROOT / "status.txt").write_text("rejected", encoding="utf-8")
            raise SystemExit(1)
