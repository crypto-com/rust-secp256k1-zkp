// Bitcoin secp256k1 bindings
// Written in 2014 by
//   Dawid Ciężarkiewicz
//   Andrew Poelstra
//
// To the extent possible under law, the author(s) have dedicated all
// copyright and related and neighboring rights to this software to
// the public domain worldwide. This software is distributed without
// any warranty.
//
// You should have received a copy of the CC0 Public Domain Dedication
// along with this software.
// If not, see <http://creativecommons.org/publicdomain/zero/1.0/>.
//

//! # Public and secret keys

#[cfg(any(test, feature = "rand"))] use rand::Rng;
#[cfg(feature = "zeroize")] use zeroize::Zeroize;

use core::{fmt, mem, str};

use super::{from_hex, Secp256k1};
use super::Error::{self, InvalidPublicKey, InvalidSecretKey};
use Signing;
use Verification;
use constants;
use ffi;

/// Secret 256-bit key used as `x` in an ECDSA signature
pub struct SecretKey([u8; constants::SECRET_KEY_SIZE]);
impl_array_newtype!(SecretKey, u8, constants::SECRET_KEY_SIZE);
impl_pretty_debug!(SecretKey);

#[cfg(feature = "zeroize")]
impl Zeroize for SecretKey {
    fn zeroize(&mut self) {
        self.0.zeroize();
    }
}

#[cfg(feature = "zeroize")]
impl Drop for SecretKey {
    fn drop(&mut self) {
        self.zeroize();
    }
}

impl fmt::Display for SecretKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        for ch in &self.0[..] {
            write!(f, "{:02x}", *ch)?;
        }
        Ok(())
    }
}

impl str::FromStr for SecretKey {
    type Err = Error;
    fn from_str(s: &str) -> Result<SecretKey, Error> {
        let mut res = [0; constants::SECRET_KEY_SIZE];
        match from_hex(s, &mut res) {
            Ok(constants::SECRET_KEY_SIZE) => Ok(SecretKey(res)),
            _ => Err(Error::InvalidSecretKey)
        }
    }
}

/// The number 1 encoded as a secret key
pub const ONE_KEY: SecretKey = SecretKey([0, 0, 0, 0, 0, 0, 0, 0,
                                          0, 0, 0, 0, 0, 0, 0, 0,
                                          0, 0, 0, 0, 0, 0, 0, 0,
                                          0, 0, 0, 0, 0, 0, 0, 1]);

/// A Secp256k1 public key, used for verification of signatures
#[derive(Clone, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
#[cfg_attr(not(feature = "zeroize"), derive(Copy))]
pub struct PublicKey(ffi::PublicKey);

impl fmt::Display for PublicKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ser = self.serialize();
        for ch in &ser[..] {
            write!(f, "{:02x}", *ch)?;
        }
        Ok(())
    }
}

impl str::FromStr for PublicKey {
    type Err = Error;
    fn from_str(s: &str) -> Result<PublicKey, Error> {
        let mut res = [0; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE];
        match from_hex(s, &mut res) {
            Ok(constants::PUBLIC_KEY_SIZE) => {
                PublicKey::from_slice(
                    &res[0..constants::PUBLIC_KEY_SIZE]
                )
            }
            Ok(constants::UNCOMPRESSED_PUBLIC_KEY_SIZE) => {
                PublicKey::from_slice(&res)
            }
            _ => Err(Error::InvalidPublicKey)
        }
    }
}

/// A Secp256k1 X-only public key, used for verification of Schnorr signatures
#[derive(Clone, PartialEq, Eq, Debug, PartialOrd, Ord, Hash)]
#[cfg_attr(not(feature = "zeroize"), derive(Copy))]
pub struct XOnlyPublicKey(ffi::XOnlyPublicKey);

impl fmt::Display for XOnlyPublicKey {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let ser = self.serialize();
        for ch in &ser[..] {
            write!(f, "{:02x}", *ch)?;
        }
        Ok(())
    }
}

impl str::FromStr for XOnlyPublicKey {
    type Err = Error;
    fn from_str(s: &str) -> Result<XOnlyPublicKey, Error> {
        let mut res = [0; constants::XONLY_PUBLIC_KEY_SIZE];
        match from_hex(s, &mut res) {
            Ok(constants::XONLY_PUBLIC_KEY_SIZE) => {
                XOnlyPublicKey::from_slice(
                    &res[0..constants::XONLY_PUBLIC_KEY_SIZE]
                )
            }
            _ => Err(Error::InvalidPublicKey)
        }
    }
}

