"""Exercise a failed account batch, continuation and account isolation without HTTP."""
import json
from pathlib import Path
import tempfile
from unittest.mock import patch
import zipfile
import archive_account as account
import archive_paths
from receive_hms import protect

with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    token = root / 'session.dpapi'
    token.write_bytes(protect(b'{"uid":"test","accessToken":"a","refreshToken":"r","accessTokenExpireTime":9999999999999}'))
    apk = root / 'test.apk'
    with zipfile.ZipFile(apk, 'w') as z:
        z.writestr('assets/dict_config.txt', '{"dictTypes":[{"typeId":10006,"category":0}]}')
    fail_report = True
    cursors = []
    def read(session, path, params):
        if path == 'common/getSyncVersions':
            return json.dumps(dict(resultCode=0, versions=[
                dict(type=k['type'], version=10 if k['type'] == 7 else 0)
                for k in params['syncKeys']])).encode()
        if path == 'report/getPersonalReport':
            return json.dumps(dict(resultCode=1102 if fail_report else 0)).encode()
        assert path == 'health/getHealthDataByVersion' and params['type'] == 7
        cursors.append(params['version'])
        return (b'{"resultCode":0,"currentVersion":10,"detailInfos":[{}]}'
                if params['version'] == 0 else b'{"resultCode":0}')
    with patch.object(account, 'read_page', side_effect=read), patch.object(archive_paths, 'read_page', side_effect=read):
        try:
            account.main(token, apk, root / 'failed')
        except ValueError as error:
            assert str(error) == 'Personal report failed'
        else:
            raise AssertionError('Failed report marked complete')
        assert not (root / 'failed/complete.json').exists()
        assert cursors == [0, 10]
        fail_report = False
        account.main(token, apk, root / 'resumed', since=root / 'failed')
        assert cursors == [0, 10, 10]
        assert (root / 'resumed/complete.json').exists()
        # An interrupted batch without a stream checkpoint must inherit its parent.
        gap = root / 'gap'
        gap.mkdir()
        account.save(gap / 'account.dpapi', dict(uid='test', parent=str(root / 'resumed')))
        account.main(token, apk, root / 'after-gap', since=gap)
        assert cursors == [0, 10, 10, 10]
    def zero_versions(session, path, params):
        assert path == 'common/getSyncVersions'
        return json.dumps(dict(resultCode=0, versions=[
            dict(type=k['type'], version=0) for k in params['syncKeys']])).encode()
    checkpoint_path = root / 'resumed/legacy-7/checkpoint.dpapi'
    checkpoint = json.loads(protect(checkpoint_path.read_bytes(), decrypt=True))
    checkpoint['expected'] = 20
    checkpoint_path.write_bytes(protect(json.dumps(checkpoint).encode()))
    with patch.object(account, 'read_page', side_effect=zero_versions):
        try:
            account.main(token, apk, root / 'regressed', since=root / 'resumed')
        except ValueError as error:
            assert str(error) == 'Stream version regressed to zero'
        else:
            raise AssertionError('Unfinished stream skipped on zero version')
        assert not (root / 'regressed/complete.json').exists()
    with patch.object(account, 'read_page', side_effect=read), \
            patch.object(archive_paths, 'load_session', return_value={'uid': 'other'}), \
            patch.object(archive_paths, 'read_page') as network:
        try:
            account.main(token, apk, root / 'switched')
        except ValueError as error:
            assert str(error) == 'Account changed before stream'
        else:
            raise AssertionError('Account switched between group and stream')
        network.assert_not_called()
        assert not (root / 'switched/complete.json').exists()
    token.write_bytes(protect(b'{"uid":"other","accessToken":"a","refreshToken":"r","accessTokenExpireTime":9999999999999}'))
    with patch.object(account, 'read_page') as network:
        try:
            account.main(token, apk, root / 'wrong', since=root / 'resumed')
        except ValueError:
            pass
        else:
            raise AssertionError('Other account accepted')
        network.assert_not_called()
        assert not (root / 'wrong').exists()
print('PASS: account continuation skips saved history and rejects another account')
