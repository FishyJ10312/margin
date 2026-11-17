"""Fail-closed packaging checks. Pattern checks are not a security certification."""
import hashlib
import json
import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
PATTERNS = {
    # /build/home is the deliberate generic Rust mapping, not a user's home.
    'personal-home': re.compile(rb'(?:/Users|(?<!/build)/home)/[A-Za-z0-9_.-]+[/\\]|[A-Za-z]:[\\/]Users[\\/][A-Za-z0-9_. -]+[\\/]'),
    'private-key': re.compile(rb'-----BEGIN (?:RSA |EC |OPENSSH )?PRIVATE KEY-----'),
    'api-key': re.compile(rb'\bsk-(?:proj-|svcacct-)?[A-Za-z0-9_-]{24,}'),
    'bearer-token': re.compile(rb'Bearer [A-Za-z0-9_+/.=-]{24,}'),
    'jwt': re.compile(rb'eyJ[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}\.[A-Za-z0-9_-]{12,}'),
}

def findings(data, label, *, synthetic_key=False):
    found = {name: len(pattern.findall(data)) for name, pattern in PATTERNS.items()}
    if synthetic_key:
        found.pop('private-key', None)
    found = {name: count for name, count in found.items() if count}
    if found:
        # Never echo matched bytes, source lines, credentials or machine paths.
        raise ValueError(f'Distribution privacy check rejected {label}: {json.dumps(found)}')

def verify_app(app):
    binaries = [app / 'Contents/MacOS/liubai', *sorted((app / 'Contents/Frameworks').glob('*.dylib'))]
    for binary in binaries:
        data = binary.read_bytes()
        label = binary.relative_to(app).as_posix()
        for prefix in (str(Path.home()), str(ROOT)):
            if len(prefix) > 1 and prefix.encode() in data:
                raise ValueError(f'Builder-specific prefix remains in {label}')
        findings(data, label)

def verify_source(relative, data):
    assets = json.loads((ROOT / 'scripts/source-assets.json').read_text())
    label = relative.as_posix()
    digest = hashlib.sha256(data).hexdigest()
    if relative.name.lower() in {'auth.json', 'credentials.json', 'accounts.json', 'tokens.json', 'id_rsa', 'id_ed25519'}:
        raise ValueError(f'Credential-store filename is not source material: {label}')
    if b'\0' in data or relative.suffix.lower() in {'.png', '.jpg', '.jpeg', '.heic', '.heif', '.ttf', '.otf', '.icns', '.ico', '.pem'}:
        if assets.get(label) != digest:
            raise ValueError(f'Unreviewed or changed source asset: {label}')
    synthetic_key = label == 'src-tauri/test-fixtures/oauth-test-only.pem' and assets.get(label) == digest
    findings(data, label, synthetic_key=synthetic_key)
