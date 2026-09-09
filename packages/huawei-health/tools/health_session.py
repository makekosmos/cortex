"""DPAPI token rotation for the verified personal EU-data/RU-login session."""
import json
import errno
import msvcrt
import os
from pathlib import Path
import tempfile
import time
import urllib.request
from receive_hms import protect


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError('Unexpected API redirect')


def load_session(filename, force=False):
    filename = Path(filename)
    # Windows releases the lock on process exit; concurrent refreshes must serialize.
    with filename.with_suffix('.lock').open('a+b') as lock:
        if lock.tell() == 0:
            lock.write(b'0')
            lock.flush()
        lock.seek(0)
        deadline = time.monotonic() + 45
        while True:
            try:
                msvcrt.locking(lock.fileno(), msvcrt.LK_NBLCK, 1)
                break
            except OSError as error:
                if error.errno not in (errno.EACCES, errno.EAGAIN, errno.EDEADLK) or time.monotonic() >= deadline:
                    raise
                time.sleep(0.1)
        try:
            token = json.loads(protect(filename.read_bytes(), decrypt=True))
            if not all(isinstance(token.get(k), str) and token[k] for k in ('uid', 'accessToken', 'refreshToken')):
                raise ValueError('Invalid stored session')
            now = int(time.time() * 1000)
            if not force and int(token['accessTokenExpireTime']) > now + 60000:
                return token
            request = urllib.request.Request(
                'https://healthcommon-drru.things.dbankcloud.ru/commonAbility/userAccessToken/refresh',
                data=json.dumps(dict(refreshToken=token['refreshToken'], appId='10414141')).encode(),
                headers={'Content-Type': 'application/json', 'x-huid': token['uid'],
                         'x-ts': str(now), 'x-version': 'and_health_16.1.6.320'})
            with urllib.request.build_opener(NoRedirect()).open(request, timeout=30) as response:
                raw = response.read()
            updated = json.loads(raw)
            if (updated.get('resultCode') != 0 or updated.get('uid') != token['uid']
                    or not all(isinstance(updated.get(k), str) and updated[k] for k in ('accessToken', 'refreshToken'))
                    or int(updated.get('accessTokenExpireTime', 0)) <= now):
                raise ValueError('Session refresh failed; stored credentials preserved')
            with tempfile.NamedTemporaryFile(dir=filename.parent, prefix='session-refresh-', suffix='.dpapi', delete=False) as output:
                temporary = Path(output.name)
                output.write(protect(raw))
                output.flush()
                os.fsync(output.fileno())
            try:
                os.replace(temporary, filename)
            except OSError:
                raise OSError(f'Refreshed session preserved in {temporary}; restore it before retrying') from None
            return updated
        finally:
            lock.seek(0)
            msvcrt.locking(lock.fileno(), msvcrt.LK_UNLCK, 1)
