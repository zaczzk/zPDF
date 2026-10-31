// zPDF: Zero-dependency native Rust PDF viewer & editor
// Module: Security (FIPS 180-4 pure-Rust SHA-256 & DLL Search-Order Lockdown)

use std::fs::File;
use std::io::{self, Read};
use std::path::Path;

const LOAD_LIBRARY_SEARCH_SYSTEM32: u32 = 0x00000800;

#[link(name = "kernel32")]
extern "system" {
    fn SetDefaultDllDirectories(directory_flags: u32) -> i32;
    fn SetDllDirectoryW(lp_path_name: *const u16) -> i32;
}

/// Hardens the current process against DLL hijacking and search-order attacks.
/// Forces Windows to only load DLLs from System32 unless an explicit absolute path is provided.
pub fn harden_process_dll_search_order() {
    unsafe {
        // Enforce System32 search order
        let _ = SetDefaultDllDirectories(LOAD_LIBRARY_SEARCH_SYSTEM32);
        // Clear current working directory from DLL search order
        let empty: [u16; 1] = [0];
        let _ = SetDllDirectoryW(empty.as_ptr());
    }
}

/// Pure-Rust FIPS 180-4 SHA-256 state machine without any external crates.
#[derive(Clone)]
pub struct Sha256 {
    state: [u32; 8],
    count: u64,
    buffer: [u8; 64],
}

impl Sha256 {
    const K: [u32; 64] = [
        0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
        0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
        0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
        0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
        0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
        0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
        0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
        0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2,
    ];

    pub fn new() -> Self {
        Self {
            state: [
                0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
                0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19,
            ],
            count: 0,
            buffer: [0u8; 64],
        }
    }

    #[inline(always)]
    fn ch(x: u32, y: u32, z: u32) -> u32 {
        (x & y) ^ (!x & z)
    }

    #[inline(always)]
    fn maj(x: u32, y: u32, z: u32) -> u32 {
        (x & y) ^ (x & z) ^ (y & z)
    }

    #[inline(always)]
    fn sigma0(x: u32) -> u32 {
        x.rotate_right(2) ^ x.rotate_right(13) ^ x.rotate_right(22)
    }

    #[inline(always)]
    fn sigma1(x: u32) -> u32 {
        x.rotate_right(6) ^ x.rotate_right(11) ^ x.rotate_right(25)
    }

    #[inline(always)]
    fn s0(x: u32) -> u32 {
        x.rotate_right(7) ^ x.rotate_right(18) ^ (x >> 3)
    }

    #[inline(always)]
    fn s1(x: u32) -> u32 {
        x.rotate_right(17) ^ x.rotate_right(19) ^ (x >> 10)
    }

    fn transform(&mut self, chunk: &[u8; 64]) {
        let mut w = [0u32; 64];
        for i in 0..16 {
            w[i] = u32::from_be_bytes([
                chunk[i * 4],
                chunk[i * 4 + 1],
                chunk[i * 4 + 2],
                chunk[i * 4 + 3],
            ]);
        }
        for i in 16..64 {
            w[i] = Self::s1(w[i - 2])
                .wrapping_add(w[i - 7])
                .wrapping_add(Self::s0(w[i - 15]))
                .wrapping_add(w[i - 16]);
        }

        let mut a = self.state[0];
        let mut b = self.state[1];
        let mut c = self.state[2];
        let mut d = self.state[3];
        let mut e = self.state[4];
        let mut f = self.state[5];
        let mut g = self.state[6];
        let mut h = self.state[7];

        for i in 0..64 {
            let t1 = h
                .wrapping_add(Self::sigma1(e))
                .wrapping_add(Self::ch(e, f, g))
                .wrapping_add(Self::K[i])
                .wrapping_add(w[i]);
            let t2 = Self::sigma0(a).wrapping_add(Self::maj(a, b, c));
            h = g;
            g = f;
            f = e;
            e = d.wrapping_add(t1);
            d = c;
            c = b;
            b = a;
            a = t1.wrapping_add(t2);
        }

        self.state[0] = self.state[0].wrapping_add(a);
        self.state[1] = self.state[1].wrapping_add(b);
        self.state[2] = self.state[2].wrapping_add(c);
        self.state[3] = self.state[3].wrapping_add(d);
        self.state[4] = self.state[4].wrapping_add(e);
        self.state[5] = self.state[5].wrapping_add(f);
        self.state[6] = self.state[6].wrapping_add(g);
        self.state[7] = self.state[7].wrapping_add(h);
    }

    pub fn update(&mut self, mut input: &[u8]) {
        let mut buffer_idx = (self.count as usize) & 0x3f;
        self.count = self.count.wrapping_add(input.len() as u64);

        if buffer_idx > 0 && buffer_idx + input.len() >= 64 {
            let take = 64 - buffer_idx;
            self.buffer[buffer_idx..64].copy_from_slice(&input[..take]);
            let chunk = self.buffer;
            self.transform(&chunk);
            input = &input[take..];
            buffer_idx = 0;
        }

        while input.len() >= 64 {
            let chunk: &[u8; 64] = input[..64].try_into().unwrap();
            self.transform(chunk);
            input = &input[64..];
        }

        if !input.is_empty() {
            self.buffer[buffer_idx..buffer_idx + input.len()].copy_from_slice(input);
        }
    }

    pub fn finalize(mut self) -> [u8; 32] {
        let bit_len = self.count.wrapping_mul(8);
        let buffer_idx = (self.count as usize) & 0x3f;

        let pad_len = if buffer_idx < 56 {
            56 - buffer_idx
        } else {
            120 - buffer_idx
        };

        let mut padding = [0u8; 64];
        padding[0] = 0x80;
        self.update(&padding[..pad_len]);

        let len_bytes = bit_len.to_be_bytes();
        self.update(&len_bytes);

        let mut digest = [0u8; 32];
        for (i, &word) in self.state.iter().enumerate() {
            digest[i * 4..i * 4 + 4].copy_from_slice(&word.to_be_bytes());
        }
        digest
    }

    pub fn hash(data: &[u8]) -> [u8; 32] {
        let mut hasher = Self::new();
        hasher.update(data);
        hasher.finalize()
    }
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{:02x}", b));
    }
    s
}

/// Hashes an entire file using pure-Rust streaming SHA-256 with an 8 KB buffer.
pub fn hash_file(path: &Path) -> io::Result<[u8; 32]> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hasher.finalize())
}

/// Cryptographically validates the `pdfium.dll` binary prior to calling `LoadLibraryExW`.
/// Prevents untrusted DLL injection or tampering.
pub fn verify_pdfium_integrity(path: &Path, expected_hex: Option<&str>) -> Result<String, String> {
    if !path.exists() {
        return Err(format!("PDFium library file does not exist: {}", path.display()));
    }

    let actual_hash = hash_file(path).map_err(|e| format!("Failed to read/hash PDFium binary: {}", e))?;
    let actual_hex = bytes_to_hex(&actual_hash);

    if let Some(expected) = expected_hex {
        let clean_expected = expected.trim().to_ascii_lowercase();
        if clean_expected != actual_hex {
            return Err(format!(
                "PDFium SHA-256 integrity mismatch!\nExpected: {}\nActual:   {}",
                clean_expected, actual_hex
            ));
        }
    }

    Ok(actual_hex)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sha256_nist_vectors() {
        // NIST test vector: "" (empty)
        assert_eq!(
            bytes_to_hex(&Sha256::hash(b"")),
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        );
        // NIST test vector: "abc"
        assert_eq!(
            bytes_to_hex(&Sha256::hash(b"abc")),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