#[cfg(any(test, feature = "rand"))]
pub fn random_32_bytes<R: Rng + ?Sized>(rng: &mut R) -> [u8; 32] {
    let mut ret = [0u8; 32];
    rng.fill_bytes(&mut ret);
    ret
}

impl SecretKey {
    /// Creates a new random secret key. Requires compilation with the "rand" feature.
    #[inline]
    #[cfg(any(test, feature = "rand"))]
    pub fn new<R: Rng + ?Sized>(rng: &mut R) -> SecretKey {
        let mut data = random_32_bytes(rng);
        unsafe {
            while ffi::secp256k1_ec_seckey_verify(
                ffi::secp256k1_context_no_precomp,
                data.as_ptr(),
            ) == 0
            {
                data = random_32_bytes(rng);
            }
        }
        SecretKey(data)
    }

    /// Converts a `SECRET_KEY_SIZE`-byte slice to a secret key
    #[inline]
    pub fn from_slice(data: &[u8])-> Result<SecretKey, Error> {
        match data.len() {
            constants::SECRET_KEY_SIZE => {
                let mut ret = [0; constants::SECRET_KEY_SIZE];
                unsafe {
                    if ffi::secp256k1_ec_seckey_verify(
                        ffi::secp256k1_context_no_precomp,
                        data.as_ptr(),
                    ) == 0
                    {
                        return Err(InvalidSecretKey);
                    }
                }
                ret[..].copy_from_slice(data);
                Ok(SecretKey(ret))
            }
            _ => Err(InvalidSecretKey)
        }
    }

    #[inline]
    /// Adds one secret key to another, modulo the curve order. WIll
    /// return an error if the resulting key would be invalid or if
    /// the tweak was not a 32-byte length slice.
    pub fn add_assign(
        &mut self,
        other: &[u8],
    ) -> Result<(), Error> {
        if other.len() != 32 {
            return Err(Error::InvalidTweak);
        }
        unsafe {
            if ffi::secp256k1_ec_privkey_tweak_add(
                ffi::secp256k1_context_no_precomp,
                self.as_mut_ptr(),
                other.as_ptr(),
            ) != 1
            {
                Err(Error::InvalidTweak)
            } else {
                Ok(())
            }
        }
    }

    #[inline]
    /// Multiplies one secret key by another, modulo the curve order. Will
    /// return an error if the resulting key would be invalid or if
    /// the tweak was not a 32-byte length slice.
    pub fn mul_assign(
        &mut self,
        other: &[u8],
    ) -> Result<(), Error> {
        if other.len() != 32 {
            return Err(Error::InvalidTweak);
        }
        unsafe {
            if ffi::secp256k1_ec_privkey_tweak_mul(
                ffi::secp256k1_context_no_precomp,
                self.as_mut_ptr(),
                other.as_ptr(),
            ) != 1
            {
                Err(Error::InvalidTweak)
            } else {
                Ok(())
            }
        }
    }
}

serde_impl!(SecretKey, constants::SECRET_KEY_SIZE);

impl PublicKey {
    /// Obtains a raw const pointer suitable for use with FFI functions
    #[inline]
    pub fn as_ptr(&self) -> *const ffi::PublicKey {
        &self.0 as *const _
    }

    /// Obtains a raw mutable pointer suitable for use with FFI functions
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut ffi::PublicKey {
        &mut self.0 as *mut _
    }

    /// Creates a new public key from a secret key.
    #[inline]
    pub fn from_secret_key<C: Signing>(secp: &Secp256k1<C>,
                           sk: &SecretKey)
                           -> PublicKey {
        let mut pk = unsafe { ffi::PublicKey::blank() };
        unsafe {
            // We can assume the return value because it's not possible to construct
            // an invalid `SecretKey` without transmute trickery or something
            let res = ffi::secp256k1_ec_pubkey_create(secp.ctx, &mut pk, sk.as_ptr());
            debug_assert_eq!(res, 1);
        }
        PublicKey(pk)
    }

