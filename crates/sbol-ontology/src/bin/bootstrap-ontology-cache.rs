//! Bootstrap raw upstream ontology files into a local cache.
//!
//! This tool is intentionally opt-in. Normal builds and tests use the compact
//! facts embedded in `sbol-ontology` and never fetch network resources.

use std::env;
use std::ffi::OsString;
use std::fs;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use sbol_ontology::download;
use sha2::{Digest, Sha256};

const DEFAULT_CACHE_ROOT: &str = "target/ontology-cache";
const CACHE_ENV: &str = "SBOL_ONTOLOGY_CACHE";

const SOURCES: &[Source] = &[
    Source {
        name: "EDAM.owl",
        url: "http://edamontology.org/EDAM.owl",
        sha256: "a3ebbd7df78f845854c285389f5e788bb6a4852bb50cc6f7a991843dbc538158",
    },
    Source {
        name: "sbo.owl",
        url: "http://purl.obolibrary.org/obo/sbo.owl",
        sha256: "b6a6671bbaa0936fb9f11f093b07b77cd89ab476c2b55b2e11184e0473634105",
    },
    Source {
        name: "so.owl",
        url: "http://purl.obolibrary.org/obo/so.owl",
        sha256: "28d19f7767d8848ceb9088658079ede9b467f4ee95205220520229a45582c6aa",
    },
    Source {
        name: "go-basic.obo",
        url: "http://purl.obolibrary.org/obo/go/releases/2026-03-25/go-basic.obo",
        sha256: "a77e356737dab39a4f620dce35fc4d6eb531c4b6153af6cacaaa322b49b804bd",
    },
    Source {
        name: "chebi.owl",
        url: "http://purl.obolibrary.org/obo/chebi/254/chebi.owl",
        sha256: "4557df5b668394b371e6e75158ec9f7728424852ffdaa2e6a6975cfc31e461ba",
    },
    Source {
        name: "cl-basic.obo",
        url: "http://purl.obolibrary.org/obo/cl/releases/2026-06-08/cl-basic.obo",
        sha256: "73996c6349283e7a8cbd8183367a1cb810d0077e9c291ae0c72bd31e869f8c1c",
    },
];

struct Source {
    name: &'static str,
    url: &'static str,
    /// Expected SHA-256 of the downloaded bytes. The compact fact snapshot is
    /// generated from these exact inputs, so a mismatch means upstream drifted
    /// and the snapshot must be regenerated against the new content.
    sha256: &'static str,
}

#[derive(Debug)]
struct Options {
    cache_root: PathBuf,
    dry_run: bool,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let Some(options) = Options::parse(env::args_os().skip(1))? else {
        print_usage();
        return Ok(());
    };

    if options.dry_run {
        for source in SOURCES {
            println!(
                "would download {} -> {}",
                source.url,
                options.cache_root.join(source.name).display()
            );
        }
        return Ok(());
    }

    fs::create_dir_all(&options.cache_root).map_err(|error| {
        format!(
            "failed to create cache directory `{}`: {error}",
            options.cache_root.display()
        )
    })?;

    for source in SOURCES {
        let target = options.cache_root.join(source.name);
        println!("downloading {} -> {}", source.url, target.display());
        download(source, &target)?;
        verify_checksum(source, &target)?;
        write_checksum(&target)?;
    }

    println!("ontology cache written to {}", options.cache_root.display());
    Ok(())
}

impl Options {
    fn parse<I>(args: I) -> Result<Option<Self>, String>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut cache_root = env::var_os(CACHE_ENV)
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from(DEFAULT_CACHE_ROOT));
        let mut dry_run = false;

        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            let arg = arg
                .into_string()
                .map_err(|_| "arguments must be valid UTF-8".to_owned())?;
            match arg.as_str() {
                "-h" | "--help" => return Ok(None),
                "-n" | "--dry-run" => dry_run = true,
                "--cache" => {
                    let Some(value) = args.next() else {
                        return Err("missing path after --cache".to_owned());
                    };
                    cache_root = PathBuf::from(value);
                }
                _ if arg.starts_with("--cache=") => {
                    cache_root = PathBuf::from(arg.trim_start_matches("--cache="));
                }
                _ => return Err(format!("unknown argument `{arg}`")),
            }
        }

        Ok(Some(Self {
            cache_root,
            dry_run,
        }))
    }
}

fn download(source: &Source, target: &Path) -> Result<(), String> {
    let bytes = download::fetch(source.url)
        .map_err(|error| format!("failed to download {}: {error}", source.url))?;
    let mut file = File::create(target)
        .map_err(|error| format!("failed to create `{}`: {error}", target.display()))?;
    file.write_all(&bytes)
        .map_err(|error| format!("failed to write `{}`: {error}", target.display()))?;
    file.flush()
        .map_err(|error| format!("failed to flush `{}`: {error}", target.display()))
}

fn verify_checksum(source: &Source, target: &Path) -> Result<(), String> {
    let actual = sha256_hex(target)?;
    if actual != source.sha256 {
        return Err(format!(
            "checksum mismatch for {}: expected {}, downloaded {actual}. Upstream content has drifted; update the `sha256` (and any version/URL) for `{}` in this tool and in data/ontology_sources.tsv, then regenerate the fact snapshot with `cargo run -p sbol-ontology --bin generate-ontology-facts`.",
            source.name, source.sha256, source.name
        ));
    }
    Ok(())
}

fn write_checksum(target: &Path) -> Result<(), String> {
    let checksum = format!("{}  {}\n", sha256_hex(target)?, target.display());
    let checksum_path = checksum_path(target);
    fs::write(&checksum_path, checksum).map_err(|error| {
        format!(
            "failed to write checksum `{}`: {error}",
            checksum_path.display()
        )
    })
}

fn sha256_hex(target: &Path) -> Result<String, String> {
    let mut file = File::open(target)
        .map_err(|error| format!("failed to open `{}`: {error}", target.display()))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 64 * 1024];

    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("failed to read `{}`: {error}", target.display()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }

    Ok(hex_lower(&hasher.finalize()))
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn checksum_path(target: &Path) -> PathBuf {
    let mut value = target.as_os_str().to_os_string();
    value.push(".sha256");
    PathBuf::from(value)
}

fn print_usage() {
    println!(
        "\
Usage: cargo run -p sbol-ontology --bin bootstrap-ontology-cache -- [OPTIONS]

Options:
      --cache <PATH>  Cache directory [env: {CACHE_ENV}, default: {DEFAULT_CACHE_ROOT}]
  -n, --dry-run       Print planned downloads without fetching
  -h, --help          Print help
"
    );
}
