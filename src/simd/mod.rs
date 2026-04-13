// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[cfg(target_arch = "x86_64")]
mod compare;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
mod prefix;
#[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
mod scan;

use std::cmp::Ordering;

#[inline]
pub(crate) fn bytes_cmp(a: &[u8], b: &[u8]) -> Ordering {
    #[cfg(target_arch = "x86_64")]
    {
        compare::bytes_cmp_simd(a, b)
    }
    #[cfg(not(target_arch = "x86_64"))]
    {
        a.cmp(b)
    }
}

#[inline]
pub(crate) fn common_prefix_len(a: &[u8], b: &[u8]) -> u16 {
    #[cfg(any(target_arch = "x86_64", target_arch = "aarch64"))]
    {
        prefix::common_prefix_len_simd(a, b)
    }
    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        let min_len = std::cmp::min(a.len(), b.len());
        let mut prefix_len = 0;
        for i in 0..min_len {
            if a[i] == b[i] {
                prefix_len += 1;
            } else {
                break;
            }
        }
        prefix_len
    }
}
