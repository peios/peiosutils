// spell-checker:ignore (libs) mkuki initramfs cmdline uucore uumain COFF initrd
//! mkuki ~ (peiosutils) — build a UEFI unified kernel image from Peios boot inputs.
//!
//! The tool takes a PE/COFF EFI stub and appends UKI payload sections:
//! `.cmdline`, `.linux`, and `.initrd`. The result is a single EFI binary UEFI
//! firmware boots directly. Part of Peios' Dynamic Boot system, alongside mkirf.

use std::error::Error;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::{Arg, ArgAction, Command};
use uucore::error::{UResult, USimpleError};

const PE_OFFSET_PTR: usize = 0x3c;
const COFF_HEADER_SIZE: usize = 20;
const SECTION_HEADER_SIZE: usize = 40;

const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x0000_0040;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const SECTION_CHARACTERISTICS: u32 = IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ;
const DEFAULT_STUB: &[u8] = include_bytes!("../stubs/linuxx64.efi.stub");
const DEFAULT_STUB_SOURCE: &str = include_str!("../stubs/SOURCE.systemd-stub");

struct Config {
    stub: Option<PathBuf>,
    kernel: PathBuf,
    initramfs: PathBuf,
    cmdline: Cmdline,
    out: PathBuf,
}

enum Cmdline {
    Literal(String),
    File(PathBuf),
}

struct PayloadSection {
    name: &'static str,
    data: Vec<u8>,
}

struct PeInfo {
    coff_off: usize,
    section_table_off: usize,
    section_count: u16,
    section_alignment: u32,
    file_alignment: u32,
    size_of_image_off: usize,
    size_of_headers_off: usize,
    size_of_headers: u32,
    first_raw_section_off: usize,
    next_virtual_address: u32,
    next_raw_pointer: u32,
}

#[derive(Clone, Copy)]
struct ExistingSection {
    virtual_address: u32,
    virtual_size: u32,
    raw_pointer: u32,
    raw_size: u32,
}

#[uucore::main]
pub fn uumain(args: impl uucore::Args) -> UResult<()> {
    let matches = match uu_app().try_get_matches_from(args) {
        Ok(m) => m,
        Err(e) => {
            // clap prints help/version (exit 0) and usage errors (exit 2)
            // itself; relay its exit code to the multi-call runtime.
            let code = e.exit_code();
            e.print().ok();
            return if code == 0 {
                Ok(())
            } else {
                Err(USimpleError::new(code, ""))
            };
        }
    };

    if matches.get_flag("stub-info") {
        print_stub_info();
        return Ok(());
    }

    let cfg = Config::from_matches(&matches).map_err(|e| USimpleError::new(2, e))?;
    run(&cfg).map_err(|e| USimpleError::new(1, e.to_string()))
}

pub fn uu_app() -> Command {
    Command::new("mkuki")
        .about("build a UEFI unified kernel image from Peios boot inputs")
        .arg(
            Arg::new("stub")
                .long("stub")
                .value_name("PATH")
                .value_parser(clap::value_parser!(PathBuf))
                .help("PE/COFF EFI stub to append sections to (default: bundled systemd-stub)"),
        )
        .arg(
            Arg::new("kernel")
                .long("kernel")
                .value_name("PATH")
                .required_unless_present("stub-info")
                .value_parser(clap::value_parser!(PathBuf))
                .help("kernel image; becomes the .linux section"),
        )
        .arg(
            Arg::new("initramfs")
                .long("initramfs")
                .value_name("PATH")
                .required_unless_present("stub-info")
                .value_parser(clap::value_parser!(PathBuf))
                .help("initramfs cpio; becomes the .initrd section"),
        )
        .arg(
            Arg::new("cmdline")
                .long("cmdline")
                .value_name("TEXT")
                .conflicts_with("cmdline-file")
                .help("kernel command line; becomes the .cmdline section"),
        )
        .arg(
            Arg::new("cmdline-file")
                .long("cmdline-file")
                .value_name("PATH")
                .value_parser(clap::value_parser!(PathBuf))
                .help("read the kernel command line from PATH"),
        )
        .arg(
            Arg::new("out")
                .long("out")
                .value_name("PATH")
                .required_unless_present("stub-info")
                .value_parser(clap::value_parser!(PathBuf))
                .help("output UKI path"),
        )
        .arg(
            Arg::new("stub-info")
                .long("stub-info")
                .action(ArgAction::SetTrue)
                .help("print the bundled stub's provenance and exit"),
        )
}

