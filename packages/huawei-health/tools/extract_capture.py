"""Extract complete archive events from a frida-tools -o log, without overwriting."""
import json
import os
import pathlib
import sys

PREFIX = 'HUAWEI_ARCHIVE '


def extract(source, destination):
    count = 0
    partial = destination.with_name(destination.name + '.partial')
    if destination.exists():
        raise FileExistsError(destination)
    with source.open(encoding='utf-8-sig') as lines, partial.open('x', encoding='utf-8') as out:
        for number, line in enumerate(lines, 1):
            if 'HUAWEI_ARCHIVE_ERROR' in line:
                raise ValueError(f'Capture reported a lost response at log line {number}')
            if PREFIX not in line:
                continue
            event = json.loads(line.split(PREFIX, 1)[1])
            if event.get('schema') != 1 or not isinstance(event.get('response_json'), str):
                raise ValueError(f'Invalid archive event at log line {number}')
            json.loads(event['response_json'])  # Validate, but retain original integer precision.
            out.write(json.dumps(event, ensure_ascii=False) + '\n')
            count += 1
    if not count:
        raise ValueError('No responses captured; this is not a successful backup')
    # Publish only validated captures; link fails atomically if destination exists.
    os.link(partial, destination)
    partial.unlink()
    return count


if __name__ == '__main__':
    print('Saved responses:', extract(pathlib.Path(sys.argv[1]), pathlib.Path(sys.argv[2])))