    /// Creates a public key directly from a slice
    #[inline]
    pub fn from_slice(data: &[u8]) -> Result<PublicKey, Error> {
        let mut pk = unsafe { ffi::PublicKey::blank() };
        unsafe {
            if ffi::secp256k1_ec_pubkey_parse(
                ffi::secp256k1_context_no_precomp,
                &mut pk,
                data.as_ptr(),
                data.len() as usize,
            ) == 1
            {
                Ok(PublicKey(pk))
            } else {
                Err(InvalidPublicKey)
            }
        }
    }

    #[inline]
    /// Serialize the key as a byte-encoded pair of values. In compressed form
    /// the y-coordinate is represented by only a single bit, as x determines
    /// it up to one bit.
    pub fn serialize(&self) -> [u8; constants::PUBLIC_KEY_SIZE] {
        let mut ret = [0; constants::PUBLIC_KEY_SIZE];

        unsafe {
            let mut ret_len = constants::PUBLIC_KEY_SIZE as usize;
            let err = ffi::secp256k1_ec_pubkey_serialize(
                ffi::secp256k1_context_no_precomp,
                ret.as_mut_ptr(),
                &mut ret_len,
                self.as_ptr(),
                ffi::SECP256K1_SER_COMPRESSED,
            );
            debug_assert_eq!(err, 1);
            debug_assert_eq!(ret_len, ret.len());
        }
        ret
    }

    /// Serialize the key as a byte-encoded pair of values, in uncompressed form
    pub fn serialize_uncompressed(&self) -> [u8; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE] {
        let mut ret = [0; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE];

        unsafe {
            let mut ret_len = constants::UNCOMPRESSED_PUBLIC_KEY_SIZE as usize;
            let err = ffi::secp256k1_ec_pubkey_serialize(
                ffi::secp256k1_context_no_precomp,
                ret.as_mut_ptr(),
                &mut ret_len,
                self.as_ptr(),
                ffi::SECP256K1_SER_UNCOMPRESSED,
            );
            debug_assert_eq!(err, 1);
            debug_assert_eq!(ret_len, ret.len());
        }
        ret
    }

    #[inline]
    /// Adds the pk corresponding to `other` to the pk `self` in place
    /// Will return an error if the resulting key would be invalid or
    /// if the tweak was not a 32-byte length slice.
    pub fn add_exp_assign<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        other: &[u8]
    ) -> Result<(), Error> {
        if other.len() != 32 {
            return Err(Error::InvalidTweak);
        }
        unsafe {
            if ffi::secp256k1_ec_pubkey_tweak_add(secp.ctx, &mut self.0 as *mut _,
                                                  other.as_ptr()) == 1 {
                Ok(())
            } else {
                Err(Error::InvalidTweak)
            }
        }
    }

    #[inline]
    /// Muliplies the pk `self` in place by the scalar `other`
    /// Will return an error if the resulting key would be invalid or
    /// if the tweak was not a 32-byte length slice.
    pub fn mul_assign<C: Verification>(
        &mut self,
        secp: &Secp256k1<C>,
        other: &[u8],
    ) -> Result<(), Error> {
        if other.len() != 32 {
            return Err(Error::InvalidTweak);
        }
        unsafe {
            if ffi::secp256k1_ec_pubkey_tweak_mul(secp.ctx, &mut self.0 as *mut _,
                                                  other.as_ptr()) == 1 {
                Ok(())
            } else {
                Err(Error::InvalidTweak)
            }
        }
    }

    /// Adds a second key to this one, returning the sum. Returns an error if
    /// the result would be the point at infinity, i.e. we are adding this point
    /// to its own negation
    pub fn combine(&self, other: &PublicKey) -> Result<PublicKey, Error> {
        unsafe {
            let mut ret = mem::uninitialized();
            let ptrs = [self.as_ptr(), other.as_ptr()];
            if ffi::secp256k1_ec_pubkey_combine(
                ffi::secp256k1_context_no_precomp,
                &mut ret,
                ptrs.as_ptr(),
                2
            ) == 1
            {
                Ok(PublicKey(ret))
            } else {
                Err(InvalidPublicKey)
            }
        }
    }
}

/// Creates a new public key from a FFI public key
impl From<ffi::PublicKey> for PublicKey {
    #[inline]
    fn from(pk: ffi::PublicKey) -> PublicKey {
        PublicKey(pk)
    }
}

