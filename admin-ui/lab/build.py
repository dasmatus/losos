#!/usr/bin/env python3
"""Assemble LosOS Lab from src/ into one of three shapes.

  build.py box    OUT   what the appliance serves at /lab/: index.html, lab.js,
                        lab.css and plate.png, no inline script, no external
                        host. Simulated consoles, plus the "This box" setup
                        read from the box's own API.
  build.py hosted OUT   the same files for a cross-origin isolated host
                        (Vercel, serve.py), where engine/build.sh has put
                        qemu-wasm and the guests beside them; consoles are
                        real x86_64 guests.
  build.py single FILE  one self-contained page, the logo inlined, for a
                        viewer that serves nothing else.

Python's standard library only, so the nix derivation needs no npm.

LOSOS_LAB_MEME names a folder with no.jpg and yes.jpg: the splash then
draws them left of its two halves. The repository carries no such folder.
"""
import base64, json, os, pathlib, shutil, sys

here = pathlib.Path(__file__).resolve().parent
src = here / 'src'
plate = here.parent / 'themes' / 'brand' / 'plate-128.png'
meme = pathlib.Path(os.environ['LOSOS_LAB_MEME']) if os.environ.get('LOSOS_LAB_MEME') else None
ORDER = ['icons.js', 'model.js', 'sim.js', 'pages.js', 'term.js', 'scenarios.js',
         'thisbox.js', 'app.js', 'side.js', 'engine.js', 'files.js', 'main.js']


def script(config: dict) -> str:
    js = 'const LAB = ' + json.dumps(config) + ';\n'
    for f in ORDER:
        js += f'\n// ===== {f} =====\n' + (src / f).read_text()
    return js


def page(head: str, body_end: str) -> str:
    shell = (src / 'shell.html').read_text()
    shell = shell.replace('<!--HEAD-->', head).replace('<!--SCRIPT-->', body_end)
    return ('<!doctype html>\n<html lang="en">\n<head>\n<meta charset="utf-8">\n'
            '<meta name="viewport" content="width=device-width, initial-scale=1, viewport-fit=cover">\n'
            + shell + '\n</html>\n')


def split(out: pathlib.Path, config: dict) -> None:
    out.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(plate, out / 'plate.png')
    if meme:
        for f in ('no.jpg', 'yes.jpg'):
            shutil.copyfile(meme / f, out / ('meme-' + f))
        config['meme'] = ['meme-no.jpg', 'meme-yes.jpg']
    (out / 'lab.css').write_text((src / 'style.css').read_text())
    (out / 'lab.js').write_text(script(config))
    (out / 'index.html').write_text(page(
        '<link rel="stylesheet" href="lab.css">',
        '<script src="lab.js"></script>'))


def single(out: pathlib.Path) -> None:
    uri = 'data:image/png;base64,' + base64.b64encode(plate.read_bytes()).decode()
    config = {'box': False, 'engine': False, 'plate': uri}
    if meme:
        config['meme'] = ['data:image/jpeg;base64,' + base64.b64encode((meme / f).read_bytes()).decode() for f in ('no.jpg', 'yes.jpg')]
    js = script(config).replace('</script', '<\\/script')
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(page(
        '<style>' + (src / 'style.css').read_text() + '</style>',
        '<script>\n' + js + '\n</script>'))


if __name__ == '__main__':
    if len(sys.argv) != 3 or sys.argv[1] not in ('box', 'hosted', 'single'):
        sys.exit(__doc__)
    mode, out = sys.argv[1], pathlib.Path(sys.argv[2])
    if mode == 'single':
        single(out)
    else:
        split(out, {'box': mode == 'box', 'engine': mode == 'hosted', 'plate': 'plate.png'})
    print('built', mode, out)
