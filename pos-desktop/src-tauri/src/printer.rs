//! Silent receipt printing on ESC/POS thermal printers, plus the cash-drawer kick.
//!
//! The UI draws the receipt on a canvas (so Lao and Thai text print correctly — most thermal
//! printers have no Lao code page) and hands over a 1-bit raster. It is sent as `GS v 0` bands to:
//! * a **network printer** (`host:9100`, raw TCP), or
//! * an **installed printer** by name — Windows spooler in RAW mode (USB printers with a Windows
//!   driver), or CUPS `lp -o raw` on macOS.

use std::time::Duration;

use tokio::{io::AsyncWriteExt, net::TcpStream, time::timeout};

pub const INIT: &[u8] = &[0x1B, 0x40];
/// ESC p m t1 t2 — pulse drawer pin 2.
pub const DRAWER_KICK: &[u8] = &[0x1B, 0x70, 0x00, 0x19, 0xFA];
/// GS V 66 n — feed and partial cut.
pub const CUT: &[u8] = &[0x1D, 0x56, 0x42, 0x03];

/// Build an ESC/POS job from a packed 1-bpp raster (MSB first, 1 = black).
pub fn raster_job(width_bytes: usize, height: usize, bits: &[u8], cut: bool, drawer: bool) -> Result<Vec<u8>, String> {
    if width_bytes == 0 || width_bytes > 128 || height == 0 || height > 20_000 {
        return Err("invalid receipt image size".into());
    }
    if bits.len() != width_bytes * height {
        return Err("receipt image data has the wrong length".into());
    }
    let mut out = Vec::with_capacity(bits.len() + 64);
    out.extend_from_slice(INIT);
    if drawer {
        out.extend_from_slice(DRAWER_KICK);
    }
    // Bands of at most 256 rows: some printers limit one GS v 0 image's height.
    for band in bits.chunks(width_bytes * 256) {
        let rows = band.len() / width_bytes;
        out.extend_from_slice(&[0x1D, 0x76, 0x30, 0x00]);
        out.extend_from_slice(&[(width_bytes & 0xFF) as u8, (width_bytes >> 8) as u8, (rows & 0xFF) as u8, (rows >> 8) as u8]);
        out.extend_from_slice(band);
    }
    out.extend_from_slice(&[0x1B, 0x64, 0x03]); // feed 3 lines
    if cut {
        out.extend_from_slice(CUT);
    }
    Ok(out)
}

pub async fn send_tcp(host: &str, port: u16, data: &[u8]) -> Result<(), String> {
    let addr = format!("{}:{port}", host.trim());
    let mut s = timeout(Duration::from_secs(5), TcpStream::connect(&addr))
        .await
        .map_err(|_| format!("printer {addr} did not answer"))?
        .map_err(|e| format!("printer {addr}: {e}"))?;
    timeout(Duration::from_secs(20), async {
        s.write_all(data).await?;
        s.flush().await?;
        s.shutdown().await
    })
    .await
    .map_err(|_| format!("printer {addr} timed out"))?
    .map_err(|e| format!("printer {addr}: {e}"))
}

/// Installed printers (names usable with [`send_to_printer`]).
pub fn list_printers() -> Vec<String> {
    imp::list()
}

pub fn send_to_printer(name: &str, data: &[u8]) -> Result<(), String> {
    imp::send(name, data)
}

#[cfg(windows)]
mod imp {
    use std::{ffi::c_void, ptr};

    use windows_sys::Win32::{
        Foundation::HANDLE,
        Graphics::Printing::{
            ClosePrinter, EndDocPrinter, EndPagePrinter, EnumPrintersW, OpenPrinterW, StartDocPrinterW, StartPagePrinter, WritePrinter, DOC_INFO_1W,
            PRINTER_ENUM_CONNECTIONS, PRINTER_ENUM_LOCAL, PRINTER_INFO_4W,
        },
    };

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    unsafe fn from_wide(p: *const u16) -> String {
        if p.is_null() {
            return String::new();
        }
        let mut n = 0;
        while *p.add(n) != 0 {
            n += 1;
        }
        String::from_utf16_lossy(std::slice::from_raw_parts(p, n))
    }

