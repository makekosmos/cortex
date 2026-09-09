import json
import pathlib
import tempfile
from extract_capture import extract

with tempfile.TemporaryDirectory() as tmp:
    root = pathlib.Path(tmp)
    source = root / 'capture.log'
    raw = '{"currentVersion":9223372036854775806,"detailInfos":[]}'
    event = {'schema': 1, 'response_json': raw}
    source.write_text('banner\nHUAWEI_ARCHIVE ' + json.dumps(event) + '\n', encoding='utf-8')
    destination = root / 'archive.jsonl'
    assert extract(source, destination) == 1
    assert json.loads(destination.read_text())['response_json'] == raw
    try:
        extract(source, destination)
        raise AssertionError('Overwrote existing archive')
    except FileExistsError:
        pass
    for number, content in enumerate(['banner\n', 'HUAWEI_ARCHIVE {broken', 'HUAWEI_ARCHIVE_ERROR lost']):
        source.write_text(content, encoding='utf-8')
        try:
            extract(source, root / f'bad-{number}.jsonl')
            raise AssertionError('Accepted incomplete/empty capture')
        except ValueError:
            pass
        assert not (root / f'bad-{number}.jsonl').exists()
        assert (root / f'bad-{number}.jsonl.partial').exists()
print('PASS: extraction, integer preservation, no overwrite, incomplete capture rejected')
