//! Code signing services.

use std::{mem::MaybeUninit, str::FromStr};

use core_foundation::{
    base::{TCFType, TCFTypeRef, ToVoid},
    data::CFDataRef,
    dictionary::CFMutableDictionary,
    number::CFNumber,
    string::{CFString, CFStringRef},
    url::CFURL,
};
use libc::pid_t;
use security_framework_sys::code_signing::{
    kSecCSBasicValidateOnly, kSecCSCheckAllArchitectures, kSecCSCheckGatekeeperArchitectures,
    kSecCSCheckNestedCode, kSecCSCheckTrustedAnchors, kSecCSConsiderExpiration,
    kSecCSDoNotValidateExecutable, kSecCSDoNotValidateResources, kSecCSEnforceRevocationChecks,
    kSecCSFullReport, kSecCSNoNetworkAccess, kSecCSQuickCheck, kSecCSReportProgress,
    kSecCSRestrictSidebandData, kSecCSRestrictSymlinks, kSecCSRestrictToAppLike,
    kSecCSSingleThreaded, kSecCSStrictValidate, kSecCSUseSoftwareSigningCert, kSecCSValidatePEH,
    kSecGuestAttributeAudit, kSecGuestAttributePid, SecCodeCheckValidity,
    SecCodeCopyGuestWithAttributes, SecCodeCopyPath, SecCodeCopySelf, SecCodeGetTypeID, SecCodeRef,
    SecRequirementCreateWithString, SecRequirementGetTypeID, SecRequirementRef,
    SecStaticCodeCheckValidity, SecStaticCodeCreateWithPath, SecStaticCodeGetTypeID,
    SecStaticCodeRef,
};

use crate::{cvt, Result};

bitflags::bitflags! {

    /// Values that can be used in the flags parameter to most code signing
    /// functions.
    pub struct Flags: u32 {
        /// Use the default behaviour.
        const NONE = 0;

        /// For multi-architecture (universal) Mach-O programs, validate all
        /// architectures included.
        const CHECK_ALL_ARCHITECTURES = kSecCSCheckAllArchitectures;

        /// Do not validate the contents of the main executable.
        const DO_NOT_VALIDATE_EXECUTABLE = kSecCSDoNotValidateExecutable;

        /// Do not validate the presence and contents of all bundle resources
        /// if any.
        const DO_NOT_VALIDATE_RESOURCES = kSecCSDoNotValidateResources;

        /// Do not validate either the main executable or the bundle resources,
        /// if any.
        const BASIC_VALIDATE_ONLY = kSecCSBasicValidateOnly;

        /// For code in bundle form, locate and recursively check embedded code.
        const CHECK_NESTED_CODE = kSecCSCheckNestedCode;

        /// Perform additional checks to ensure the validity of code in bundle
        /// form.
        const STRICT_VALIDATE = kSecCSStrictValidate;

        /// Apple have not documented this flag.
        const FULL_REPORT = kSecCSFullReport;

        /// Apple have not documented this flag.
        const CHECK_GATEKEEPER_ARCHITECTURES = kSecCSCheckGatekeeperArchitectures;

        /// Apple have not documented this flag.
        const RESTRICT_SYMLINKS = kSecCSRestrictSymlinks;

        /// Apple have not documented this flag.
        const RESTRICT_TO_APP_LIKE = kSecCSRestrictToAppLike;

        /// Apple have not documented this flag.
        const RESTRICT_SIDEBAND_DATA = kSecCSRestrictSidebandData;

        /// Apple have not documented this flag.
        const USE_SOFTWARE_SIGNING_CERT = kSecCSUseSoftwareSigningCert;

        /// Apple have not documented this flag.
        const VALIDATE_PEH = kSecCSValidatePEH;

        /// Apple have not documented this flag.
        const SINGLE_THREADED = kSecCSSingleThreaded;

        /// Apple have not documented this flag.
        const QUICK_CHECK = kSecCSQuickCheck;

        /// Apple have not documented this flag.
        const CHECK_TRUSTED_ANCHORS = kSecCSCheckTrustedAnchors;

        /// Apple have not documented this flag.
        const REPORT_PROGRESS = kSecCSReportProgress;

        /// Apple have not documented this flag.
        const NO_NETWORK_ACCESS = kSecCSNoNetworkAccess;

        /// Apple have not documented this flag.
        const ENFORCE_REVOCATION_CHECKS = kSecCSEnforceRevocationChecks;

        /// Apple have not documented this flag.
        const CONSIDER_EXPIRATION = kSecCSConsiderExpiration;
    }
}