    pub fn list() -> Vec<String> {
        unsafe {
            let flags = PRINTER_ENUM_LOCAL | PRINTER_ENUM_CONNECTIONS;
            let (mut needed, mut count) = (0u32, 0u32);
            EnumPrintersW(flags, ptr::null(), 4, ptr::null_mut(), 0, &mut needed, &mut count);
            if needed == 0 {
                return vec![];
            }
            let mut buf = vec![0u8; needed as usize];
            if EnumPrintersW(flags, ptr::null(), 4, buf.as_mut_ptr(), needed, &mut needed, &mut count) == 0 {
                return vec![];
            }
            let infos = std::slice::from_raw_parts(buf.as_ptr() as *const PRINTER_INFO_4W, count as usize);
            infos.iter().map(|i| from_wide(i.pPrinterName)).filter(|s| !s.is_empty()).collect()
        }
    }

    pub fn send(name: &str, data: &[u8]) -> Result<(), String> {
        unsafe {
            let wname = wide(name);
            let mut h: HANDLE = ptr::null_mut();
            if OpenPrinterW(wname.as_ptr(), &mut h, ptr::null()) == 0 {
                return Err(format!("cannot open printer '{name}'"));
            }
            let doc_name = wide("zaokaiy receipt");
            let datatype = wide("RAW");
            let info = DOC_INFO_1W { pDocName: doc_name.as_ptr() as *mut u16, pOutputFile: ptr::null_mut(), pDatatype: datatype.as_ptr() as *mut u16 };
            let result = (|| {
                if StartDocPrinterW(h, 1, &info as *const DOC_INFO_1W as *const _) == 0 {
                    return Err(format!("printer '{name}' refused the job"));
                }
                StartPagePrinter(h);
                let mut written = 0u32;
                let ok = WritePrinter(h, data.as_ptr() as *const c_void, data.len() as u32, &mut written);
                EndPagePrinter(h);
                EndDocPrinter(h);
                if ok == 0 || written as usize != data.len() {
                    return Err(format!("could not send the receipt to '{name}'"));
                }
                Ok(())
            })();
            ClosePrinter(h);
            result
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use std::{
        io::Write,
        process::{Command, Stdio},
    };

    pub fn list() -> Vec<String> {
        // "lpstat -e" prints one destination per line (macOS and CUPS on Linux).
        Command::new("lpstat")
            .arg("-e")
            .output()
            .ok()
            .map(|o| String::from_utf8_lossy(&o.stdout).lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect())
            .unwrap_or_default()
    }

    pub fn send(name: &str, data: &[u8]) -> Result<(), String> {
        let mut child = Command::new("lp")
            .args(["-d", name, "-o", "raw"])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| format!("cannot start lp: {e}"))?;
        child.stdin.take().ok_or("lp has no stdin")?.write_all(data).map_err(|e| e.to_string())?;
        let out = child.wait_with_output().map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(())
        } else {
            Err(format!("printer '{name}': {}", String::from_utf8_lossy(&out.stderr).trim()))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raster_bands() {
        let (w, h) = (48usize, 300usize); // 384 dots wide (58 mm), 300 rows → 2 bands
        let job = raster_job(w, h, &vec![0xAA; w * h], true, true).unwrap();
        assert!(job.starts_with(&[0x1B, 0x40, 0x1B, 0x70]));
        assert!(job.ends_with(CUT));
        let bands = job.windows(4).filter(|x| *x == [0x1D, 0x76, 0x30, 0x00]).count();
        assert_eq!(bands, 2);
        assert_eq!(job.len(), 2 + 5 + 2 * 8 + w * h + 3 + 4);
        assert!(raster_job(w, h, &[0; 10], false, false).is_err());
    }

    #[tokio::test]
    async fn tcp_printer() {
        let l = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = l.local_addr().unwrap().port();
        let srv = tokio::spawn(async move {
            use tokio::io::AsyncReadExt;
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = vec![];
            s.read_to_end(&mut buf).await.unwrap();
            buf
        });
        send_tcp("127.0.0.1", port, DRAWER_KICK).await.unwrap();
        assert_eq!(srv.await.unwrap(), DRAWER_KICK);
        assert!(send_tcp("127.0.0.1", 1, b"x").await.is_err());
    }
}