#[cfg(feature = "serde")]
impl ::serde::Serialize for PublicKey {
    fn serialize<S: ::serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_bytes(&self.serialize())
    }
}

#[cfg(feature = "serde")]
impl<'de> ::serde::Deserialize<'de> for PublicKey {
    fn deserialize<D: ::serde::Deserializer<'de>>(d: D) -> Result<PublicKey, D::Error> {
        use ::serde::de::Error;

        let sl: &[u8] = ::serde::Deserialize::deserialize(d)?;
        PublicKey::from_slice(sl).map_err(D::Error::custom)
    }
}

/// Denotes if the point encoded by `xonly_pubkey` is the negation of `pubkey`
pub type IsNegated = bool;

impl XOnlyPublicKey {

    /// Obtains a raw const pointer suitable for use with FFI functions
    #[inline]
    pub fn as_ptr(&self) -> *const ffi::XOnlyPublicKey {
        &self.0 as *const _
    }

    /// Obtains a raw mutable pointer suitable for use with FFI functions
    #[inline]
    pub fn as_mut_ptr(&mut self) -> *mut ffi::XOnlyPublicKey {
        &mut self.0 as *mut _
    }

    /// Creates a new x-only public key from a secret key.
    #[inline]
    pub fn from_secret_key<C: Signing>(secp: &Secp256k1<C>,
                           sk: &SecretKey)
                           -> XOnlyPublicKey {
        let mut pk = unsafe { ffi::XOnlyPublicKey::blank() };
        unsafe {
            // We can assume the return value because it's not possible to construct
            // an invalid `SecretKey` without transmute trickery or something
            let res = ffi::secp256k1_xonly_pubkey_create(secp.ctx, &mut pk, sk.as_ptr());
            debug_assert_eq!(res, 1);
        }
        XOnlyPublicKey(pk)
    }

    /// Serialize an xonly_pubkey object into a 32-byte sequence.
    #[inline]
    pub fn serialize(&self) -> [u8; constants::XONLY_PUBLIC_KEY_SIZE] {
        let mut ret = [0; constants::XONLY_PUBLIC_KEY_SIZE];

        unsafe {
            let err = ffi::secp256k1_xonly_pubkey_serialize(
                ffi::secp256k1_context_no_precomp,
                ret.as_mut_ptr(),
                self.as_ptr(),
            );
            debug_assert_eq!(err, 1);
        }
        ret
    }

    /// Creates a x-only public key directly from a slice
    #[inline]
    pub fn from_slice(data: &[u8]) -> Result<XOnlyPublicKey, Error> {
        let mut pk = unsafe { ffi::XOnlyPublicKey::blank() };
        if data.len() != constants::XONLY_PUBLIC_KEY_SIZE {
            Err(InvalidPublicKey)
        } else {
            unsafe {
                if ffi::secp256k1_xonly_pubkey_parse(
                    ffi::secp256k1_context_no_precomp,
                    &mut pk,
                    data.as_ptr()
                ) == 1 {
                    Ok(XOnlyPublicKey(pk))
                } else {
                    Err(InvalidPublicKey)
                }
            }
        }
    }

    /// Creates a x-only public key from a public key
    #[inline]
    pub fn from_pubkey(pk: &PublicKey) -> (XOnlyPublicKey, IsNegated) {
        let mut ret = unsafe { ffi::XOnlyPublicKey::blank() };
        let mut is_negated: i32 = 0;
        unsafe {
            let err = ffi::secp256k1_xonly_pubkey_from_pubkey(
                ffi::secp256k1_context_no_precomp,
                &mut ret,
                &mut is_negated,
                pk.as_ptr(),
            );
            debug_assert_eq!(err, 1);
        }
        (XOnlyPublicKey(ret), is_negated == 1)
    }

}

/// Creates a new x-only public key from a FFI x-only public key
impl From<ffi::XOnlyPublicKey> for XOnlyPublicKey {
    #[inline]
    fn from(pk: ffi::XOnlyPublicKey) -> XOnlyPublicKey {
        XOnlyPublicKey(pk)
    }
}


/// MuSig: Public key hash
pub struct PublicKeyHash([u8; constants::PK_HASH_SIZE]);
impl_array_newtype!(PublicKeyHash, u8, constants::PK_HASH_SIZE);
impl_pretty_debug!(PublicKeyHash);

