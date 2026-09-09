"""Bounded retries only for the observed read-only service-busy response."""
from unittest.mock import MagicMock, patch
import archive_paths as archive

token = dict(uid='test', accessToken='test')
for replies, delays in [
    ([b'{"resultCode":1102}', b'{"resultCode":0}'], [5]),
    ([b'{"resultCode":1102}'] * 4, [5, 15, 30]),
    ([b'{"resultCode":1001}'], []),
]:
    opener = MagicMock()
    opener.open.return_value.__enter__.return_value.read.side_effect = replies
    with patch.object(archive.urllib.request, 'build_opener', return_value=opener), \
            patch.object(archive.time, 'sleep') as sleep:
        result = archive.read_page(token, 'health/getHealthDataByVersion', {'version': 10})
    assert result == replies[-1]
    assert opener.open.call_count == len(replies)
    assert [call.args[0] for call in sleep.call_args_list] == delays
print('PASS: busy responses retry with bounded delays; other errors return unchanged')
