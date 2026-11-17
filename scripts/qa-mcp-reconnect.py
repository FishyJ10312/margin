#!/usr/bin/env python3
"""Lost-result/reconnect checks against an explicitly isolated native app.

Setup imports/removals only establish the synthetic state for these new cases;
normal discovery/export acceptance is covered by qa-mcp-native.py.
"""
import argparse
import hashlib
import importlib.util
import json
from pathlib import Path
import time

spec = importlib.util.spec_from_file_location('native_client', Path(__file__).with_name('qa-mcp-native.py'))
native_client = importlib.util.module_from_spec(spec)
spec.loader.exec_module(native_client)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for field in ['app', 'connection', 'fixtures', 'out']:
        parser.add_argument('--' + field, type=Path, required=True)
    args = parser.parse_args()
    assert 'com.fishyj.margin.qa.' in str(args.connection), 'Refusing a production profile'
    output = args.out.resolve()
    fixtures = args.fixtures.resolve()
    assert output in fixtures.parents, 'Fixtures must be inside the isolated output folder'
    sources = sorted(fixtures.glob('*.png'))
    assert len(sources) == 26, 'Expected exactly 26 synthetic PNG fixtures'
    originals = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sources}
    clients = []

    def connect():
        log = output / 'clients' / str(len(clients))
        log.mkdir(parents=True)
        client = native_client.Client(args.app / 'Contents/MacOS/liubai', args.connection, log)
        clients.append(client)
        client.initialize()
        return client

    def close(client):
        client.close()
        clients[clients.index(client)] = None

    def send_without_reading(client, name, arguments):
        client.identifier += 1
        request = {'jsonrpc': '2.0', 'id': client.identifier, 'method': 'tools/call', 'params': {'name': name, 'arguments': arguments}}
        client.process.stdin.write((json.dumps(request) + '\n').encode())
        client.process.stdin.flush()

    def wait_for(read, predicate):
        for _ in range(100):
            result = read()
            if predicate(result):
                return result
            time.sleep(.05)
        raise AssertionError('Expected isolated app state did not arrive')

    def replay(client, name, arguments):
        result = wait_for(lambda: client.tool(name, arguments, allow_error=True), lambda r: not r.get('isError'))
        assert 'error' not in result, result
        return result['structuredContent']

    try:
        observer = connect()
        assert observer.data('margin_get_workspace')['photoCount'] == 0
        assert observer.data('margin_import_photos', {'paths': [str(p) for p in sources]})['added'] == 26
        workspace = observer.data('margin_get_workspace')
        ids = [p['id'] for p in workspace['photos']]
        observer.data('margin_select_photos', {'ids': ids[:1]})

        first = connect()
        export_args = {'operationId': 'native-lost-export-result', 'outputDirectory': str(output / 'lost-result-exports'), 'scope': 'ids', 'ids': ids[:2]}
        send_without_reading(first, 'margin_export_photos', export_args)
        batch = wait_for(lambda: observer.data('margin_get_export_status')['batch'], lambda b: b and b['status'] == 'completed')
        assert len(batch['items']) == 2 and all(item['status'] == 'completed' for item in batch['items'])
        close(first)  # The original client never consumed the tools/call reply.
        reconnected = connect()
        repeated = replay(reconnected, 'margin_export_photos', export_args)
        assert repeated['batch']['id'] == batch['id']
        assert observer.data('margin_get_export_status')['batch']['id'] == batch['id']
        assert len(list((output / 'lost-result-exports').glob('*.jpg'))) == 2
        evidence = {'lostExportResult': {'sameBatchAfterNewClient': True, 'exactlyTwoOutputFiles': True}, 'realModelCalls': 0}
        print(json.dumps({'phase': 'export-reconnect', 'passed': True}), flush=True)

        for _ in range(19):
            observer.data('margin_remove_photos')
        remover = connect()
        remove_args = {'operationId': 'native-lost-removal-result'}
        send_without_reading(remover, 'margin_remove_photos', remove_args)
        before = wait_for(lambda: observer.data('margin_get_workspace'), lambda w: w['photoCount'] == 6 and w['history']['undoCount'] == 20)
        close(remover)
        another = connect()
        replay(another, 'margin_remove_photos', remove_args)
        after = observer.data('margin_get_workspace')
        for field in ['photos', 'selection', 'history']:
            assert after[field] == before[field], field
        evidence['lostRemovalResult'] = {'newClientReplayKeptSixPhotos': True, 'allTwentyUndoRecordsKept': True, 'selectionKept': True}
        assert {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in sources} == originals
        evidence['syntheticSourcesUnchanged'] = 26
        (output / 'native-reconnect-results.json').write_text(json.dumps(evidence, indent=2) + '\n')
        print(json.dumps({'phase': 'removal-reconnect', 'passed': True, 'syntheticSourcesUnchanged': 26}), flush=True)
    finally:
        for client in clients:
            if client is not None:
                client.close()


if __name__ == '__main__':
    main()
