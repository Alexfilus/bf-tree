// Copyright (c) Microsoft Corporation.
// Licensed under the MIT license.

#[cfg(target_arch = "x86_64")]
mod compare;
mod prefix;
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
    prefix::common_prefix_len_simd(a, b)
}
