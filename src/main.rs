use std::io::Read;
use std::process;

fn print_usage() {
    eprintln!("Usage: seqdiagsvg [OPTIONS] <INPUT> <OUTPUT>");
    eprintln!();
    eprintln!("Arguments:");
    eprintln!("  <INPUT>    Input file path (sequence diagram text) or \"-\" for stdin");
    eprintln!("  <OUTPUT>   Output file path (.png or .svg)");
    eprintln!();
    eprintln!("Options:");
    eprintln!("  --format <FMT>       Output format: png or svg (default: auto from extension)");
    eprintln!("  --font-size <PT>     Font size in points (default: 14)");
    eprintln!("  --scale <FACTOR>     Scale factor (default: 2.0)");
    eprintln!("  --fg <RRGGBB[AA]>    Foreground color as hex (default: 333333ff)");
    eprintln!("  --bg <RRGGBB[AA]>    Background color as hex (default: ffffffff)");
    eprintln!("  --padding <PX>       Padding in pixels (default: 16)");
    eprintln!("  --font <PATH>        Additional font file (OTF/TTF) for extra glyph coverage");
    eprintln!("  -h, --help           Print this help");
    eprintln!();
    eprintln!("Examples:");
    eprintln!(r#"  seqdiagsvg diagram.txt diagram.svg"#);
    eprintln!(r#"  echo "Alice->Bob: Hello" | seqdiagsvg - output.png"#);
    eprintln!(r#"  seqdiagsvg --fg 0066cc --font-size 16 input.txt output.svg"#);
    eprintln!(r#"  seqdiagsvg --font /path/to/NotoSansCJK.ttf input.txt output.png"#);
}

fn parse_hex_color(s: &str) -> Result<[u8; 4], String> {
    let s = s.trim_start_matches('#');
    let bytes: Vec<u8> = (0..s.len())
        .step_by(2)
        .map(|i| {
            u8::from_str_radix(s.get(i..i + 2).unwrap_or("00"), 16)
                .map_err(|e| format!("invalid hex color: {e}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    match bytes.len() {
        3 => Ok([bytes[0], bytes[1], bytes[2], 255]),
        4 => Ok([bytes[0], bytes[1], bytes[2], bytes[3]]),
        _ => Err(format!("expected 6 or 8 hex digits, got {}", s.len())),
    }
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.is_empty() || args.iter().any(|a| a == "-h" || a == "--help") {
        print_usage();
        process::exit(if args.is_empty() { 1 } else { 0 });
    }

    let mut opts = seqdiagsvg::RenderOptions::default();
    let mut positional = Vec::new();
    let mut format_override: Option<String> = None;
    let mut i = 0;

    while i < args.len() {
        match args[i].as_str() {
            "--format" => {
                i += 1;
                format_override = Some(args[i].clone());
            }
            "--font-size" => {
                i += 1;
                opts.font_size_pt = args[i]
                    .parse()
                    .unwrap_or_else(|_| { eprintln!("invalid font size"); process::exit(1) });
            }
            "--scale" => {
                i += 1;
                opts.scale = args[i]
                    .parse()
                    .unwrap_or_else(|_| { eprintln!("invalid scale"); process::exit(1) });
            }
            "--fg" => {
                i += 1;
                opts.fg_color = parse_hex_color(&args[i])
                    .unwrap_or_else(|e| { eprintln!("{e}"); process::exit(1) });
            }
            "--bg" => {
                i += 1;
                opts.bg_color = parse_hex_color(&args[i])
                    .unwrap_or_else(|e| { eprintln!("{e}"); process::exit(1) });
            }
            "--padding" => {
                i += 1;
                opts.padding = args[i]
                    .parse()
                    .unwrap_or_else(|_| { eprintln!("invalid padding"); process::exit(1) });
            }
            "--font" => {
                i += 1;
                opts.system_font = Some(args[i].clone());
            }
            arg if arg.starts_with('-') && arg != "-" => {
                eprintln!("unknown option: {arg}");
                process::exit(1);
            }
            _ => {
                positional.push(args[i].clone());
            }
        }
        i += 1;
    }

    if positional.len() != 2 {
        eprintln!("expected exactly 2 positional arguments: <INPUT> <OUTPUT>");
        print_usage();
        process::exit(1);
    }

    let input_path = &positional[0];
    let output_path = &positional[1];

    // Read input
    let input = if input_path == "-" {
        let mut buf = String::new();
        std::io::stdin()
            .read_to_string(&mut buf)
            .unwrap_or_else(|e| {
                eprintln!("failed to read stdin: {e}");
                process::exit(1);
            });
        buf
    } else {
        std::fs::read_to_string(input_path).unwrap_or_else(|e| {
            eprintln!("failed to read {input_path}: {e}");
            process::exit(1);
        })
    };

    // Determine output format
    let use_svg = match format_override.as_deref() {
        Some("svg") => true,
        Some("png") => false,
        Some(other) => {
            eprintln!("unknown format: {other} (expected png or svg)");
            process::exit(1);
        }
        None => output_path.ends_with(".svg"),
    };

    let result: Result<Vec<u8>, _> = if use_svg {
        seqdiagsvg::render_to_svg(&input, Some(opts)).map(|s| s.into_bytes())
    } else {
        seqdiagsvg::render_to_png(&input, Some(opts))
    };

    match result {
        Ok(bytes) => {
            if let Err(e) = std::fs::write(output_path, &bytes) {
                eprintln!("failed to write {output_path}: {e}");
                process::exit(1);
            }
            eprintln!("wrote {output_path}");
        }
        Err(e) => {
            eprintln!("render error: {e}");
            process::exit(1);
        }
    }
}
