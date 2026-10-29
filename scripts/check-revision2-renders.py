import json, pathlib, shutil, subprocess
from PIL import Image

root = pathlib.Path(__file__).resolve().parent.parent
exports = root / 'target/f6-export-tests'
out = root / 'target/f9-revision2-independent'
for folder in ('reference', 'pdf', 'eps'):
    (out / folder).mkdir(parents=True, exist_ok=True)
pdftoppm = shutil.which('pdftoppm')
gs = pathlib.Path('C:/Program Files/gs/gs10.01.2/bin/gswin64c.exe')
assert pdftoppm and gs.is_file()
ids = ['circle_r50', 'ring', 'concentric', 'logo_original_1', 'logo_original_2',
       'logo_compass', 'illustration_fish', 'illustration_robot',
       'illustration_rocket', 'photo_chelsea', 'photo_coffee', 'photo_camera',
       'photo_brick', 'photo_astronaut', 'ellipse', 'diagonal_touch']
for name in ids:
    reference = exports / (name + '.png')
    with Image.open(reference) as image:
        w, h = image.size
    shutil.copyfile(reference, out / 'reference' / reference.name)
    subprocess.run([pdftoppm, '-png', '-singlefile', '-r', '96', '-scale-to-x',
                    str(w), '-scale-to-y', str(h), str(exports / (name + '.pdf')),
                    str(out / 'pdf' / name)], check=True, capture_output=True)
    subprocess.run([str(gs), '-dSAFER', '-dBATCH', '-dNOPAUSE', '-sDEVICE=pngalpha',
                    '-dEPSCrop', '-r96', f'-g{w}x{h}', '-dTextAlphaBits=4',
                    '-dGraphicsAlphaBits=4', f'-sOutputFile={out / "eps" / reference.name}',
                    str(exports / (name + '.eps'))], check=True, capture_output=True)
    for kind in ('pdf', 'eps'):
        with Image.open(out / kind / reference.name) as image:
            assert image.size == (w, h), (name, kind, image.size, (w, h))
    print(name, flush=True)
(out / 'method.json').write_text(json.dumps({
    'source': 'all_formats_parse_and_preserve_the_entire_corpus outputs',
    'params': 'resolve_defaults Logo plus circular_loops=true; opaque_canvas=true',
    'cases': ids, 'poppler': '26.07.0', 'ghostscript': '10.01.2',
    'method': 'source-resolution independent AA render comparison, not topology gate',
    'product_linkage': 'external QA programs only; neither renderer distributed'
}, indent=2))
