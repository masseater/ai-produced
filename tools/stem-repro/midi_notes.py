import mido


def notes(path, track=None):
    m = mido.MidiFile(path)
    tmap = [(0, 0.0, 500000)]
    tick = 0
    for msg in mido.merge_tracks(m.tracks):
        tick += msg.time
        if msg.type == "set_tempo":
            t0, s0, tempo = tmap[-1]
            tmap.append((tick, s0 + (tick - t0) * tempo / 1e6 / m.ticks_per_beat, msg.tempo))

    def seconds(tk):
        t0, s0, tempo = [e for e in tmap if e[0] <= tk][-1]
        return s0 + (tk - t0) * tempo / 1e6 / m.ticks_per_beat

    out = []
    for tr in [m.tracks[track]] if track is not None else m.tracks:
        tick = 0
        held = {}
        for msg in tr:
            tick += msg.time
            key = (getattr(msg, "channel", 0), getattr(msg, "note", 0))
            if msg.type == "note_on" and msg.velocity > 0:
                held.setdefault(key, []).append((tick, msg.velocity))
            elif msg.type in ("note_off", "note_on") and held.get(key):
                start, vel = held[key].pop(0)
                out.append((seconds(start), seconds(tick), msg.note, vel))
    return sorted(out)


def events(lst, t0=0.0, t1=None):
    ev = []
    for s, e, p, v in lst:
        end = e if t1 is None else min(e, t1)
        ev.append((mido.Message("note_on", note=p, velocity=v).bytes(), max(0.0, s - t0)))
        ev.append((mido.Message("note_off", note=p).bytes(), end - t0))
    return sorted(ev, key=lambda x: x[1])