impl Config {
    fn from_matches(m: &clap::ArgMatches) -> Result<Self, String> {
        let cmdline = match (
            m.get_one::<String>("cmdline"),
            m.get_one::<PathBuf>("cmdline-file"),
        ) {
            (Some(s), None) => Cmdline::Literal(s.clone()),
            (None, Some(p)) => Cmdline::File(p.clone()),
            (Some(_), Some(_)) => return Err("use only one of --cmdline or --cmdline-file".into()),
            (None, None) => return Err("missing --cmdline TEXT or --cmdline-file PATH".into()),
        };
        Ok(Config {
            stub: m.get_one::<PathBuf>("stub").cloned(),
            kernel: m
                .get_one::<PathBuf>("kernel")
                .cloned()
                .ok_or("missing --kernel PATH")?,
            initramfs: m
                .get_one::<PathBuf>("initramfs")
                .cloned()
                .ok_or("missing --initramfs PATH")?,
            cmdline,
            out: m
                .get_one::<PathBuf>("out")
                .cloned()
                .ok_or("missing --out PATH")?,
        })
    }
}

fn print_stub_info() {
    print!("{DEFAULT_STUB_SOURCE}");
}

fn run(cfg: &Config) -> Result<(), Box<dyn Error>> {
    let stub = match &cfg.stub {
        Some(path) => read_file(path)?,
        None => DEFAULT_STUB.to_vec(),
    };
    let kernel = read_file(&cfg.kernel)?;
    let initramfs = read_file(&cfg.initramfs)?;
    let cmdline = read_cmdline(&cfg.cmdline)?;

    let sections = [
        PayloadSection {
            name: ".cmdline",
            data: cmdline,
        },
        PayloadSection {
            name: ".linux",
            data: kernel,
        },
        PayloadSection {
            name: ".initrd",
            data: initramfs,
        },
    ];

    let image = build_uki(stub, &sections)?;
    if let Some(parent) = cfg.out.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
    }

    let tmp = temp_path(&cfg.out);
    if let Err(e) = fs::write(&tmp, &image).map_err(|e| format!("{}: {e}", tmp.display())) {
        let _ = fs::remove_file(&tmp);
        return Err(e.into());
    }
    fs::rename(&tmp, &cfg.out).map_err(|e| format!("{}: {e}", cfg.out.display()))?;
    eprintln!("mkuki: wrote {} ({} bytes)", cfg.out.display(), image.len());
    Ok(())
}

fn read_file(path: &Path) -> Result<Vec<u8>, Box<dyn Error>> {
    fs::read(path).map_err(|e| format!("{}: {e}", path.display()).into())
}

fn read_cmdline(cmdline: &Cmdline) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut bytes = match cmdline {
        Cmdline::Literal(s) => s.as_bytes().to_vec(),
        Cmdline::File(path) => fs::read(path).map_err(|e| format!("{}: {e}", path.display()))?,
    };

    while matches!(bytes.last(), Some(b'\n' | b'\r')) {
        bytes.pop();
    }
    if bytes.contains(&0) {
        return Err("kernel command line must not contain NUL bytes".into());
    }
    bytes.push(0);
    Ok(bytes)
}

fn build_uki(mut image: Vec<u8>, sections: &[PayloadSection]) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut info = parse_pe(&image)?;
    let added = u16::try_from(sections.len()).map_err(|_| "too many sections to add")?;
    let new_section_count = info
        .section_count
        .checked_add(added)
        .ok_or("too many sections in PE image")?;
    ensure_section_header_room(&mut image, &mut info, new_section_count)?;

    let mut next_va = info.next_virtual_address;
    let mut next_raw = info.next_raw_pointer;
    if image.len() < usize::try_from(next_raw)? {
        image.resize(usize::try_from(next_raw)?, 0);
    }

    for (i, section) in sections.iter().enumerate() {
        if section.data.is_empty() {
            return Err(format!("{} payload is empty", section.name).into());
        }
        let virtual_size = u32::try_from(section.data.len())
            .map_err(|_| format!("{} payload is too large", section.name))?;
        next_va = align_u32(next_va, info.section_alignment)?;
        next_raw = align_u32(next_raw, info.file_alignment)?;
        let raw_size = align_u32(virtual_size, info.file_alignment)?;

        let raw_start = usize::try_from(next_raw)?;
        let raw_end = raw_start
            .checked_add(usize::try_from(raw_size)?)
            .ok_or("PE image too large")?;
        if image.len() < raw_start {
            image.resize(raw_start, 0);
        }
        image.extend_from_slice(&section.data);
        image.resize(raw_end, 0);

        let header_off =
            info.section_table_off + (usize::from(info.section_count) + i) * SECTION_HEADER_SIZE;
        write_section_header(
            &mut image[header_off..header_off + SECTION_HEADER_SIZE],
            section.name,
            virtual_size,
            next_va,
            raw_size,
            next_raw,
        )?;

        next_va = next_va
            .checked_add(virtual_size)
            .ok_or("PE virtual address overflow")?;
        next_raw = next_raw.checked_add(raw_size).ok_or("PE file too large")?;
    }

    write_u16(&mut image, info.coff_off + 2, new_section_count);
    let required_header_end =
        info.section_table_off + usize::from(new_section_count) * SECTION_HEADER_SIZE;
    let required_size_of_headers = align_u32(
        u32::try_from(required_header_end).map_err(|_| "PE headers are too large")?,
        info.file_alignment,
    )?;
    if required_size_of_headers > u32::try_from(info.first_raw_section_off)? {
        return Err("new PE headers would overlap the first section".into());
    }
    write_u32(
        &mut image,
        info.size_of_headers_off,
        info.size_of_headers.max(required_size_of_headers),
    );
    let size_of_image = align_u32(next_va, info.section_alignment)?;
    write_u32(&mut image, info.size_of_image_off, size_of_image);
    Ok(image)
}

