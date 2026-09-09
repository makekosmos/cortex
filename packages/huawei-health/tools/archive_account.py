"""Archive verified Huawei streams; each run uses a new directory, optionally --since a prior run."""
import argparse
import json
import os
from pathlib import Path
import zipfile
from archive_paths import read_page, run
from health_session import load_session
from receive_hms import protect


def save(path, value):
    with path.open('xb') as output:
        output.write(protect(value if isinstance(value, bytes) else json.dumps(value).encode()))
        output.flush()
        os.fsync(output.fileno())


def previous_stream(root, key, account):
    seen = set()
    while root is not None:
        root = Path(root).resolve()
        if root in seen:
            raise ValueError('Archive parent cycle')
        seen.add(root)
        state = json.loads(protect((root / 'account.dpapi').read_bytes(), decrypt=True))
        if state['uid'] != account:
            raise ValueError('Archive belongs to another account')
        stream = root / key
        if (stream / 'checkpoint.dpapi').is_file():
            return stream
        root = state['parent']
    return None


def main(token_file, apk, destination, since=None):
    token = load_session(token_file)
    account = token['uid']
    # Validate the parent before publishing or making data requests.
    if since is not None:
        previous_stream(since, '_validate', account)
    with zipfile.ZipFile(apk) as source:
        dictionary = json.loads(source.read('assets/dict_config.txt'))['dictTypes']
    groups = [
        ('legacy', [1, 2, 4, 7, 9, 11, 12, 13, 14, 15, 16, 18, 19, 21, 34001, 900000000]),
        ('point', [x['typeId'] for x in dictionary if x.get('category') == 0]),
        ('sequence', [x['typeId'] for x in dictionary if x.get('category') == 1]),
        ('statistics', [x['typeId'] for x in dictionary if x.get('category') == 0
                        and x['typeId'] not in (200005, 300002)]),
    ]
    root = Path(destination)
    root.mkdir(exist_ok=False)
    save(root / 'account.dpapi', dict(uid=account, parent=str(Path(since).resolve()) if since else None))
    completed = []
    for group, kinds in groups:
        keys = [dict(type=kind, dataType=2) for kind in kinds]
        if group == 'statistics':
            for key in keys:
                key['category'] = 'SampleStatistic'
        token = load_session(token_file)
        if token['uid'] != account:
            raise ValueError('Account changed during archive')
        raw = read_page(token, 'common/getSyncVersions', dict(syncKeys=keys))
        response = json.loads(raw)
        save(root / f'{group}-versions.dpapi', raw)
        versions = response.get('versions', [])
        if (response.get('resultCode') != 0 or len(versions) != len(set(kinds))
                or {x['type'] for x in versions} != set(kinds)
                or any(type(x.get('version')) is not int or x['version'] < 0 for x in versions)):
            raise ValueError('Incomplete or invalid version response')
        for item in versions:
            kind = item['type']
            key = f'{group}-{kind}'
            prior = previous_stream(since, key, account) if since else None
            if item['version'] == 0:
                if prior is not None:
                    checkpoint = json.loads(protect((prior / 'checkpoint.dpapi').read_bytes(), decrypt=True))
                    if max(checkpoint['cursor'], checkpoint.get('expected', 0)) > 0:
                        raise ValueError('Stream version regressed to zero')
                continue
            path = 'health/getHealthDataByVersion'
            request_type = kind
            if group == 'legacy' and kind in (1, 2):
                path = 'sport/getSportsDataByVersion' if kind == 1 else 'path/getMotionPathByVersion'
                request_type = None
            elif group == 'sequence':
                path = 'sequence/getSampleSequenceByVersion'
            elif group == 'statistics':
                path = 'health/getHealthStatisticsByVersion'
            print(json.dumps(dict(stream=key)), flush=True)
            run(token_file, root / key, path, request_type, item['version'], since=prior,
                expected_account=account)
            completed.append(key)
    token = load_session(token_file)
    if token['uid'] != account:
        raise ValueError('Account changed during archive')
    report = read_page(token, 'report/getPersonalReport', dict(
        bestItems=['bestRopeSkippingSingleCount', 'bestRopeSkippingContinuousCount',
                   'bestRopeSkippingMaxSpeed1MIN', 'bestRopeSkippingEnduranceAbility',
                   'bestRopeSkippingEnduranceTimeAbility'],
        accumulatedItems=['accumPerfectGoalAchievedDays']))
    save(root / 'personal-report.dpapi', report)
    if json.loads(report).get('resultCode') != 0:
        raise ValueError('Personal report failed')
    (root / 'complete.json').write_text(json.dumps(dict(
        scope='Verified legacy, APK point/sequence/statistics and selected personal report fields',
        streams=completed)), encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('token')
    parser.add_argument('apk')
    parser.add_argument('destination')
    parser.add_argument('--since')
    args = parser.parse_args()
    main(args.token, args.apk, args.destination, args.since)