impl Default for Flags {
    #[inline(always)]
    fn default() -> Self {
        Self::NONE
    }
}

/// A helper to create guest attributes, which are normally passed as a
/// `CFDictionary` with varying types.
pub struct GuestAttributes {
    inner: CFMutableDictionary,
}

impl GuestAttributes {
    // Not implemented:
    // - architecture
    // - canonical
    // - dynamic code
    // - dynamic code info plist
    // - hash
    // - mach port
    // - sub-architecture

    /// The guest's audit token.
    pub fn set_audit_token(&mut self, token: CFDataRef) {
        let key = unsafe { CFString::wrap_under_get_rule(kSecGuestAttributeAudit) };
        self.inner.add(&key.as_CFTypeRef(), &token.as_void_ptr());
    }

    /// The guest's pid.
    pub fn set_pid(&mut self, pid: pid_t) {
        let key = unsafe { CFString::wrap_under_get_rule(kSecGuestAttributePid) };
        let pid = CFNumber::from(pid);
        self.inner.add(&key.as_CFTypeRef(), &pid.as_CFTypeRef());
    }

    /// Support for arbirtary guest attributes.
    pub fn set_other<V: ToVoid<V>>(&mut self, key: CFStringRef, value: V) {
        self.inner.add(&key.as_void_ptr(), &value.to_void());
    }
}

declare_TCFType! {
    /// A code object representing signed code running on the system.
    SecRequirement, SecRequirementRef
}
impl_TCFType!(SecRequirement, SecRequirementRef, SecRequirementGetTypeID);

impl FromStr for SecRequirement {
    type Err = crate::base::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let text = CFString::new(s);
        let mut requirement = MaybeUninit::uninit();

        unsafe {
            cvt(SecRequirementCreateWithString(
                text.as_concrete_TypeRef(),
                0,
                requirement.as_mut_ptr(),
            ))?;

            Ok(Self::wrap_under_create_rule(requirement.assume_init()))
        }
    }
}

declare_TCFType! {
    /// A code object representing signed code running on the system.
    SecCode, SecCodeRef
}
impl_TCFType!(SecCode, SecCodeRef, SecCodeGetTypeID);

impl SecCode {
    /// Retrieves the code object for the code making the call.
    pub fn for_self(flags: Flags) -> Result<Self> {
        let mut code = MaybeUninit::uninit();

        unsafe {
            cvt(SecCodeCopySelf(flags.bits(), code.as_mut_ptr()))?;
            Ok(Self::wrap_under_create_rule(code.assume_init()))
        }
    }

    /// Performs dynamic validation of signed code.
    pub fn check_validity(&self, flags: Flags, requirement: &SecRequirement) -> Result<()> {
        unsafe {
            cvt(SecCodeCheckValidity(
                self.as_concrete_TypeRef(),
                flags.bits(),
                requirement.as_concrete_TypeRef(),
            ))
        }
    }

