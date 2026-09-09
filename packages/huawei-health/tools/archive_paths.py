"""Read the proven EU motion-path endpoint into a DPAPI-encrypted page archive."""
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import urllib.request
from receive_hms import protect
from health_session import load_session, NoRedirect


def read_page(token, path, parameters):
    if path not in ('path/getMotionPathByVersion', 'sport/getSportsDataByVersion', 'health/getHealthDataByVersion',
                    'common/getSyncVersions', 'sequence/getSampleSequenceByVersion',
                    'health/getHealthStatisticsByVersion', 'report/getPersonalReport'):
        raise ValueError('Unverified read endpoint')
    body = dict(ts=int(time.time()*1000), tokenType=2, token=token['accessToken'],
                source=1, appId='com.huawei.health', deviceId='clientnull',
                deviceType='0', upDeviceType='0', siteId='7',
                sysVersion='Windows', language='en', isManually=1)
    if set(parameters) - {'version', 'type', 'dataType', 'syncKeys', 'deviceCode', 'dataSource',
                          'bestItems', 'accumulatedItems'}:
        raise ValueError('Unexpected request parameter')
    body.update(parameters)
    request = urllib.request.Request(
        'https://sportdata-dre.things.dbankcloud.com/dataQuery/' + path,
        data=json.dumps(body).encode(), headers={
            'Content-Type': 'application/json;charset=UTF-8',
            'x-huid': str(token['uid']), 'x-version': 'and_health_16.1.6.320'})
    opener = urllib.request.build_opener(NoRedirect())
    for delay in (5, 15, 30, None):
        with opener.open(request, timeout=30) as response:
            raw = response.read()
        if json.loads(raw).get('resultCode') != 1102 or delay is None:
            return raw
        print(json.dumps(dict(retry='service_busy', delay=delay)), flush=True)
        time.sleep(delay)


def page_records(page):
    keys = [key for key in ('data', 'statisticTotal', 'detailInfos') if key in page]
    if len(keys) > 1:
        raise ValueError('Ambiguous page records')
    if keys and keys[0] != 'detailInfos':
        data = page[keys[0]]
        if not isinstance(data, dict) or any(not isinstance(v, list) for v in data.values()):
            raise ValueError('Invalid grouped page')
        return [record for group in data.values() for record in group]
    records = page.get('detailInfos', [])
    if not isinstance(records, list):
        raise ValueError('Invalid page records')
    return records


def next_version(page, previous, expected=0):
    if page.get('resultCode') != 0:
        raise ValueError('Huawei API error: ' + str(page.get('resultCode')))
    records = page_records(page)
    deleted = page.get('deleteInfos', [])
    if not isinstance(records, list) or not isinstance(deleted, list):
        raise ValueError('Invalid page lists')
    version = page.get('currentVersion')
    if not records and not deleted and type(version) is int and version > previous:
        return version
    if not records and not deleted and (version is None or type(version) is int and (version == 0 or version >= previous)):
        if previous < expected:
            raise ValueError('Empty page before advertised stream version')
        return None
    if type(version) is not int or version < previous:
        raise ValueError('Invalid cursor')
    if not records and not deleted:
        return None
    if version <= previous:
        raise ValueError('Stalled cursor with records')
    return version


