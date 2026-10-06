import sys,re
SMALL=set("ゃゅょぁぃぅぇぉゎ")
V={}
for s,v in [("あかさたなはまやらわがざだばぱぁゃゎ","a"),("いきしちにひみりぎじぢびぴぃ","i"),("うくすつぬふむゆるぐずづぶぷぅゅ","u"),("えけせてねへめれげぜでべぺぇ","e"),("おこそとのほもよろをごぞどぼぽぉょ","o")]:
    for c in s: V[c]=v
VL=set("かきくけこさしすせそたちつてとはひふへほぱぴぷぺぽ")
def moras(t):
    t=re.sub(r"\s","",t); out=[]
    for c in t:
        if c in SMALL and out: out[-1]+=c
        else: out.append(c)
    return out
def vow(m):
    if m=="っ": return "Q"
    if m=="ん": return "N"
    if m=="ー": return "-"
    return V.get(m[-1],"?")
def cons_voiceless(m): return m=="っ" or m[0] in VL
rows=[];sec=None
for line in open(sys.argv[1],encoding="utf-8"):
    line=line.rstrip("\n")
    if line.startswith("#"):
        sec,bars=line[1:].split("|"); rows.append(("SEC",sec,bars)); continue
    kanji,kana=line.split("|")
    ms=moras(kana); vs=[vow(m) for m in ms]
    dev=[]
    for i,m in enumerate(ms):
        if vs[i] in "iu" and m[0] in VL and i+1<len(ms) and cons_voiceless(ms[i+1]): dev.append(m)
    rows.append((sec,kanji,kana,len(ms),"".join(vs),vs[-1] if vs else "",",".join(dev)))
cur=None
for r in rows:
    if r[0]=="SEC":
        print(f"\n### {r[1]}（{r[2]}小節）\n\n| 歌詞 | 読み | 音数 | 母音列 | 末尾 | 無声化しやすい音 |\n|---|---|---|---|---|---|"); continue
    print(f"| {r[1]} | {r[2]} | {r[3]} | {r[4]} | {r[5]} | {r[6] or '—'} |")