impl PublicKeyHash {
    /// Serializes public key hash into array
    #[inline]
    pub fn serialize(&self) -> [u8; constants::PK_HASH_SIZE] {
        self.0
    }

    /// Deserializes array into public key hash
    #[inline]
    pub fn deserialize_from(array: [u8; constants::PK_HASH_SIZE]) -> Self {
        Self(array)
    }
}

/// MuSig: Computes a combined public key and the hash of the given public keys
#[cfg(feature = "musig")]
pub fn pubkey_combine<C: Verification>(secp: &Secp256k1<C>, 
                                       pubkeys: &[PublicKey])
                                       -> Result<(PublicKey, PublicKeyHash), Error> {
    let mut pk_hash32 = [0u8; constants::PK_HASH_SIZE];
    let pks: Vec<ffi::PublicKey> = pubkeys.iter().map(|x| x.0.clone()).collect();
    // currently uses an inefficient algorithm; TODO: utilize scratch space
    unsafe {
        let mut combined_pk = ffi::PublicKey::blank();
        let res = ffi::secp256k1_musig_pubkey_combine(
            secp.ctx,
            std::ptr::null_mut(),
            &mut combined_pk,
            pk_hash32.as_mut_ptr(),
            pks.as_ptr(),
            pks.len());
        if res == 1 {
            Ok((PublicKey::from(combined_pk), PublicKeyHash(pk_hash32)))
        } else {
            Err(InvalidPublicKey)
        }
    }       
}

#[cfg(test)]
mod test {
    use Secp256k1;
    use from_hex;
    use super::super::Error::{InvalidPublicKey, InvalidSecretKey};
    use super::{PublicKey, SecretKey};
    use super::super::constants;

    use rand::{Error, RngCore, thread_rng};
    use rand_core::impls;
    use std::iter;
    use std::str::FromStr;

    macro_rules! hex {
        ($hex:expr) => ({
            let mut result = vec![0; $hex.len() / 2];
            from_hex($hex, &mut result).expect("valid hex string");
            result
        });
    }

    #[test]
    fn skey_from_slice() {
        let sk = SecretKey::from_slice(&[1; 31]);
        assert_eq!(sk, Err(InvalidSecretKey));

        let sk = SecretKey::from_slice(&[1; 32]);
        assert!(sk.is_ok());
    }

    #[test]
    fn pubkey_from_slice() {
        assert_eq!(PublicKey::from_slice(&[]), Err(InvalidPublicKey));
        assert_eq!(PublicKey::from_slice(&[1, 2, 3]), Err(InvalidPublicKey));

        let uncompressed = PublicKey::from_slice(&[4, 54, 57, 149, 239, 162, 148, 175, 246, 254, 239, 75, 154, 152, 10, 82, 234, 224, 85, 220, 40, 100, 57, 121, 30, 162, 94, 156, 135, 67, 74, 49, 179, 57, 236, 53, 162, 124, 149, 144, 168, 77, 74, 30, 72, 211, 229, 110, 111, 55, 96, 193, 86, 227, 183, 152, 195, 155, 51, 247, 123, 113, 60, 228, 188]);
        assert!(uncompressed.is_ok());

        let compressed = PublicKey::from_slice(&[3, 23, 183, 225, 206, 31, 159, 148, 195, 42, 67, 115, 146, 41, 248, 140, 11, 3, 51, 41, 111, 180, 110, 143, 114, 134, 88, 73, 198, 174, 52, 184, 78]);
        assert!(compressed.is_ok());
    }

    #[test]
    fn keypair_slice_round_trip() {
        let s = Secp256k1::new();

        let (sk1, pk1) = s.generate_keypair(&mut thread_rng());
        assert_eq!(SecretKey::from_slice(&sk1[..]), Ok(sk1));
        assert_eq!(PublicKey::from_slice(&pk1.serialize()[..]), Ok(pk1));
        assert_eq!(PublicKey::from_slice(&pk1.serialize_uncompressed()[..]), Ok(pk1));
    }

    #[test]
    fn invalid_secret_key() {
        // Zero
        assert_eq!(SecretKey::from_slice(&[0; 32]), Err(InvalidSecretKey));
        // -1
        assert_eq!(SecretKey::from_slice(&[0xff; 32]), Err(InvalidSecretKey));
        // Top of range
        assert!(SecretKey::from_slice(&[
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
            0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B,
            0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x40,
        ]).is_ok());
        // One past top of range
        assert!(SecretKey::from_slice(&[
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
            0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFE,
            0xBA, 0xAE, 0xDC, 0xE6, 0xAF, 0x48, 0xA0, 0x3B,
            0xBF, 0xD2, 0x5E, 0x8C, 0xD0, 0x36, 0x41, 0x41,
        ]).is_err());
    }