fn ensure_section_header_room(
    image: &mut Vec<u8>,
    info: &mut PeInfo,
    new_section_count: u16,
) -> Result<(), Box<dyn Error>> {
    let required_header_end =
        info.section_table_off + usize::from(new_section_count) * SECTION_HEADER_SIZE;
    if required_header_end <= info.first_raw_section_off {
        return Ok(());
    }

    let new_first_raw = usize::try_from(align_u32(
        u32::try_from(required_header_end).map_err(|_| "PE headers are too large")?,
        info.file_alignment,
    )?)?;
    let delta = new_first_raw
        .checked_sub(info.first_raw_section_off)
        .ok_or("PE header expansion underflow")?;
    image.splice(
        info.first_raw_section_off..info.first_raw_section_off,
        vec![0; delta],
    );

    let delta_u32 = u32::try_from(delta).map_err(|_| "PE header expansion is too large")?;
    for i in 0..usize::from(info.section_count) {
        let off = info.section_table_off + i * SECTION_HEADER_SIZE + 20;
        let raw_pointer = read_u32(image, off)?;
        if raw_pointer != 0 && usize::try_from(raw_pointer)? >= info.first_raw_section_off {
            write_u32(
                image,
                off,
                raw_pointer
                    .checked_add(delta_u32)
                    .ok_or("PE raw pointer overflow")?,
            );
        }
    }

    info.first_raw_section_off = new_first_raw;
    info.next_raw_pointer = info
        .next_raw_pointer
        .checked_add(delta_u32)
        .ok_or("PE raw pointer overflow")?;
    Ok(())
}

fn parse_pe(image: &[u8]) -> Result<PeInfo, Box<dyn Error>> {
    if image.len() < PE_OFFSET_PTR + 4 || &image[..2] != b"MZ" {
        return Err("EFI stub is not an MZ executable".into());
    }

    let pe_off = usize::try_from(read_u32(image, PE_OFFSET_PTR)?)?;
    require(image, pe_off, 4, "PE signature")?;
    if &image[pe_off..pe_off + 4] != b"PE\0\0" {
        return Err("EFI stub is missing a PE signature".into());
    }

    let coff_off = pe_off + 4;
    require(image, coff_off, COFF_HEADER_SIZE, "COFF header")?;
    let section_count = read_u16(image, coff_off + 2)?;
    let optional_size = usize::from(read_u16(image, coff_off + 16)?);
    let optional_off = coff_off + COFF_HEADER_SIZE;
    require(image, optional_off, optional_size, "optional header")?;
    if optional_size < 64 {
        return Err("PE optional header is too small".into());
    }

    let magic = read_u16(image, optional_off)?;
    if magic != 0x010b && magic != 0x020b {
        return Err(format!("unsupported PE optional header magic 0x{magic:04x}").into());
    }

    let section_table_off = optional_off + optional_size;
    let section_table_size = usize::from(section_count) * SECTION_HEADER_SIZE;
    require(
        image,
        section_table_off,
        section_table_size,
        "section table",
    )?;

    let section_alignment = read_u32(image, optional_off + 32)?;
    let file_alignment = read_u32(image, optional_off + 36)?;
    let size_of_headers = read_u32(image, optional_off + 60)?;
    if section_alignment == 0 || file_alignment == 0 {
        return Err("PE image has zero alignment".into());
    }

    let sections = read_sections(image, section_table_off, section_count)?;
    let first_raw_section_off = sections
        .iter()
        .filter_map(|s| (s.raw_pointer != 0).then_some(s.raw_pointer))
        .min()
        .ok_or("PE image has no raw sections")?;
    let first_raw_section_off = usize::try_from(first_raw_section_off)?;

    let mut next_virtual_address = 0;
    let mut next_raw_pointer = 0;
    for section in sections {
        let virtual_end = section
            .virtual_address
            .checked_add(section.virtual_size)
            .ok_or("PE virtual address overflow")?;
        next_virtual_address = next_virtual_address.max(virtual_end);
        let raw_end = section
            .raw_pointer
            .checked_add(section.raw_size)
            .ok_or("PE raw pointer overflow")?;
        next_raw_pointer = next_raw_pointer.max(raw_end);
    }

    Ok(PeInfo {
        coff_off,
        section_table_off,
        section_count,
        section_alignment,
        file_alignment,
        size_of_image_off: optional_off + 56,
        size_of_headers_off: optional_off + 60,
        size_of_headers,
        first_raw_section_off,
        next_virtual_address,
        next_raw_pointer,
    })
}

