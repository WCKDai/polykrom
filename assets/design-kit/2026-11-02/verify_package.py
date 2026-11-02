from pathlib import Path
import hashlib
root=Path(__file__).resolve().parent
for line in (root/'SHA256SUMS.txt').read_text().splitlines():
    expected,name=line.split('  ',1)
    assert hashlib.sha256((root/name).read_bytes()).hexdigest()==expected,name
print('All package checksums passed.')
