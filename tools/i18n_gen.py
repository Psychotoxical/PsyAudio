import sys
d = {}
for line in open(sys.argv[1], encoding='utf-8'):
    line = line.rstrip('\n')
    if line.strip():
        k, v = line.split('\t'); d.setdefault(k, v)
rs = lambda s: '"' + s.replace('"', '\\"') + '"'
out = ['// Generated: German UI text -> English. German stays the key.', 'pub(crate) static EN: &[(&str, &str)] = &[']
out += [f'    ({rs(k)}, {rs(v)}),' for k, v in d.items()] + ['];']
open(sys.argv[2], 'w', encoding='utf-8', newline='\n').write('\n'.join(out) + '\n')
print(len(d))
