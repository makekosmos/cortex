"""Offline callback-to-session check; no browser, registry or network access."""
import json
from pathlib import Path
import tempfile
import time
import urllib.parse
from unittest.mock import MagicMock, patch
import receive_hms

with tempfile.TemporaryDirectory() as temp:
    root = Path(temp)
    token = dict(resultCode=0, uid='test-user', accessToken='access', refreshToken='refresh', accessTokenExpireTime=9999999999999)
    opener = MagicMock()
    opener.open.return_value.__enter__.return_value.read.return_value = json.dumps(token).encode()
    url = 'hms://redirect_url?' + urllib.parse.urlencode(dict(state='expected', code='test+code/='))
    with patch.object(receive_hms, 'ROOT', root):
        (root / 'challenge.json').write_text(json.dumps(dict(state='expected', expires=time.time()+60)))
        with patch('urllib.request.build_opener', return_value=opener):
            try:
                receive_hms.receive(url.replace('expected', 'wrong'))
            except ValueError:
                pass
            else:
                raise AssertionError('Unbound callback accepted')
            opener.open.assert_not_called()
            receive_hms.receive(url)
            assert json.loads(opener.open.call_args.args[0].data)['authorizationCode'] == 'test+code/='
            assert json.loads(receive_hms.protect((root/'session.dpapi').read_bytes(), decrypt=True)) == token
            assert (root/'status.txt').read_text() == 'session-ready'
            try:
                receive_hms.receive(url)
            except ValueError:
                pass
            else:
                raise AssertionError('Replay overwrote session')
            assert opener.open.call_count == 1
    failed = root / 'failed'
    failed.mkdir()
    (failed / 'challenge.json').write_text((root / 'challenge.json').read_text())
    opener.open.return_value.__enter__.return_value.read.return_value = b'{"resultCode":1}'
    with patch.object(receive_hms, 'ROOT', failed), patch('urllib.request.build_opener', return_value=opener):
        try:
            receive_hms.receive(url)
        except ValueError:
            pass
        else:
            raise AssertionError('Failed exchange accepted')
        assert not (failed / 'session.dpapi').exists()
        assert (failed / 'exchange-response.dpapi').exists()
print('PASS: state binding, code decoding, exchange, DPAPI session, replay/failure rejection')