def run(token_file, destination, path='path/getMotionPathByVersion', health_type=None, expected=0, since=None,
        expected_account=None):
    token = load_session(token_file)
    account = token['uid']
    if expected_account is not None and account != expected_account:
        raise ValueError('Account changed before stream')
    version = 0
    if since is not None:
        checkpoint = json.loads(protect((Path(since) / 'checkpoint.dpapi').read_bytes(), decrypt=True))
        if (checkpoint['uid'], checkpoint['path'], checkpoint['type']) != (token['uid'], path, health_type):
            raise ValueError('Checkpoint belongs to a different account or stream')
        version = checkpoint['cursor']
        if type(version) is not int or version < 0:
            raise ValueError('Invalid checkpoint cursor')
        target = checkpoint.get('expected', version)
        if type(target) is not int or target < 0:
            raise ValueError('Invalid checkpoint target')
        expected = max(expected, target)
    destination = Path(destination)
    destination.mkdir(exist_ok=False)
    def checkpoint():
        state = dict(uid=account, path=path, type=health_type, cursor=version, expected=expected,
                     parent=str(Path(since).resolve()) if since else None)
        with tempfile.NamedTemporaryFile(dir=destination, delete=False) as output:
            temporary = Path(output.name)
            output.write(protect(json.dumps(state).encode()))
            output.flush()
            os.fsync(output.fileno())
        try:
            os.replace(temporary, destination / 'checkpoint.dpapi')
        finally:
            temporary.unlink(missing_ok=True)
    total = 0
    for number in range(1000):
        token = load_session(token_file)
        if token['uid'] != account:
            raise ValueError('Account changed during archive')
        parameters = dict(version=version, dataType=2)
        if health_type is not None:
            parameters['type'] = health_type
        if path == 'sequence/getSampleSequenceByVersion':
            parameters['deviceCode'] = 0
        if path == 'health/getHealthStatisticsByVersion':
            parameters['dataSource'] = 2
        raw = read_page(token, path, parameters)
        # Store the exact response before advancing; preserve integer precision.
        with (destination / f'{number:05d}.dpapi').open('xb') as output:
            output.write(protect(raw))
            output.flush()
            os.fsync(output.fileno())
        page = json.loads(raw)
        following = next_version(page, version, expected)
        count = len(page_records(page))
        total += count
        print(json.dumps(dict(page=number, records=count,
                              bytes=len(raw), total=total)), flush=True)
        if following is None:
            checkpoint()
            (destination / 'complete.json').write_text(
                json.dumps(dict(pages=number+1, records=total, cursor=version)), encoding='utf-8')
            return
        version = following
        checkpoint()
    raise RuntimeError('Page limit reached; archive incomplete')


def check():
    try:
        read_page({}, 'delete', {})
    except ValueError:
        pass
    else:
        raise AssertionError('Write endpoint accepted')
    assert next_version(dict(resultCode=0, currentVersion=2, detailInfos=[{}]), 1) == 2
    assert page_records(dict(data={'20200101': [{}, {}]})) == [{}, {}]
    assert page_records(dict(statisticTotal={'400012': [{}, {}]})) == [{}, {}]
    assert next_version(dict(resultCode=0, currentVersion=2, statisticTotal={'400012': [{}]}), 1) == 2
    assert next_version(dict(resultCode=0, currentVersion=2, data={'20200101': [{}]}), 1) == 2
    assert next_version(dict(resultCode=0, currentVersion=2), 2) is None
    assert next_version(dict(resultCode=0, currentVersion=0), 2) is None
    assert next_version(dict(resultCode=0), 2) is None
    assert next_version(dict(resultCode=0, currentVersion=10), 0, 10) == 10
    try:
        next_version(dict(resultCode=0), 0, 10)
    except ValueError:
        pass
    else:
        raise AssertionError('Premature terminal page accepted')
    for page in [dict(resultCode=1), dict(resultCode=0, currentVersion=1, detailInfos=[{}]),
                 dict(resultCode=0, currentVersion=0, detailInfos=[{}]), dict(resultCode=0, currentVersion=True),
                 dict(resultCode=0, currentVersion=1, data={'20200101': [{}]}),
                 dict(resultCode=0, currentVersion=2, data={'bad': 'not a list'}),
                 dict(resultCode=0, currentVersion=2, statisticTotal={'bad': {}}),
                 dict(resultCode=0, currentVersion=2, data={}, statisticTotal={})]:
        try:
            next_version(page, 1)
        except ValueError:
            continue
        raise AssertionError('Invalid page accepted')
    print('PASS: cursor and error checks')


if __name__ == '__main__':
    if sys.argv[1:] == ['--self-test']:
        check()
    elif len(sys.argv) == 3:
        run(*sys.argv[1:])
    elif len(sys.argv) == 5 and sys.argv[3] == '--since':
        run(sys.argv[1], sys.argv[2], since=sys.argv[4])
    else:
        raise SystemExit('Usage: archive_paths.py TOKEN.dpapi NEW_DIRECTORY [--since ARCHIVE] | --self-test')
