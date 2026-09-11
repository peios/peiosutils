//! End-to-end tests exercising the real `mkuki` binary.

use std::fs;
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use tempfile::tempdir;

const MKUKI: &str = env!("CARGO_BIN_EXE_mkuki");
const PE_OFFSET_PTR: usize = 0x3c;
const COFF_HEADER_SIZE: usize = 20;
const SECTION_HEADER_SIZE: usize = 40;

struct KillOnDrop(Child);

impl Drop for KillOnDrop {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn wait_for_payload(path: &Path, payload: &[u8]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if fs::read(path)
            .is_ok_and(|bytes| bytes.windows(payload.len()).any(|window| window == payload))
        {
            return;
        }
        thread::sleep(Duration::from_millis(25));
    }
    panic!(
        "{} did not contain {:?} before the timeout",
        path.display(),
        String::from_utf8_lossy(payload)
    );
}

fn put_u16(buf: &mut [u8], off: usize, value: u16) {
    buf[off..off + 2].copy_from_slice(&value.to_le_bytes());
}

fn put_u32(buf: &mut [u8], off: usize, value: u32) {
    buf[off..off + 4].copy_from_slice(&value.to_le_bytes());
}

fn read_u32(buf: &[u8], off: usize) -> u32 {
    u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
}

fn align(value: usize, align: usize) -> usize {
    value.div_ceil(align) * align
}

fn write_fake_stub(path: &Path) {
    let pe_off = 0x80;
    let coff_off = pe_off + 4;
    let opt_off = coff_off + COFF_HEADER_SIZE;
    let opt_size = 0xf0;
    let section_table_off = opt_off + opt_size;
    let raw_off = align(section_table_off + SECTION_HEADER_SIZE * 4, 0x200);

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

    fs::write(path, image).unwrap();
}

#[test]
fn writes_a_uki_from_boot_inputs() {
    let dir = tempdir().unwrap();
    let stub = dir.path().join("stub.efi");
    let kernel = dir.path().join("vmlinuz");
    let initramfs = dir.path().join("initramfs.cpio.gz");
    let out = dir.path().join("boot/efi/EFI/BOOT/BOOTX64.EFI");

    write_fake_stub(&stub);
    fs::write(&kernel, b"kernel").unwrap();
    fs::write(&initramfs, b"initramfs").unwrap();

    let status = Command::new(MKUKI)
        .arg("--stub")
        .arg(&stub)
        .arg("--kernel")
        .arg(&kernel)
        .arg("--initramfs")
        .arg(&initramfs)
        .arg("--cmdline")
        .arg("console=ttyS0")
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());

    let image = fs::read(out).unwrap();
    assert_eq!(&image[..2], b"MZ");

    let pe_off = read_u32(&image, PE_OFFSET_PTR) as usize;
    let coff_off = pe_off + 4;
    let opt_off = coff_off + COFF_HEADER_SIZE;
    let section_table_off = opt_off + 0xf0;
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
}

#[test]
fn writes_a_uki_with_the_embedded_default_stub() {
    let dir = tempdir().unwrap();
    let kernel = dir.path().join("vmlinuz");
    let initramfs = dir.path().join("initramfs.cpio.gz");
    let out = dir.path().join("BOOTX64.EFI");

    fs::write(&kernel, b"kernel").unwrap();
    fs::write(&initramfs, b"initramfs").unwrap();

    let status = Command::new(MKUKI)
        .arg("--kernel")
        .arg(&kernel)
        .arg("--initramfs")
        .arg(&initramfs)
        .arg("--cmdline")
        .arg("console=ttyS0")
        .arg("--out")
        .arg(&out)
        .status()
        .unwrap();
    assert!(status.success());

    let image = fs::read(out).unwrap();
    assert_eq!(&image[..2], b"MZ");
    assert!(
        image.windows(b".linux".len()).any(|w| w == b".linux"),
        "missing embedded kernel section",
    );
}

