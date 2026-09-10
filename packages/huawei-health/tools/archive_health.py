"""Archive point-data categories defined by the inspected APK dictionary."""
import argparse
import json
from pathlib import Path
import zipfile
from archive_paths import read_page, run
from receive_hms import protect
from health_session import load_session


def main(token_file, apk, destination, category=0):
    if category not in (0, 1, 'statistics'):
        raise ValueError('Unsupported dictionary category')
    root = Path(destination)
    root.mkdir(exist_ok=False)
    with zipfile.ZipFile(apk) as source:
        dictionary = json.loads(source.read('assets/dict_config.txt'))['dictTypes']
        extra = (json.loads(source.read('assets/dict_config.json'))['dictTypes']
                 if 'assets/dict_config.json' in source.namelist() else [])
    types = {entry['typeId']: entry for entry in dictionary
             if entry.get('category') == (0 if category == 'statistics' else category)}
    if category == 0:
        for entry in extra:
            if entry['typeId'] in (500021, 500023, 500024, 500026):
                types[entry['typeId']] = entry
    elif category == 'statistics' and extra:
        types[800003] = {'typeId': 800003, 'name': 'SLEEP_PRO_RECORD'}
    if category == 'statistics':
        # APK excludes active hours and sport-goal achievements from this API.
        types = {kind: entry for kind, entry in types.items() if kind not in (200005, 300002)}
    keys = [dict(type=i, dataType=2) for i in types]
    if category == 'statistics':
        for key in keys:
            key['category'] = 'SampleStatistic'
    token = load_session(token_file)
    raw = read_page(token, 'common/getSyncVersions',
                    dict(syncKeys=keys))
    (root / 'versions.dpapi').write_bytes(protect(raw))
    response = json.loads(raw)
    if response.get('resultCode') != 0:
        raise ValueError('Version query failed')
    versions = response['versions']
    if {v['type'] for v in versions} != set(types):
        raise ValueError('Incomplete version response')
    for item in versions:
        if item['version'] == 0:
            continue
        kind = item['type']
        print(json.dumps(dict(type=kind, name=types[kind]['name'])), flush=True)
        path = 'health/getHealthDataByVersion' if category == 0 else 'sequence/getSampleSequenceByVersion'
        if category == 'statistics':
            path = 'health/getHealthStatisticsByVersion'
        run(token_file, root / str(kind), path, kind, item['version'])
    (root / 'complete.json').write_text(json.dumps(dict(
        scope=f'APK dictionary category={category} plus confirmed extra streams', queried=len(types),
        streams=sum(v['version'] > 0 for v in versions))), encoding='utf-8')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('token')
    parser.add_argument('apk')
    parser.add_argument('destination')
    parser.add_argument('--category', choices=('point', 'sequence', 'statistics'), default='point')
    args = parser.parse_args()
    main(args.token, args.apk, args.destination,
         category={'point': 0, 'sequence': 1, 'statistics': 'statistics'}[args.category])
