//! Additional original artwork plus licensed photographic sources for F9.
use crate::{
    corpus::{self, Entry},
    Result,
};
use std::{fs, path::Path};

pub fn generate(root: &Path) -> Result<()> {
    corpus::generate(root)?;
    let mut entries = corpus::load(root)?;
    let logos = [
        ("orbit", "<ellipse cx='64' cy='64' rx='51' ry='22' fill='none' stroke='#183060' stroke-width='8' transform='rotate(-25 64 64)'/><circle cx='64' cy='64' r='20' fill='#d06820'/>",3),
        ("mountain", "<path fill='#183060' d='M10 105 48 24 72 66 90 40 118 105Z'/><path fill='#20a080' d='M37 48 48 24 62 49 49 43Z'/>",3),
        ("ribbon", "<path fill='#183060' d='M16 20H102L82 50H37L58 78H112L91 108H16L37 78 16 50Z'/>",2),
        ("diamond", "<path fill='#183060' d='M64 8 120 64 64 120 8 64Z'/><path fill='#d06820' d='M64 31 97 64 64 97 31 64Z'/><path fill='white' d='M64 48 80 64 64 80 48 64Z'/>",3),
        ("leaf", "<path fill='#208050' d='M20 104C8 35 57 9 112 16 117 72 79 119 20 104Z'/><path d='M21 103 91 35' stroke='white' stroke-width='7'/>",2),
        ("waves", "<path d='M8 38Q36 6 64 38T120 38M8 65Q36 33 64 65T120 65M8 92Q36 60 64 92T120 92' fill='none' stroke='#183060' stroke-width='10'/>",2),
        ("hexagon", "<path fill='#183060' d='M36 15H92L120 64 92 113H36L8 64Z'/><path fill='#d06820' d='M49 38H79L94 64 79 90H49L34 64Z'/>",3),
        ("bridge", "<path d='M15 107V49Q64-12 113 49V107H92V52Q64 16 36 52V107Z' fill='#183060'/><path d='M8 95H120' stroke='#d06820' stroke-width='9'/>",3),
        ("compass", "<circle cx='64' cy='64' r='48' fill='#183060'/><path fill='white' d='M44 84 55 45 85 28 74 74Z'/><path fill='#d06820' d='M55 45 85 28 64 64Z'/>",3),
        ("interlock", "<path d='M12 20H76V84H12ZM52 44H116V108H52Z' fill='none' stroke='#183060' stroke-width='10'/><path d='M52 54V84H76' fill='none' stroke='#d06820' stroke-width='10'/>",3),
    ];
    let lineart = [
        ("house", "M12 60 64 14 116 60M24 50V112H104V50M52 112V77H76V112"),
        ("bicycle", "M46 89A22 22 0 1 0 2 89A22 22 0 1 0 46 89ZM124 89A22 22 0 1 0 80 89A22 22 0 1 0 124 89ZM24 89 45 48H89L65 89H24M65 89 43 37H31M103 89 85 28H100"),
        ("signature", "M8 92C42 12 74 1 52 61S10 119 31 80 100 18 82 60 62 101 90 74Q107 58 121 68"),
        ("flower", "M64 119V73M64 96Q28 100 22 79Q48 76 64 96M64 83Q88 59 110 71Q96 93 64 83M64 72C-6 66 29 20 58 43C36-8 96-5 81 42C128 11 143 71 88 65C123 106 62 114 64 72Z"),
        ("portrait", "M35 74Q19 22 64 14Q109 22 93 74Q84 99 64 100Q44 99 35 74M38 50H50M78 50H90M64 56 58 72H67M51 84Q64 92 77 84M12 122Q16 102 48 99M80 99Q112 102 116 122"),
        ("maze", "M8 8H120V120H8V31H97V97H31V54H74V74H52M8 8V20M31 120V108M120 54H108"),
        ("cloud", "M28 94C-6 96 0 49 30 53C22 9 84 6 87 44C120 29 139 83 105 94ZM47 105 39 120M72 105 64 120M97 105 89 120"),
        ("music", "M54 88V21L106 10V77M54 39 106 28M54 88A18 12 0 1 0 18 88A18 12 0 1 0 54 88ZM106 77A18 12 0 1 0 70 77A18 12 0 1 0 106 77Z"),
        ("book", "M64 30Q38 7 8 20V106Q38 93 64 116Q90 93 120 106V20Q90 7 64 30V116M21 39Q37 32 51 43M21 60Q37 53 51 64M77 43Q92 32 107 39M77 64Q92 53 107 60"),
        ("circuit", "M8 16H48V48H80V16H120M8 112H48V80H80V112H120M16 48V80H32M112 48V80H96M56 56H72V72H56Z"),
    ];
    let illustrations = [
        ("landscape", "<path fill='#80c0e0' d='M0 0H128V128H0Z'/><circle cx='96' cy='28' r='15' fill='#f0c040'/><path fill='#208060' d='M0 90 40 35 80 94 112 65 128 90V128H0Z'/><path fill='#183060' d='M0 110Q50 75 128 115V128H0Z'/>",4),
        ("rocket", "<path fill='#183060' d='M42 80 22 103 29 65 49 53M86 80 106 103 99 65 79 53'/><path fill='#d04830' d='M44 88Q31 37 64 8Q97 37 84 88Z'/><circle fill='#80c0e0' cx='64' cy='42' r='12'/><path fill='#e0b030' d='M51 91 64 123 77 91Z'/>",5),
        ("tree", "<path fill='#805030' d='M55 68H73V120H55Z'/><circle fill='#208050' cx='41' cy='55' r='28'/><circle fill='#40a060' cx='86' cy='55' r='28'/><circle fill='#70b050' cx='64' cy='30' r='26'/>",5),
        ("cup", "<path fill='none' stroke='#183060' stroke-width='10' d='M96 42C134 27 133 88 99 77'/><path fill='#d06820' d='M19 32H103L94 103H28Z'/><path fill='#183060' d='M15 106H107V115H15Z'/><path fill='none' stroke='#a0c0d0' stroke-width='6' d='M46 24Q35 13 46 4M73 24Q62 13 73 4'/>",4),
        ("envelope", "<rect x='10' y='25' width='108' height='80' rx='8' fill='#e0b030'/><path fill='#d06820' d='M10 105 64 59 118 105Z'/><path fill='#183060' d='M10 25 64 72 118 25Z'/>",4),
        ("fish", "<path fill='#208090' d='M98 64 123 32V96Z'/><ellipse cx='55' cy='64' rx='49' ry='30' fill='#d06820'/><path fill='#e0b030' d='M45 37 70 12 77 43M45 91 70 116 77 85Z'/><circle fill='#183060' cx='27' cy='58' r='5'/>",5),
        ("camera", "<rect x='9' y='32' width='110' height='76' rx='10' fill='#183060'/><path fill='#208090' d='M31 32 41 16H77L87 32Z'/><circle fill='#80c0e0' cx='64' cy='69' r='28'/><circle fill='#183060' cx='64' cy='69' r='16'/><circle fill='#e0b030' cx='105' cy='45' r='5'/>",5),
        ("plant", "<path fill='#805030' d='M36 78H92L82 119H46Z'/><path fill='none' stroke='#208050' stroke-width='8' d='M64 83V16'/><path fill='#40a060' d='M62 55Q7 62 16 19Q57 17 62 55ZM66 72Q119 76 111 35Q70 32 66 72Z'/>",4),
        ("sailboat", "<path fill='#183060' d='M12 89H116L99 113H29Z'/><path fill='#d06820' d='M59 14V83H13Z'/><path fill='#e0b030' d='M67 28 112 83H67Z'/><path fill='none' stroke='#208090' stroke-width='5' d='M0 121Q16 111 32 121T64 121T96 121T128 121'/>",5),
        ("robot", "<rect x='25' y='28' width='78' height='65' rx='10' fill='#208090'/><path fill='#183060' d='M59 8H69V28H59ZM10 46H25V78H10ZM103 46H118V78H103ZM36 93H51V119H36ZM77 93H92V119H77Z'/><circle fill='#e0b030' cx='46' cy='51' r='8'/><circle fill='#e0b030' cx='82' cy='51' r='8'/><path fill='white' d='M42 74H86V82H42Z'/>",4),
    ];
    for (category, scenes) in [
        ("logo", logos.as_slice()),
        ("illustration", illustrations.as_slice()),
    ] {
        for &(name, body, colors) in scenes {
            append(root, &mut entries, category, name, body, colors)?;
        }
    }
    for (name, path) in lineart {
        append(root,&mut entries,"lineart",name,&format!("<path d='{path}' fill='none' stroke='#101010' stroke-width='3' stroke-linejoin='round' stroke-linecap='round'/>"),2)?;
    }
    let source_dir = root.join("sources/scikit-image-v0.25.2");
    let sources: Vec<serde_json::Value> =
        serde_json::from_slice(&fs::read(source_dir.join("manifest.json"))?)?;
    for source in sources {
        let name = source["name"].as_str().ok_or("invalid photo source")?;
        let raster = image::open(source_dir.join(format!("{name}.png")))?
            .thumbnail(384, 384)
            .to_rgba8();
        let image = av_core::RgbaImage::new(
            raster.width(),
            raster.height(),
            raster.pixels().map(|p| p.0).collect(),
        )?;
        let file = format!("images/photo_{name}.png");
        corpus::save_png(&root.join(&file), &image)?;
        entries.push(Entry { id:format!("photo_{name}"),file,category:"photo".into(),source:format!("{}; author {}; license evidence {}",source["url"],source["author"],source["license_source"]),license:source["license"].as_str().ok_or("missing photo license")?.into(), expected_colors:None,notes:"Photographic source retained with official SHA256; image::thumbnail longest side384px for reproducible corpus runtime; not performance reference".into(),ground_truth:None,expected_decode_error:false,corners:vec![] });
    }
    stress_cases(root, &mut entries)?;
    fs::write(
        root.join("manifest.json"),
        serde_json::to_vec_pretty(&entries)?,
    )?;
    println!("expanded complete corpus: {} entries", entries.len());
    Ok(())
}

