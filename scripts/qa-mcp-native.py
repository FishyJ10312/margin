#!/usr/bin/env python3
"""Exercise the packaged stdio connector against an isolated, open Margin app.

Interactive phases let native UI observation and a real GUI process restart happen
while the same MCP connector process stays alive. Never use a user's real profile.
"""
import argparse
import base64
import hashlib
import json
from pathlib import Path
import selectors
import subprocess
import sys
import time


class Client:
    def __init__(self, executable, connection, output):
        self.errors = (output / 'stdio-stderr.log').open('wb')
        self.process = subprocess.Popen([str(executable), '--mcp', '--connection-file', str(connection)], stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.errors)
        self.selector = selectors.DefaultSelector()
        self.selector.register(self.process.stdout, selectors.EVENT_READ)
        self.identifier = 0
        self.initialized = False

    def rpc(self, method, params=None, notification=False):
        self.identifier += 1
        request = {'jsonrpc': '2.0', 'method': method}
        if params is not None:
            request['params'] = params
        if not notification:
            request['id'] = self.identifier
        self.process.stdin.write((json.dumps(request, ensure_ascii=False) + '\n').encode())
        self.process.stdin.flush()
        if notification:
            return None
        if not self.selector.select(120):
            raise RuntimeError('MCP response timed out; inspect the app before retrying')
        line = self.process.stdout.readline()
        response = json.loads(line)
        assert response.get('jsonrpc') == '2.0' and response.get('id') == self.identifier, response
        return response

    def initialize(self):
        if self.initialized:
            return
        result = self.rpc('initialize', {'protocolVersion': '2025-11-25', 'capabilities': {}, 'clientInfo': {'name': 'Margin native acceptance', 'version': '1'}})
        assert result['result']['protocolVersion'] == '2025-11-25', result
        self.rpc('notifications/initialized', notification=True)
        tools = self.rpc('tools/list')['result']['tools']
        assert len(tools) == 14
        self.initialized = True

    def tool(self, name, arguments=None, allow_error=False):
        result = self.rpc('tools/call', {'name': name, 'arguments': arguments or {}})
        if 'error' in result:
            if allow_error:
                return result
            raise RuntimeError(result['error']['message'])
        result = result['result']
        if result.get('isError') and not allow_error:
            raise RuntimeError(result['content'][0]['text'])
        return result

    def data(self, name, arguments=None):
        return self.tool(name, arguments)['structuredContent']

    def close(self):
        self.process.stdin.close()
        self.process.wait(timeout=5)
        self.errors.close()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--app', type=Path, required=True)
    parser.add_argument('--connection', type=Path, required=True)
    parser.add_argument('--fixtures', type=Path, required=True)
    parser.add_argument('--out', type=Path, required=True)
    args = parser.parse_args()
    assert 'com.fishyj.margin.qa.' in str(args.connection), 'Refusing a non-QA profile'
    output = args.out.resolve()
    client = Client(args.app / 'Contents/MacOS/liubai', args.connection, output)
    photos = sorted(args.fixtures.glob('*.png'))
    hashes = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in photos}
    evidence = {'realModelCalls': 0, 'transport': 'packaged executable stdio -> private loopback -> live Svelte window'}
    original = []
    selected = []

    def workspace():
        return client.data('margin_get_workspace')

    def sources_intact():
        assert {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in photos} == hashes

    def finish_batch():
        for _ in range(120):
            batch = client.data('margin_get_export_status')['batch']
            if batch['status'] != 'running':
                return batch
            time.sleep(.15)
        raise RuntimeError('Export did not finish')

    try:
        print('Ready: edit, export, history, restore, readonly, modern, disabled, done', flush=True)
        for line in sys.stdin:
            phase = line.strip()
            if phase == 'done':
                break
            if phase != 'disabled':
                client.initialize()
            if phase == 'edit':
                assert workspace()['photoCount'] == 0
                imported = client.data('margin_import_photos', {'paths': [str(p.resolve()) for p in photos]})
                assert imported['added'] == 6 and not imported['errors']
                templates = client.data('margin_list_templates')['templates']
                assert templates and all('thumbnail' not in t for t in templates)
                white = next((t for t in templates if 'white' in (t.get('englishName', '') + t['name']).lower()), templates[0])
                client.data('margin_apply_template', {'id': white['id']})
                fonts = client.data('margin_list_fonts')['fonts']
                chinese = next(f for f in fonts if f['chinese'] and f['bundled'])
                original = [p['id'] for p in workspace()['photos']]
                selected = [original[1], original[3]]
                client.data('margin_select_photos', {'ids': selected, 'activeId': selected[-1], 'anchorId': selected[0]})
                client.data('margin_update_settings', {'patch': {'ratio': {'width': 4, 'height': 5}, 'borderPercent': 5, 'frameColor': '#F3ECDD', 'watermark': {'kind': 'text', 'text': 'FishyJ · MCP 测试', 'fontFamily': chinese['family'], 'sizePercent': 2.5, 'region': 'border', 'position': 'bottom-center', 'color': '#355E49'}}})
                preview = client.tool('margin_get_preview')
                picture = next(c for c in preview['content'] if c['type'] == 'image')
                extension = 'png' if picture['mimeType'] == 'image/png' else 'jpg'
                (output / ('mcp-preview.' + extension)).write_bytes(base64.b64decode(picture['data'], validate=True))
                assert preview['structuredContent']['width'] == 688 and preview['structuredContent']['height'] == 860
                current = workspace()
                assert current['selection']['ids'] == selected and current['settings']['watermark']['text'] == 'FishyJ · MCP 测试'
                assert all('thumbnail' not in p for p in current['photos'])
                evidence['edit'] = {'photos': 6, 'selected': 2, 'previewSize': [688, 860], 'template': white['id'], 'chineseFont': chinese['family'], 'previewImageReceived': True}
            elif phase == 'export':
                exported = client.data('margin_export_photos', {'outputDirectory': str(output / 'qa-exports')})
                assert exported['photoCount'] == 2
                batch = finish_batch()
                assert batch['status'] == 'completed' and len(batch['items']) == 2
                from PIL import Image
                for item in batch['items']:
                    assert item['status'] == 'completed'
                    with Image.open(item['outputPath']) as image:
                        assert image.size == (688, 860)
                client.data('margin_export_photos', {'outputDirectory': str(output / 'qa-cancel-exports'), 'scope': 'all'})
                client.data('margin_cancel_export')
                cancelled = finish_batch()
                assert cancelled['status'] in ('cancelled', 'completed')
                evidence['export'] = {'selectedOnlyCompleted': 2, 'dimensions': [688, 860], 'cancelRequestAcknowledged': True, 'cancelBatchStatus': cancelled['status']}
            elif phase == 'history':
                removed = client.data('margin_remove_photos', {'ids': selected})
                assert removed['photoCount'] == 4
                restored = client.data('margin_undo_removal')
                assert [p['id'] for p in restored['photos']] == original and restored['selection']['ids'] == selected
                assert client.data('margin_redo_removal')['photoCount'] == 4
                evidence['history'] = {'counts': [6, 4, 6, 4], 'selectionRestored': True, 'awaitingGuiRestart': True}
            elif phase == 'restore':
                assert workspace()['photoCount'] == 4
                restored = client.data('margin_undo_removal')
                assert [p['id'] for p in restored['photos']] == original and restored['selection']['ids'] == selected
                evidence['restart'] = {'sameStdioProcessReconnected': True, 'undoRestoredOrderAndSelection': True}
            elif phase == 'readonly':
                before = workspace()
                result = client.tool('margin_update_settings', {'patch': {'borderPercent': 7}}, allow_error=True)
                assert result.get('isError')
                assert workspace()['settings'] == before['settings']
                denied = client.rpc('tools/call', {'name': 'get_chatgpt_state', 'arguments': {}})
                assert denied['error']['code'] == -32602
                evidence['readOnly'] = {'workspaceReadable': True, 'settingsMutationDenied': True, 'accountRpcAbsent': True}
            elif phase == 'modern':
                meta = {'io.modelcontextprotocol/protocolVersion': '2026-07-28', 'io.modelcontextprotocol/clientInfo': {'name': 'Margin native acceptance', 'version': '1'}, 'io.modelcontextprotocol/clientCapabilities': {}}
                discovered = client.rpc('server/discover', {'_meta': meta})
                assert discovered['result']['supportedVersions'][0] == '2026-07-28'
                result = client.rpc('tools/call', {'name': 'margin_get_workspace', 'arguments': {}, '_meta': meta})
                assert result['result']['resultType'] == 'complete' and result['result']['structuredContent']['photoCount'] == 6
                evidence['modern'] = {'discovery': True, 'perRequestMetadataAndHeaders': True}
            elif phase == 'disabled':
                result = client.tool('margin_get_workspace', allow_error=True)
                assert result.get('error') and result['error']['code'] == -31000
                evidence['disabled'] = {'furtherRequestsDenied': True}
            else:
                raise ValueError('Unknown phase: ' + phase)
            sources_intact()
            evidence['sourceHashesUnchanged'] = 6
            (output / 'native-mcp-results.json').write_text(json.dumps(evidence, indent=2, ensure_ascii=False) + '\n')
            print(json.dumps({'phase': phase, 'passed': True, 'result': evidence.get(phase, evidence.get('restart'))}, ensure_ascii=False), flush=True)
    finally:
        client.close()


if __name__ == '__main__':
    main()
