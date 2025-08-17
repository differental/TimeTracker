use num::traits::ToBytes;
use sled::IVec;

pub fn ivec_to_u64(v: IVec) -> u64 {
    let slice = v.as_ref();
    let mut bytes = [0u8; 8];
    bytes.copy_from_slice(&slice[0..8]);
    u64::from_ne_bytes(bytes)
}

pub fn to_ivec<T: ToBytes>(n: T) -> IVec
where
    IVec: for<'a> From<&'a T::Bytes>,
{
    // There's gotta be some way to not express this in such an ugly way...
    let bytes = n.to_ne_bytes();
    IVec::from(&bytes)
}