    #[test]
    fn test_out_of_range() {

        struct BadRng(u8);
        impl RngCore for BadRng {
            fn next_u32(&mut self) -> u32 { unimplemented!() }
            fn next_u64(&mut self) -> u64 { unimplemented!() }
            // This will set a secret key to a little over the
            // group order, then decrement with repeated calls
            // until it returns a valid key
            fn fill_bytes(&mut self, data: &mut [u8]) {
                let group_order: [u8; 32] = [
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
                    0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xfe,
                    0xba, 0xae, 0xdc, 0xe6, 0xaf, 0x48, 0xa0, 0x3b,
                    0xbf, 0xd2, 0x5e, 0x8c, 0xd0, 0x36, 0x41, 0x41];
                assert_eq!(data.len(), 32);
                data.copy_from_slice(&group_order[..]);
                data[31] = self.0;
                self.0 -= 1;
            }
            fn try_fill_bytes(&mut self, dest: &mut [u8]) -> Result<(), Error> {
                Ok(self.fill_bytes(dest))
            }
        }

        let s = Secp256k1::new();
        s.generate_keypair(&mut BadRng(0xff));
    }

    #[test]
    fn test_pubkey_from_bad_slice() {
        // Bad sizes
        assert_eq!(
            PublicKey::from_slice(&[0; constants::PUBLIC_KEY_SIZE - 1]),
            Err(InvalidPublicKey)
        );
        assert_eq!(
            PublicKey::from_slice(&[0; constants::PUBLIC_KEY_SIZE + 1]),
            Err(InvalidPublicKey)
        );
        assert_eq!(
            PublicKey::from_slice(&[0; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE - 1]),
            Err(InvalidPublicKey)
        );
        assert_eq!(
            PublicKey::from_slice(&[0; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE + 1]),
            Err(InvalidPublicKey)
        );

        // Bad parse
        assert_eq!(
            PublicKey::from_slice(&[0xff; constants::UNCOMPRESSED_PUBLIC_KEY_SIZE]),
            Err(InvalidPublicKey)
        );
        assert_eq!(
            PublicKey::from_slice(&[0x55; constants::PUBLIC_KEY_SIZE]),
            Err(InvalidPublicKey)
        );
    }

    #[test]
    fn test_debug_output() {
        struct DumbRng(u32);
        impl RngCore for DumbRng {
            fn next_u32(&mut self) -> u32 {
                self.0 = self.0.wrapping_add(1);
                self.0
            }
            fn next_u64(&mut self) -> u64 {
                self.next_u32() as u64
            }
            fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), Error> {
                Err(Error::new("not implemented"))
            }

            fn fill_bytes(&mut self, dest: &mut [u8]) {
                impls::fill_bytes_via_next(self, dest);
            }
        }

        let s = Secp256k1::new();
        let (sk, _) = s.generate_keypair(&mut DumbRng(0));