#[test]
fn resolves_a_unique_kernel_from_a_release_directory() {
    let dir = tempdir().unwrap();
    let modules = dir.path().join("usr/lib/modules");
    let release = modules.join("7.0.9-peios-0.8.3");
    let kernel = release.join("vmlinuz-7.0.9-peios-0.8.3");
    let initramfs = dir.path().join("initramfs.cpio.zst");
    let out = dir.path().join("BOOTX64.EFI");

    fs::create_dir_all(&release).unwrap();
    fs::write(&kernel, b"unique-directory-kernel").unwrap();
    fs::write(&initramfs, b"initramfs").unwrap();

    let output = Command::new(MKUKI)
        .arg("--kernel-dir")
        .arg(&modules)
        .arg("--initramfs")
        .arg(&initramfs)
        .arg("--cmdline")
        .arg("console=ttyS0")
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "mkuki failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        fs::read(out)
            .unwrap()
            .windows(b"unique-directory-kernel".len())
            .any(|window| window == b"unique-directory-kernel")
    );
}

#[test]
fn refuses_to_guess_between_multiple_directory_kernels() {
    let dir = tempdir().unwrap();
    let modules = dir.path().join("usr/lib/modules");
    let initramfs = dir.path().join("initramfs.cpio.zst");
    let out = dir.path().join("BOOTX64.EFI");
    for version in ["0.8.2", "0.8.3"] {
        let release = modules.join(format!("7.0.9-peios-{version}"));
        fs::create_dir_all(&release).unwrap();
        fs::write(
            release.join(format!("vmlinuz-7.0.9-peios-{version}")),
            b"kernel",
        )
        .unwrap();
    }
    fs::write(&initramfs, b"initramfs").unwrap();

    let output = Command::new(MKUKI)
        .arg("--kernel-dir")
        .arg(&modules)
        .arg("--initramfs")
        .arg(&initramfs)
        .arg("--cmdline")
        .arg("console=ttyS0")
        .arg("--out")
        .arg(&out)
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("2 kernels found"));
    assert!(!out.exists());
}

#[test]
fn watch_adopts_a_replaced_release_directory() {
    let dir = tempdir().unwrap();
    let modules = dir.path().join("usr/lib/modules");
    let old_release = modules.join("7.0.9-peios-0.8.2");
    let new_release = modules.join("7.0.9-peios-0.8.3");
    let initramfs = dir.path().join("initramfs.cpio.zst");
    let out = dir.path().join("boot/efi/EFI/BOOT/BOOTX64.EFI");

    fs::create_dir_all(&old_release).unwrap();
    fs::write(
        old_release.join("vmlinuz-7.0.9-peios-0.8.2"),
        b"old-directory-kernel",
    )
    .unwrap();
    fs::write(&initramfs, b"initramfs").unwrap();

    let child = Command::new(MKUKI)
        .arg("--kernel-dir")
        .arg(&modules)
        .arg("--initramfs")
        .arg(&initramfs)
        .arg("--cmdline")
        .arg("console=ttyS0")
        .arg("--out")
        .arg(&out)
        .arg("--watch")
        .arg("--debounce")
        .arg("1")
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let _watcher = KillOnDrop(child);

    wait_for_payload(&out, b"old-directory-kernel");

    fs::create_dir_all(&new_release).unwrap();
    fs::write(
        new_release.join("vmlinuz-7.0.9-peios-0.8.3"),
        b"new-directory-kernel",
    )
    .unwrap();
    fs::remove_dir_all(&old_release).unwrap();

    wait_for_payload(&out, b"new-directory-kernel");
}

#[test]
fn prints_embedded_stub_info() {
    let output = Command::new(MKUKI).arg("--stub-info").output().unwrap();
    assert!(output.status.success());
    let stdout = String::from_utf8(output.stdout).unwrap();
    assert!(stdout.contains("systemd-stub"));
    assert!(stdout.contains("embedded stub sha256"));
}

#[test]
fn rejects_missing_arguments() {
    assert!(!Command::new(MKUKI).status().unwrap().success());
}
