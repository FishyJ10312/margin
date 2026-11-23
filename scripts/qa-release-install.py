#!/usr/bin/env python3
"""Smoke-test the exact installer on a disposable GitHub-hosted runner.

Never seed the production bundle identity on a user's Mac or Windows PC.
No external inference, photos or credentials are used.
"""
import argparse
import base64
import hashlib
import json
import os
from pathlib import Path
import queue
import shutil
import subprocess
import sys
import threading
import time

from PIL import Image
from distribution_privacy import findings


class Client:
    def __init__(self, exe, connection, error_log):
        self.log = error_log.open("wb")
        self.process = subprocess.Popen(
            [str(exe), "--mcp", "--connection-file", str(connection)],
            stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.log,
        )
        self.lines = queue.Queue()
        self.identifier = 0
        threading.Thread(target=self.read, daemon=True).start()

    def read(self):
        for line in self.process.stdout:
            self.lines.put(line)
        self.lines.put(None)

    def rpc(self, method, params=None, notification=False):
        self.identifier += 1
        request = {"jsonrpc": "2.0", "method": method, "params": params or {}}
        if not notification:
            request["id"] = self.identifier
        self.process.stdin.write((json.dumps(request) + "\n").encode("utf-8"))
        self.process.stdin.flush()
        if notification:
            return None
        try:
            line = self.lines.get(timeout=120)
        except queue.Empty:
            raise RuntimeError("Installed app did not answer MCP within 120 seconds")
        if line is None:
            raise RuntimeError("Installed MCP connector exited unexpectedly")
        response = json.loads(line)
        assert response["jsonrpc"] == "2.0" and response["id"] == self.identifier
        return response

    def initialize(self):
        answer = self.rpc("initialize", {
            "protocolVersion": "2025-11-25", "capabilities": {},
            "clientInfo": {"name": "Margin release smoke", "version": "1"},
        })
        assert answer["result"]["protocolVersion"] == "2025-11-25"
        self.rpc("notifications/initialized", notification=True)
        assert len(self.rpc("tools/list")["result"]["tools"]) == 14

    def tool(self, name, args=None):
        answer = self.rpc("tools/call", {"name": name, "arguments": args or {}})
        if "error" in answer:
            raise RuntimeError(answer["error"]["message"])
        result = answer["result"]
        if result.get("isError"):
            raise RuntimeError(result["content"][0]["text"])
        return result

    def data(self, name, args=None):
        return self.tool(name, args)["structuredContent"]

    def close(self):
        try:
            self.process.stdin.close()
            try:
                self.process.wait(timeout=5)
            except subprocess.TimeoutExpired:
                self.process.terminate()
                self.process.wait(timeout=5)
        finally:
            self.log.close()


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    cli = argparse.ArgumentParser(description=__doc__)
    cli.add_argument("--installer", type=Path, help="Windows NSIS executable")
    cli.add_argument("--app", type=Path, help="Mac app copied from the mounted DMG")
    args = cli.parse_args()
    if os.environ.get("GITHUB_ACTIONS") != "true" or not os.environ.get("RUNNER_TEMP"):
        raise SystemExit("Refusing to use the production identity outside an isolated hosted runner")
    root = Path(os.environ["RUNNER_TEMP"]).resolve() / "margin-release-smoke"
    root.mkdir()
    if sys.platform == "win32":
        assert args.installer and not args.app
        profile = Path(os.environ["APPDATA"]) / "com.fishyj.liubai"
    elif sys.platform == "darwin":
        assert args.app and not args.installer
        profile = Path.home() / "Library/Application Support/com.fishyj.liubai"
    else:
        raise SystemExit("Unsupported host")
    assert not profile.exists(), "Refusing to touch a pre-existing Margin profile"
    if args.installer:
        destination = root / "installed"
        # NSIS requires /D to be the final argument. The entire remainder is the path.
        command = f'"{args.installer.resolve()}" /S /D={destination}'
        subprocess.run(command, check=True, timeout=180)
        exe = destination / "liubai.exe"
        assert exe.is_file(), "Installer did not install the executable"
        dlls = sorted(destination.glob("*.dll"))
        assert any("heif" in p.name.lower() for p in dlls)
        assert any("de265" in p.name.lower() for p in dlls)
        assert (destination / "Licenses/NativeSources/README.md").is_file()
        assert (destination / "Licenses/NativeSources/libheif-1.23.7-vcpkg-source.tar.gz").is_file()
        assert (destination / "Licenses/NativeSources/libde265-1.1.3-vcpkg-source.tar.gz").is_file()
        for binary in [exe, *dlls]:
            findings(binary.read_bytes(), binary.name)
    else:
        exe = args.app.resolve() / "Contents/MacOS/liubai"
        assert exe.is_file()
        subprocess.run(["codesign", "--verify", "--deep", "--strict", str(args.app)], check=True)
    agent = profile / "agent-mcp"
    agent.mkdir(parents=True, mode=0o700)
    settings = agent / "settings.json"
    settings.write_text('{"enabled":true,"allowWrites":true}', encoding="utf-8")
    os.chmod(settings, 0o600)
    connection = agent / "connection.json"
    media = root / "照片"
    media.mkdir()
    paths = [media / "portrait.jpg", media / "landscape.png", media / "HEIC示例.heic"]
    Image.new("RGB", (600, 900), (80, 130, 170)).save(paths[0])
    Image.new("RGBA", (900, 600), (200, 130, 70, 220)).save(paths[1])
    shutil.copy2(Path(__file__).resolve().parents[1] / "tests/example.heic", paths[2])
    hashes = {str(p): digest(p) for p in paths}
    outputs = root / "exports"
    with (root / "desktop-error.log").open("wb") as log:
        app = subprocess.Popen([str(exe)], stdout=log, stderr=log)
        client = None
        try:
            deadline = time.monotonic() + 90
            while not connection.is_file():
                assert app.poll() is None, "Installed app exited before starting its window"
                if time.monotonic() > deadline:
                    raise RuntimeError("Installed desktop did not start")
                time.sleep(.25)
            client = Client(exe, connection, root / "connector-error.log")
            client.initialize()
            while True:
                try:
                    workspace = client.data("margin_get_workspace")
                    break
                except RuntimeError as error:
                    if ("尚未就绪" not in str(error) and "not ready" not in str(error).lower()) or time.monotonic() > deadline:
                        raise
                    time.sleep(.25)
            assert workspace["photoCount"] == 0
            imported = client.data("margin_import_photos", {"paths": [str(p) for p in paths]})
            assert imported["added"] == 3 and not imported["errors"]
            photos = client.data("margin_get_workspace")["photos"]
            ids = [photo["id"] for photo in photos]
            client.data("margin_select_photos", {"ids": ids, "activeId": ids[0], "anchorId": ids[0]})
            fonts = client.data("margin_list_fonts")["fonts"]
            chinese = next(font for font in fonts if font["chinese"] and font["bundled"])
            client.data("margin_update_settings", {"patch": {
                "ratio": {"width": 4, "height": 5}, "borderPercent": 5,
                "frameColor": "#F3ECDD",
                "watermark": {"kind": "text", "text": "Margin 测试",
                              "fontFamily": chinese["family"], "region": "photo",
                              "position": "bottom-center", "sizePercent": 3},
                "export": {"format": "png", "longEdge": 1200},
            }})
            preview = client.tool("margin_get_preview")
            image = next(c for c in preview["content"] if c["type"] == "image")
            assert base64.b64decode(image["data"], validate=True)
            client.data("margin_export_photos", {"scope": "all", "outputDirectory": str(outputs)})
            deadline = time.monotonic() + 180
            while True:
                batch = client.data("margin_get_export_status")["batch"]
                if batch["status"] != "running":
                    break
                if time.monotonic() > deadline:
                    raise RuntimeError("Installed app export timed out")
                time.sleep(.2)
            assert batch["status"] == "completed" and len(batch["items"]) == 3
            for item in batch["items"]:
                assert item["status"] == "completed"
                with Image.open(item["outputPath"]) as image:
                    image.load()
                    assert max(image.size) <= 1200 and abs(image.width * 5 - image.height * 4) <= 5
                    assert image.info.get("icc_profile")
            assert hashes == {str(p): digest(p) for p in paths}
            print(json.dumps({"platform": sys.platform, "installedPhotoImports": 3,
                              "formats": ["JPG", "PNG", "HEIC"], "renderedPreview": True,
                              "exports": 3, "originalsIntact": True, "modelCalls": 0}))
        finally:
            if client:
                client.close()
            if app.poll() is None:
                app.terminate()
                app.wait(timeout=10)


if __name__ == "__main__":
    main()