fn read_sections(
    image: &[u8],
    section_table_off: usize,
    section_count: u16,
) -> Result<Vec<ExistingSection>, Box<dyn Error>> {
    let mut sections = Vec::with_capacity(usize::from(section_count));
    for i in 0..usize::from(section_count) {
        let off = section_table_off + i * SECTION_HEADER_SIZE;
        sections.push(ExistingSection {
            virtual_size: read_u32(image, off + 8)?,
            virtual_address: read_u32(image, off + 12)?,
            raw_size: read_u32(image, off + 16)?,
            raw_pointer: read_u32(image, off + 20)?,
        });
    }
    Ok(sections)
}

fn write_section_header(
    dst: &mut [u8],
    name: &str,
    virtual_size: u32,
    virtual_address: u32,
    raw_size: u32,
    raw_pointer: u32,
) -> Result<(), Box<dyn Error>> {
    dst.fill(0);
    let name_bytes = name.as_bytes();
    if name_bytes.len() > 8 {
        return Err(format!("section name `{name}` is longer than 8 bytes").into());
    }
    dst[..name_bytes.len()].copy_from_slice(name_bytes);
    write_u32(dst, 8, virtual_size);
    write_u32(dst, 12, virtual_address);
    write_u32(dst, 16, raw_size);
    write_u32(dst, 20, raw_pointer);
    write_u32(dst, 36, SECTION_CHARACTERISTICS);
    Ok(())
}

fn require(image: &[u8], off: usize, len: usize, what: &str) -> Result<(), Box<dyn Error>> {
    let end = off.checked_add(len).ok_or("PE offset overflow")?;
    if end > image.len() {
        return Err(format!("truncated PE image while reading {what}").into());
    }
    Ok(())
}

fn read_u16(buf: &[u8], off: usize) -> Result<u16, io::Error> {
    let bytes: [u8; 2] = buf
        .get(off..off + 2)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated little-endian u16"))?
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated little-endian u16"))?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_u32(buf: &[u8], off: usize) -> Result<u32, io::Error> {
    let bytes: [u8; 4] = buf
        .get(off..off + 4)
        .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated little-endian u32"))?
        .try_into()
        .map_err(|_| io::Error::new(io::ErrorKind::UnexpectedEof, "truncated little-endian u32"))?;
    Ok(u32::from_le_bytes(bytes))
}

fn write_u16(buf: &mut [u8], off: usize, value: u16) {
    buf[off..off + 2].copy_from_slice(&value.to_le_bytes());
}

