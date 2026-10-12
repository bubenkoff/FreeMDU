# Analyse a .ramlog (8-byte ms timestamp + 1024 bytes RAM per record).
import sys, struct, time
def load(path):
    b = open(path, 'rb').read(); recs = []
    for i in range(0, len(b) - 1031, 1032):
        recs.append((struct.unpack('<Q', b[i:i+8])[0] / 1000, b[i+8:i+1032]))
    return recs
def mc14489(d):
    pts = (d[2] & 0x70) >> 4; out = ''
    for n, (code, sp) in enumerate(((d[0] & 15, d[3] & 2), (d[0] >> 4, d[3] & 4), (d[1] & 15, d[3] & 8))):
        out += ('0123456789AbCdEF'[code] if not sp else ' -?' [min(code, 2)] if code < 3 else '?')
        if pts in (n + 1, 7): out += '.'
    return out
if __name__ == '__main__':
    recs = load(sys.argv[1]); t0 = recs[0][0]
    cols = [int(x, 16) for x in sys.argv[2].split(',')]
    prev = None
    for t, d in recs:
        row = tuple(d[c] for c in cols)
        if row != prev:
            print('%5.1f min  %s  disp[%s] %s' % ((t - t0) / 60, ' '.join('%04x=%02x' % (c, d[c]) for c in cols), mc14489(d[0x9e:0xa2]), d[0x9e:0xa2].hex()))
        prev = row
