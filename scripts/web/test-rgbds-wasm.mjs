const out = process.argv[2];
const { default: asm } = await import(out + '/rgbasm.js');
const { default: lnk } = await import(out + '/rgblink.js');
const { default: fix } = await import(out + '/rgbfix.js');
async function run(f, args, files) {
  const log = [];
  let status = 'returned';
  try {
    const m = await f({ arguments: args, noInitialRun: false,
      preRun: [(m) => { for (const [k, v] of Object.entries(files)) m.FS.writeFile(k, v); }],
      print: (s) => log.push('out:' + s), printErr: (s) => log.push('err:' + s),
      onExit: (c) => { status = 'exit ' + c; } });
    return { m, log, status };
  } catch (e) { return { log, status: 'threw ' + (e && e.message || e) + ' ' + JSON.stringify(e && e.status) }; }
}
const good = 'SECTION "m",ROM0[$100]\nnop\njp Start\nStart:\n.l: jr .l\n';
let a = await run(asm, ['-Wall','--color','never','-o','a.o','--','main.asm'], {'main.asm': good});
console.log('asm good', a.status, a.log, a.m && [...a.m.FS.readFile('a.o')].length);
let a2 = await run(asm, ['--color','never','-o','a.o','--','main.asm'], {'main.asm': 'ld a,\nfoo bar\n'});
console.log('asm bad', a2.status, a2.log);
let l = await run(lnk, ['--color','never','-o','o.gb','-m','o.map','-n','o.sym','--','a.o'], {'a.o': a.m.FS.readFile('a.o')});
console.log('link', l.status, l.log, [...l.m.FS.readFile('o.gb')].length, l.m.FS.readFile('o.sym',{encoding:'utf8'}).trim().split('\n'));
let f = await run(fix, ['--color','never','-p','0xff','-v','--','o.gb'], {'o.gb': l.m.FS.readFile('o.gb')});
console.log('fix', f.status, f.log, [...f.m.FS.readFile('o.gb')].length);
// re-entry on same instance? and state reset: second run of fresh instance
let a3 = await run(asm, ['--color','never','-o','a.o','--','main.asm'], {'main.asm': good});
console.log('asm again', a3.status, a3.log);
