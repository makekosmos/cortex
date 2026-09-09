"""Offline check of category selection and incomplete-version rejection."""
import json
from pathlib import Path
import tempfile
from unittest.mock import patch
import zipfile
import archive_health
from receive_hms import protect

with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    token = root / 'token.dpapi'
    token.write_bytes(protect(b'{"uid":"test-user","accessToken":"a","refreshToken":"r","accessTokenExpireTime":9999999999999}'))
    apk = root / 'test.apk'
    with zipfile.ZipFile(apk, 'w') as source:
        source.writestr('assets/dict_config.txt', json.dumps(dict(dictTypes=[
            dict(typeId=1, name='point', category=0),
            dict(typeId=2, name='empty', category=0),
            dict(typeId=3, name='sequence', category=1)])))
    response = json.dumps(dict(resultCode=0, versions=[
        dict(type=1, version=10), dict(type=2, version=0)])).encode()
    with patch.object(archive_health, 'read_page', return_value=response) as read:
        with patch.object(archive_health, 'run') as run:
            archive_health.main(token, apk, root / 'ok')
            assert len(read.call_args.args[2]['syncKeys']) == 2
            run.assert_called_once_with(token, root / 'ok' / '1', 'health/getHealthDataByVersion', 1, 10)
    with patch.object(archive_health, 'read_page', return_value=b'{"resultCode":0,"versions":[{"type":3,"version":20}]}'):
        with patch.object(archive_health, 'run') as run:
            archive_health.main(token, apk, root / 'sequence', category=1)
            run.assert_called_once_with(token, root / 'sequence' / '3', 'sequence/getSampleSequenceByVersion', 3, 20)
    with patch.object(archive_health, 'read_page', return_value=response) as read:
        with patch.object(archive_health, 'run') as run:
            archive_health.main(token, apk, root / 'statistics', category='statistics')
            assert all(key['category'] == 'SampleStatistic' for key in read.call_args.args[2]['syncKeys'])
            run.assert_called_once_with(token, root / 'statistics' / '1', 'health/getHealthStatisticsByVersion', 1, 10)
    with patch.object(archive_health, 'read_page', return_value=b'{"resultCode":0,"versions":[]}'):
        with patch.object(archive_health, 'run') as run:
            try:
                archive_health.main(token, apk, root / 'bad')
            except ValueError:
                pass
            else:
                raise AssertionError('Incomplete response accepted')
            run.assert_not_called()
            assert not (root / 'bad' / 'complete.json').exists()
print('PASS: point selection, empty stream skip, incomplete response rejection')
