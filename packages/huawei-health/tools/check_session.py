"""Offline token rotation check; does not contact Huawei."""
import json
import errno
from pathlib import Path
import tempfile
from unittest.mock import MagicMock, patch
from receive_hms import protect
from health_session import load_session

with tempfile.TemporaryDirectory() as temp:
    path = Path(temp) / 'session.dpapi'
    old = dict(uid='test-user', accessToken='old', refreshToken='old-refresh', accessTokenExpireTime=1)
    path.write_bytes(protect(json.dumps(old).encode()))
    updated = dict(resultCode=0, uid='test-user', accessToken='new', refreshToken='new-refresh', accessTokenExpireTime=9999999999999)
    opener = MagicMock()
    opener.open.return_value.__enter__.return_value.read.return_value = json.dumps(updated).encode()
    with patch('health_session.urllib.request.build_opener', return_value=opener):
        assert load_session(path) == updated
        assert json.loads(protect(path.read_bytes(), decrypt=True)) == updated
    with patch('health_session.urllib.request.build_opener') as build:
        assert load_session(path) == updated
        build.assert_not_called()
    original = path.read_bytes()
    for bad in [dict(resultCode=1), dict(updated, uid='other-user'), dict(updated, accessToken='')]:
        opener.open.return_value.__enter__.return_value.read.return_value = json.dumps(bad).encode()
        with patch('health_session.urllib.request.build_opener', return_value=opener):
            try:
                load_session(path, force=True)
            except ValueError:
                pass
            else:
                raise AssertionError('Invalid refresh accepted')
        assert path.read_bytes() == original
    with patch('health_session.msvcrt.locking', side_effect=[OSError(errno.EACCES, 'busy'), None, None]) as locking:
        with patch('health_session.time.sleep'):
            assert load_session(path) == updated
            assert locking.call_count == 3
    opener.open.return_value.__enter__.return_value.read.return_value = json.dumps(updated).encode()
    with patch('health_session.urllib.request.build_opener', return_value=opener):
        with patch('health_session.os.replace', side_effect=OSError('disk busy')):
            try:
                load_session(path, force=True)
            except OSError:
                pass
            else:
                raise AssertionError('Replacement failure hidden')
    recovery = list(path.parent.glob('session-refresh-*.dpapi'))
    assert len(recovery) == 1 and json.loads(protect(recovery[0].read_bytes(), decrypt=True)) == updated
    assert path.read_bytes() == original
print('PASS: refresh, encrypted rotation, cached session, failure/account isolation')