    /// Asks a code host to identify one of its guests given
    /// the type and value of specific attributes of the guest code.
    ///
    /// If `host` is `None` then the code signing root of trust (currently, the
    // system kernel) should be used as the code host.
    pub fn copy_guest_with_attribues(
        host: Option<&SecCode>,
        attrs: &GuestAttributes,
        flags: Flags,
    ) -> Result<SecCode> {
        let mut code = MaybeUninit::uninit();

        let host = match host {
            Some(host) => host.as_concrete_TypeRef(),
            None => std::ptr::null_mut(),
        };

        unsafe {
            cvt(SecCodeCopyGuestWithAttributes(
                host,
                attrs.inner.as_concrete_TypeRef(),
                flags.bits(),
                code.as_mut_ptr(),
            ))?;

            Ok(SecCode::wrap_under_create_rule(code.assume_init()))
        }
    }

    /// Retrieves the location on disk of signed code, given a code or static
    /// code object.
    pub fn path(&self, flags: Flags) -> Result<CFURL> {
        let mut url = MaybeUninit::uninit();

        // The docs say we can pass a SecCodeRef instead of a SecStaticCodeRef.
        unsafe {
            cvt(SecCodeCopyPath(
                self.as_CFTypeRef() as _,
                flags.bits(),
                url.as_mut_ptr(),
            ))?;

            Ok(CFURL::wrap_under_create_rule(url.assume_init()))
        }
    }
}

declare_TCFType! {
    /// A static code object representing signed code on disk.
    SecStaticCode, SecStaticCodeRef
}
impl_TCFType!(SecStaticCode, SecStaticCodeRef, SecStaticCodeGetTypeID);

impl SecStaticCode {
    /// Creates a static code object representing the code at a specified file
    /// system path.
    pub fn from_path(path: &CFURL, flags: Flags) -> Result<Self> {
        let mut code = MaybeUninit::uninit();

        unsafe {
            cvt(SecStaticCodeCreateWithPath(
                path.as_concrete_TypeRef(),
                flags.bits(),
                code.as_mut_ptr(),
            ))?;

            Ok(Self::wrap_under_get_rule(code.assume_init()))
        }
    }

    /// Retrieves the location on disk of signed code, given a code or static
    /// code object.
    pub fn path(&self, flags: Flags) -> Result<CFURL> {
        let mut url = MaybeUninit::uninit();

        // The docs say we can pass a SecCodeRef instead of a SecStaticCodeRef.
        unsafe {
            cvt(SecCodeCopyPath(
                self.as_concrete_TypeRef(),
                flags.bits(),
                url.as_mut_ptr(),
            ))?;

            Ok(CFURL::wrap_under_create_rule(url.assume_init()))
        }
    }

    /// Performs dynamic validation of signed code.
    pub fn check_validity(&self, flags: Flags, requirement: &SecRequirement) -> Result<()> {
        unsafe {
            cvt(SecStaticCodeCheckValidity(
                self.as_concrete_TypeRef(),
                flags.bits(),
                requirement.as_concrete_TypeRef(),
            ))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn path_to_static_code_and_back() {
        let path = CFURL::from_path("/bin/bash", false).unwrap();
        let code = SecStaticCode::from_path(&path, Flags::NONE).unwrap();
        assert_eq!(code.path(Flags::NONE).unwrap(), path);
    }

    #[test]
    fn self_to_path() {
        let path = CFURL::from_path(std::env::current_exe().unwrap(), false).unwrap();
        let code = SecCode::for_self(Flags::NONE).unwrap();
        assert_eq!(code.path(Flags::NONE).unwrap(), path);
    }

    #[test]
    fn bash_is_signed_by_apple() {
        let path = CFURL::from_path("/bin/bash", false).unwrap();
        let code = SecStaticCode::from_path(&path, Flags::NONE).unwrap();
        let requirement: SecRequirement = "anchor apple".parse().unwrap();
        code.check_validity(Flags::NONE, &requirement).unwrap();
    }

    #[test]
    fn self_is_not_signed_by_apple() {
        let code = SecCode::for_self(Flags::NONE).unwrap();
        let requirement: SecRequirement = "anchor apple".parse().unwrap();

        assert_eq!(
            code.check_validity(Flags::NONE, &requirement)
                .unwrap_err()
                .code(),
            // "code object is not signed at all"
            -67062
        );
    }
}
