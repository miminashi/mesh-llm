import sys
p, which = sys.argv[1], sys.argv[2]
s = open(p).read()
M = {
 # M1: delete goes back to removing the record's install_path
 "delete": ('''        let install_path = self.install_path(name);
        // `symlink_metadata`''', '''        let install_path = self.load(name).map(|m| m.install_path).unwrap_or_else(|_| self.install_path(name));
        // `symlink_metadata`'''),
 # M2: no install_path check on load/list/save
 "check": ('''        let expected = self.install_path(name);
        (metadata.install_path != expected).then(|| {''', '''        let expected = self.install_path(name);
        (metadata.install_path != expected && false).then(|| {'''),
 # M3: only check "under the install root", not the exact directory
 "prefix": ('''        (metadata.install_path != expected).then(|| {''', '''        (!metadata.install_path.starts_with(self.install_root())).then(|| {'''),
 # M4: no install root guard before extraction
 "root": ('''    if options.install_root != expected_root {''', '''    if options.install_root != expected_root && false {'''),
}
old, new = M[which]
assert s.count(old) == 1, which
open(p, 'w').write(s.replace(old, new))