fn write_u32(buf: &mut [u8], off: usize, value: u32) {
    buf[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn align_u32(value: u32, align: u32) -> Result<u32, Box<dyn Error>> {
    let add = align.checked_sub(1).ok_or("invalid alignment")?;
    let bumped = value.checked_add(add).ok_or("alignment overflow")?;
    Ok(bumped / align * align)
}

fn temp_path(out: &Path) -> PathBuf {
    let mut name = out.file_name().unwrap_or_default().to_os_string();
    name.push(format!(".mkuki-tmp-{}", std::process::id()));
    out.with_file_name(name)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verifies_clap_config() {
        uu_app().debug_assert();
    }

    fn put_u16(buf: &mut [u8], off: usize, value: u16) {
        buf[off..off + 2].copy_from_slice(&value.to_le_bytes());
    }

    fn put_u32(buf: &mut [u8], off: usize, value: u32) {
        buf[off..off + 4].copy_from_slice(&value.to_le_bytes());
    }

    fn fake_stub(header_room: usize) -> Vec<u8> {
        let pe_off = 0x80;
        let coff_off = pe_off + 4;
        let opt_off = coff_off + COFF_HEADER_SIZE;
        let opt_size = 0xf0;
        let section_table_off = opt_off + opt_size;
        let raw_off = align_usize(section_table_off + SECTION_HEADER_SIZE + header_room, 0x200);

        let mut image = vec![0; raw_off + 0x200];
        image[..2].copy_from_slice(b"MZ");
        put_u32(&mut image, PE_OFFSET_PTR, pe_off as u32);
        image[pe_off..pe_off + 4].copy_from_slice(b"PE\0\0");

        put_u16(&mut image, coff_off, 0x8664);
        put_u16(&mut image, coff_off + 2, 1);
        put_u16(&mut image, coff_off + 16, opt_size as u16);

        put_u16(&mut image, opt_off, 0x020b);
        put_u32(&mut image, opt_off + 32, 0x1000);
        put_u32(&mut image, opt_off + 36, 0x200);
        put_u32(&mut image, opt_off + 56, 0x2000);
        put_u32(&mut image, opt_off + 60, raw_off as u32);

        let section = section_table_off;
        image[section..section + 5].copy_from_slice(b".text");
        put_u32(&mut image, section + 8, 0x100);
        put_u32(&mut image, section + 12, 0x1000);
        put_u32(&mut image, section + 16, 0x200);
        put_u32(&mut image, section + 20, raw_off as u32);
        put_u32(&mut image, section + 36, 0x6000_0020);
        image
    }

    fn fake_stub_without_header_room() -> Vec<u8> {
        let mut image = fake_stub(SECTION_HEADER_SIZE * 3);
        let pe_off = read_u32(&image, PE_OFFSET_PTR).unwrap() as usize;
        let coff_off = pe_off + 4;
        let opt_off = coff_off + COFF_HEADER_SIZE;
        let section_table_off = opt_off + 0xf0;
        let raw_off = section_table_off + SECTION_HEADER_SIZE;
        put_u32(&mut image, opt_off + 60, raw_off as u32);
        put_u32(&mut image, section_table_off + 20, raw_off as u32);
        image
    }

    fn align_usize(value: usize, align: usize) -> usize {
        value.div_ceil(align) * align
    }

    #[test]
    fn appends_expected_uki_sections() {
        let image = build_uki(
            fake_stub(SECTION_HEADER_SIZE * 3),
            &[
                PayloadSection {
                    name: ".cmdline",
                    data: b"console=ttyS0\0".to_vec(),
                },
                PayloadSection {
                    name: ".linux",
                    data: b"kernel".to_vec(),
                },
                PayloadSection {
                    name: ".initrd",
                    data: b"initramfs".to_vec(),
                },
            ],
        )
        .unwrap();

        let pe_off = read_u32(&image, PE_OFFSET_PTR).unwrap() as usize;
        let coff_off = pe_off + 4;
        let opt_off = coff_off + COFF_HEADER_SIZE;
        let section_table_off = opt_off + 0xf0;
        assert_eq!(read_u16(&image, coff_off + 2).unwrap(), 4);
        assert_eq!(
            &image[section_table_off + 40..section_table_off + 48],
            b".cmdline"
        );
        assert_eq!(
            &image[section_table_off + 80..section_table_off + 86],
            b".linux"
        );
        assert_eq!(
            &image[section_table_off + 120..section_table_off + 127],
            b".initrd"
        );
        assert_eq!(read_u32(&image, opt_off + 56).unwrap(), 0x5000);
    }

    #[test]
    fn expands_stub_without_section_header_room() {
        let image = build_uki(
            fake_stub_without_header_room(),
            &[PayloadSection {
                name: ".cmdline",
                data: b"x\0".to_vec(),
            }],
        )
        .unwrap();

        let pe_off = read_u32(&image, PE_OFFSET_PTR).unwrap() as usize;
        let coff_off = pe_off + 4;
        assert_eq!(read_u16(&image, coff_off + 2).unwrap(), 2);
    }

    #[test]
    fn cmdline_gets_nul_terminated_and_trimmed() {
        let cmdline = read_cmdline(&Cmdline::Literal("console=ttyS0\n".into())).unwrap();
        assert_eq!(cmdline, b"console=ttyS0\0");
    }

    #[test]
    fn cmdline_rejects_embedded_nul() {
        assert!(read_cmdline(&Cmdline::Literal("a\0b".into())).is_err());
    }
}
