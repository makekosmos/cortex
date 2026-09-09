"""Offline check of incremental reads and account isolation."""
import json
from pathlib import Path
import tempfile
from unittest.mock import patch
import archive_paths as archive
from receive_hms import protect

with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    token = root / 'token.dpapi'
    token.write_bytes(protect(b'{"uid":"test-user","accessToken":"a","refreshToken":"r","accessTokenExpireTime":9999999999999}'))
    first = root / 'first'
    with patch.object(archive, 'read_page', side_effect=[
        b'{"resultCode":0,"currentVersion":10,"detailInfos":[{"id":1}]}',
        b'{"resultCode":0}']):
        archive.run(token, first, expected=10)
    second = root / 'second'
    with patch.object(archive, 'read_page', return_value=b'{"resultCode":0}') as read:
        archive.run(token, second, expected=10, since=first)
        assert read.call_args.args[2]['version'] == 10
        assert json.loads((second / 'complete.json').read_text())['records'] == 0
    with patch.object(archive, 'read_page', side_effect=[
        b'{"resultCode":0,"currentVersion":11,"detailInfos":[{"id":2}]}',
        b'{"resultCode":0}']):
        archive.run(token, root / 'new', expected=11, since=second)
        summary = json.loads((root / 'new' / 'complete.json').read_text())
        assert summary['records'] == 1 and summary['cursor'] == 11
    with patch.object(archive, 'read_page', side_effect=[
        b'{"resultCode":0,"currentVersion":12,"detailInfos":[{"id":3}]}',
        b'{"resultCode":1102}']):
        try:
            archive.run(token, root / 'interrupted', expected=13, since=root / 'new')
        except ValueError:
            pass
        else:
            raise AssertionError('Busy server accepted')
    assert not (root / 'interrupted' / 'complete.json').exists()
    with patch.object(archive, 'read_page', return_value=b'{"resultCode":0}'):
        try:
            archive.run(token, root / 'premature', since=root / 'interrupted')
        except ValueError:
            pass
        else:
            raise AssertionError('Resume lost advertised target')
    assert not (root / 'premature' / 'complete.json').exists()
    with patch.object(archive, 'read_page', side_effect=[
        b'{"resultCode":0,"currentVersion":13,"detailInfos":[{"id":4}]}',
        b'{"resultCode":0}']) as read:
        archive.run(token, root / 'resumed', since=root / 'interrupted')
        assert read.call_args_list[0].args[2]['version'] == 12
        assert json.loads((root / 'resumed' / 'complete.json').read_text())['records'] == 1
    with patch.object(archive, 'read_page') as read:
        for kwargs in [dict(health_type=123), dict(path='health/getHealthDataByVersion')]:
            try:
                archive.run(token, root / 'wrong-stream', since=first, **kwargs)
            except ValueError:
                pass
            else:
                raise AssertionError('Wrong stream accepted')
        token.write_bytes(protect(b'{"uid":"different-user","accessToken":"a","refreshToken":"r","accessTokenExpireTime":9999999999999}'))
        try:
            archive.run(token, root / 'wrong-user', since=first)
        except ValueError:
            pass
        else:
            raise AssertionError('Wrong account accepted')
        read.assert_not_called()
print('PASS: incremental cursor reuse and account/stream isolation')