fn append(
    root: &Path,
    entries: &mut Vec<Entry>,
    category: &str,
    name: &str,
    body: &str,
    colors: u16,
) -> Result<()> {
    let id = format!("{category}_{name}");
    let gt=format!("<svg xmlns='http://www.w3.org/2000/svg' width='128' height='128' viewBox='0 0 128 128'><path fill='white' d='M0 0H128V128H0Z'/>{body}</svg>");
    let ground_truth = format!("gt/{id}.svg");
    let file = format!("images/{id}.png");
    fs::write(root.join(&ground_truth), &gt)?;
    corpus::save_png(&root.join(&file), &av_metrics::render::svg(&gt, true)?)?;
    entries.push(Entry {
        id,
        file,
        category: category.into(),
        source: "Original artwork generated by av-bench gen --full".into(),
        license: "CC0-1.0".into(),
        expected_colors: Some(colors),
        notes: "Original geometry, AA enabled; no copied logos or artwork".into(),
        ground_truth: Some(ground_truth),
        expected_decode_error: false,
        corners: vec![],
    });
    Ok(())
}

fn stress_cases(root: &Path, entries: &mut Vec<Entry>) -> Result<()> {
    let large = av_core::RgbaImage::new(8000, 8000, vec![[32, 64, 128, 255]; 64_000_000])?;
    corpus::save_png(&root.join("images/edge_64mp.png"), &large)?;
    drop(large);
    let image16 = image::ImageBuffer::<image::Rgba<u16>, Vec<u16>>::from_fn(8, 8, |x, _| {
        if x < 4 {
            image::Rgba([65535, 0, 0, 65535])
        } else {
            image::Rgba([0, 32768, 65535, 65535])
        }
    });
    image16.save(root.join("images/edge_png16.png"))?;
    let mut gif = image::codecs::gif::GifEncoder::new(fs::File::create(
        root.join("images/edge_animated.gif"),
    )?);
    for rgba in [[255, 0, 0, 255], [0, 0, 255, 255]] {
        gif.encode_frame(image::Frame::new(image::RgbaImage::from_pixel(
            8,
            8,
            image::Rgba(rgba),
        )))?;
    }
    drop(gif);
    fs::write(
        root.join("images/edge_corrupt.png"),
        b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR",
    )?;
    for (id, file, colors, notes, reject) in [
        (
            "edge_64mp",
            "edge_64mp.png",
            Some(1),
            "8000x8000=64,000,000px, opaque uniform; full-resolution pipeline and coverage",
            false,
        ),
        (
            "edge_png16",
            "edge_png16.png",
            Some(2),
            "RGBA16 PNG; conversion to RGBA8 through shared decoder",
            false,
        ),
        (
            "edge_animated",
            "edge_animated.gif",
            Some(1),
            "Two frames: red then blue; decoder must use only first red frame",
            false,
        ),
        (
            "edge_corrupt",
            "edge_corrupt.png",
            None,
            "Truncated PNG IHDR; expected recoverable decode error, no vector output",
            true,
        ),
    ] {
        entries.push(Entry {
            id: id.into(),
            file: format!("images/{file}"),
            category: "edge".into(),
            source: "Generated by av-bench gen --full".into(),
            license: "CC0-1.0".into(),
            expected_colors: colors,
            notes: notes.into(),
            ground_truth: None,
            expected_decode_error: reject,
            corners: vec![],
        });
    }
    Ok(())
}