        assert_eq!(&format!("{:?}", sk),
                   "SecretKey(0100000000000000020000000000000003000000000000000400000000000000)");
    }

    #[test]
    fn test_display_output() {
        static SK_BYTES: [u8; 32] = [
            0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01, 0x01,
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07,
            0xff, 0xff, 0x00, 0x00, 0xff, 0xff, 0x00, 0x00,
            0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63, 0x63,
        ];

        let s = Secp256k1::signing_only();
        let sk = SecretKey::from_slice(&SK_BYTES).expect("sk");
        let pk = PublicKey::from_secret_key(&s, &sk);

        assert_eq!(
            sk.to_string(),
            "01010101010101010001020304050607ffff0000ffff00006363636363636363"
        );
        assert_eq!(
            SecretKey::from_str("01010101010101010001020304050607ffff0000ffff00006363636363636363").unwrap(),
            sk
        );
        assert_eq!(
            pk.to_string(),
            "0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166"
        );
        assert_eq!(
            PublicKey::from_str("0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166").unwrap(),
            pk
        );
        assert_eq!(
            PublicKey::from_str("04\
                18845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd166\
                84B84DB303A340CD7D6823EE88174747D12A67D2F8F2F9BA40846EE5EE7A44F6"
            ).unwrap(),
            pk
        );

        assert!(SecretKey::from_str("fffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff").is_err());
        assert!(SecretKey::from_str("01010101010101010001020304050607ffff0000ffff0000636363636363636363").is_err());
        assert!(SecretKey::from_str("01010101010101010001020304050607ffff0000ffff0000636363636363636").is_err());
        assert!(SecretKey::from_str("01010101010101010001020304050607ffff0000ffff000063636363636363").is_err());
        assert!(SecretKey::from_str("01010101010101010001020304050607ffff0000ffff000063636363636363xx").is_err());
        assert!(PublicKey::from_str("0300000000000000000000000000000000000000000000000000000000000000000").is_err());
        assert!(PublicKey::from_str("0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd16601").is_err());
        assert!(PublicKey::from_str("0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd16").is_err());
        assert!(PublicKey::from_str("0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd1").is_err());
        assert!(PublicKey::from_str("xx0218845781f631c48f1c9709e23092067d06837f30aa0cd0544ac887fe91ddd1").is_err());

        let long_str: String = iter::repeat('a').take(1024 * 1024).collect();
        assert!(SecretKey::from_str(&long_str).is_err());
        assert!(PublicKey::from_str(&long_str).is_err());
    }

    #[test]
    fn test_pubkey_serialize() {
        struct DumbRng(u32);
        impl RngCore for DumbRng {
            fn next_u32(&mut self) -> u32 {
                self.0 = self.0.wrapping_add(1);
                self.0
            }
            fn next_u64(&mut self) -> u64 {
                self.next_u32() as u64
            }
            fn try_fill_bytes(&mut self, _dest: &mut [u8]) -> Result<(), Error> {
                Err(Error::new("not implemented"))
            }

            fn fill_bytes(&mut self, dest: &mut [u8]) {
                impls::fill_bytes_via_next(self, dest);
            }
        }

        let s = Secp256k1::new();
        let (_, pk1) = s.generate_keypair(&mut DumbRng(0));
        assert_eq!(&pk1.serialize_uncompressed()[..],
                   &[4, 124, 121, 49, 14, 253, 63, 197, 50, 39, 194, 107, 17, 193, 219, 108, 154, 126, 9, 181, 248, 2, 12, 149, 233, 198, 71, 149, 134, 250, 184, 154, 229, 185, 28, 165, 110, 27, 3, 162, 126, 238, 167, 157, 242, 221, 76, 251, 237, 34, 231, 72, 39, 245, 3, 191, 64, 111, 170, 117, 103, 82, 28, 102, 163][..]);
        assert_eq!(&pk1.serialize()[..],
                   &[3, 124, 121, 49, 14, 253, 63, 197, 50, 39, 194, 107, 17, 193, 219, 108, 154, 126, 9, 181, 248, 2, 12, 149, 233, 198, 71, 149, 134, 250, 184, 154, 229][..]);
    }

    #[test]
    fn test_addition() {
        let s = Secp256k1::new();

        let (mut sk1, mut pk1) = s.generate_keypair(&mut thread_rng());
        let (mut sk2, mut pk2) = s.generate_keypair(&mut thread_rng());

        assert_eq!(PublicKey::from_secret_key(&s, &sk1), pk1);
        assert!(sk1.add_assign(&sk2[..]).is_ok());
        assert!(pk1.add_exp_assign(&s, &sk2[..]).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk1), pk1);

        assert_eq!(PublicKey::from_secret_key(&s, &sk2), pk2);
        assert!(sk2.add_assign(&sk1[..]).is_ok());
        assert!(pk2.add_exp_assign(&s, &sk1[..]).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk2), pk2);
    }

    #[test]
    fn test_multiplication() {
        let s = Secp256k1::new();

        let (mut sk1, mut pk1) = s.generate_keypair(&mut thread_rng());
        let (mut sk2, mut pk2) = s.generate_keypair(&mut thread_rng());

        assert_eq!(PublicKey::from_secret_key(&s, &sk1), pk1);
        assert!(sk1.mul_assign(&sk2[..]).is_ok());
        assert!(pk1.mul_assign(&s, &sk2[..]).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk1), pk1);

        assert_eq!(PublicKey::from_secret_key(&s, &sk2), pk2);
        assert!(sk2.mul_assign(&sk1[..]).is_ok());
        assert!(pk2.mul_assign(&s, &sk1[..]).is_ok());
        assert_eq!(PublicKey::from_secret_key(&s, &sk2), pk2);
    }

    #[test]
    fn pubkey_hash() {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};
        use std::collections::HashSet;

        fn hash<T: Hash>(t: &T) -> u64 {
            let mut s = DefaultHasher::new();
            t.hash(&mut s);
            s.finish()
        }

        let s = Secp256k1::new();
        let mut set = HashSet::new();
        const COUNT : usize = 1024;
        let count = (0..COUNT).map(|_| {
            let (_, pk) = s.generate_keypair(&mut thread_rng());
            let hash = hash(&pk);
            assert!(!set.contains(&hash));
            set.insert(hash);
        }).count();
        assert_eq!(count, COUNT);
    }

    #[test]
    fn pubkey_combine() {
        let compressed1 = PublicKey::from_slice(
            &hex!("0241cc121c419921942add6db6482fb36243faf83317c866d2a28d8c6d7089f7ba"),
        ).unwrap();
        let compressed2 = PublicKey::from_slice(
            &hex!("02e6642fd69bd211f93f7f1f36ca51a26a5290eb2dd1b0d8279a87bb0d480c8443"),
        ).unwrap();
        let exp_sum = PublicKey::from_slice(
            &hex!("0384526253c27c7aef56c7b71a5cd25bebb66dddda437826defc5b2568bde81f07"),
        ).unwrap();

        let sum1 = compressed1.combine(&compressed2);
        assert!(sum1.is_ok());
        let sum2 = compressed2.combine(&compressed1);
        assert!(sum2.is_ok());
        assert_eq!(sum1, sum2);
        assert_eq!(sum1.unwrap(), exp_sum);
    }

    #[test]
    fn pubkey_equal() {
        let pk1 = PublicKey::from_slice(
            &hex!("0241cc121c419921942add6db6482fb36243faf83317c866d2a28d8c6d7089f7ba"),
        ).unwrap();
        let pk2 = pk1.clone();
        let pk3 = PublicKey::from_slice(
            &hex!("02e6642fd69bd211f93f7f1f36ca51a26a5290eb2dd1b0d8279a87bb0d480c8443"),
        ).unwrap();

        assert!(pk1 == pk2);
        assert!(pk1 <= pk2);
        assert!(pk2 <= pk1);
        assert!(!(pk2 < pk1));
        assert!(!(pk1 < pk2));

        assert!(pk3 < pk1);
        assert!(pk1 > pk3);
        assert!(pk3 <= pk1);
        assert!(pk1 >= pk3);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn test_signature_serde() {
        use serde_test::{Configure, Token, assert_tokens};
        static SK_BYTES: [u8; 32] = [
            1, 1, 1, 1, 1, 1, 1, 1,
            0, 1, 2, 3, 4, 5, 6, 7,
            0xff, 0xff, 0, 0, 0xff, 0xff, 0, 0,
            99, 99, 99, 99, 99, 99, 99, 99
        ];
        static SK_STR: &'static str = "\
            01010101010101010001020304050607ffff0000ffff00006363636363636363\
        ";
        static PK_BYTES: [u8; 33] = [
            0x02,
            0x18, 0x84, 0x57, 0x81, 0xf6, 0x31, 0xc4, 0x8f,
            0x1c, 0x97, 0x09, 0xe2, 0x30, 0x92, 0x06, 0x7d,
            0x06, 0x83, 0x7f, 0x30, 0xaa, 0x0c, 0xd0, 0x54,
            0x4a, 0xc8, 0x87, 0xfe, 0x91, 0xdd, 0xd1, 0x66,
        ];

        let s = Secp256k1::new();

        let sk = SecretKey::from_slice(&SK_BYTES).unwrap();
        let pk = PublicKey::from_secret_key(&s, &sk);

        assert_tokens(&sk.compact(), &[Token::BorrowedBytes(&SK_BYTES[..])]);
        assert_tokens(&sk.readable(), &[Token::BorrowedStr(SK_STR)]);
        assert_tokens(&pk, &[Token::BorrowedBytes(&PK_BYTES[..])]);
    }
}
