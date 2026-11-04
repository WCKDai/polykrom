"""Shared icon review state: read and write design/review.json (standard library only).

  {"version": 1, "final": {"<icon id>": "<sha256 of its master svg(s) when accepted>"}, "notes": {"<icon id>": "text"},
   "supplied": {"<icon id>": {"label": "<glyph name>", "source": "<kit file>", "sha256": "<sha256 of svg/supplied/<id>.svg>"}}}

`supplied` records the owner-supplied masters (svg/supplied): the design kit's exact vectors, recorded with
their source and content hash by the change that brought them in, never by the review page. They count as
accepted (valid_final) while the file still hashes as recorded; lint rejects an edited one.
Legacy acceptance lists migrate once when the review server starts; existing hashes are never refreshed implicitly.
"""
import os, re, json, glob, hashlib, tomllib, threading, uuid

LOCK = threading.RLock()

D = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))     # .../assets/icons/ui/design
REVIEW = D + '/review.json'

def aliases():
    return tomllib.load(open(D + '/aliases.toml', 'rb')).get('alias', {})
def master_of(icon_id, al=None):
    v = (aliases() if al is None else al).get(icon_id)
    return (v['master'] if isinstance(v, dict) else v) if v else icon_id
def lf(data):
    """Text as committed: a checkout with CRLF line endings (Git for Windows' default) hashes as LF."""
    return data.replace(b'\r\n', b'\n')
def master_hash(master, design=None):
    d = D if design is None else design
    h = hashlib.sha256()
    for f in sorted(glob.glob(d + '/svg/*/' + master + '.svg')):
        h.update(os.path.relpath(f, d).replace(os.sep, '/').encode()); h.update(lf(open(f, 'rb').read()))
    return h.hexdigest()
def supplied_hash(icon_id, design=None):
    """The content hash of an owner-supplied master alone (LF), the value review.json records."""
    d = D if design is None else design
    with open(d + '/svg/supplied/' + icon_id + '.svg', 'rb') as f:
        return hashlib.sha256(lf(f.read())).hexdigest()
def supplied(design=None):
    """The owner-supplied masters recorded in review.json: {id: {label, source, sha256}}."""
    d = D if design is None else design
    try:
        with open(d + '/review.json', encoding='utf-8') as f: r = json.load(f)
    except FileNotFoundError: return {}
    s = r.get('supplied', {})
    return {i: v for i, v in s.items() if isinstance(v, dict)} if isinstance(s, dict) else {}
def valid_supplied(design=None):
    """The supplied masters whose file still hashes as recorded: accepted by provenance."""
    d = D if design is None else design
    out = {}
    for i, v in supplied(d).items():
        if os.path.exists(d + '/svg/supplied/' + i + '.svg') and supplied_hash(i, d) == v.get('sha256'): out[i] = v['sha256']
    return out
def load():
    try: r = json.load(open(REVIEW, encoding='utf-8'))
    except FileNotFoundError: r = {}
    fin = r.get('final', {})
    if isinstance(fin, list): fin = {i: '' for i in fin}
    return {'final': dict(fin), 'notes': {k: v for k, v in r.get('notes', {}).items() if isinstance(v, str) and v.strip()}}
def valid_final(state=None):
    """Acceptance belongs to a particular artwork revision, not just an id. An owner-supplied master
    (and an alias that draws it) is accepted by provenance while its file hashes as recorded."""
    state = load() if state is None else state
    al = aliases()
    valid = {i: h for i, h in state['final'].items()
             if h and glob.glob(D + '/svg/*/' + master_of(i, al) + '.svg')
             and h == master_hash(master_of(i, al))}
    owner = valid_supplied()
    for i in list(owner) + [a for a, v in al.items() if not (isinstance(v, dict) and v.get('facet'))]:
        m = master_of(i, al)
        if m in owner: valid.setdefault(i, owner[m])
    return valid
def save(final, notes):
    """final: {id: hash or ''}; a missing hash is filled in from the master as it is now.
    The owner-supplied records are kept as they are: only a new owner snapshot changes them."""
    ok = lambda i: re.fullmatch(r'[a-z][a-z0-9_]*', i)
    al = aliases()
    owner = supplied()
    fin = {i: (h or master_hash(master_of(i, al))) for i, h in final.items() if ok(i) and i not in owner}
    nts = {k: v for k, v in notes.items() if ok(k) and isinstance(v, str) and v.strip()}
    data = {'version': 1, 'final': dict(sorted(fin.items())), 'notes': dict(sorted(nts.items()))}
    if owner: data['supplied'] = dict(sorted(owner.items()))
    temporary = REVIEW + '.' + uuid.uuid4().hex + '.tmp'
    with LOCK:
        try:
            with open(temporary, 'w', encoding='utf-8') as f:
                json.dump(data, f, indent=1); f.write('\n')
            os.replace(temporary, REVIEW)
        finally:
            if os.path.exists(temporary): os.unlink(temporary)
def resolve(masters_or_ids):
    """The icons were redrawn: drop their notes, and the notes on aliases of the same masters. Returns the ids cleared."""
    r = load(); al = aliases(); done = set(masters_or_ids)
    cleared = sorted(i for i in r['notes'] if i in done or master_of(i, al) in done)
    for i in cleared: del r['notes'][i]
    if cleared: save(r['final'], r['notes'])
    return cleared


def resolve_export(request, written):
    """Clear only still-matching exported notes on masters actually redrawn.

    Call after applying a snapshot's artwork feedback. Newer notes and final
    aliases are retained; this helper does not move requests or draw anything.
    """
    with LOCK:
        state = load()
        protected = {master_of(i) for i in valid_final(state)}
        cleared = []
        for icon, expected in request.get('notes', {}).items():
            master = master_of(icon)
            if master in written and master not in protected and state['notes'].get(icon) == expected:
                del state['notes'][icon]
                cleared.append(icon)
        if cleared: save(state['final'], state['notes'])
        return sorted(cleared)
