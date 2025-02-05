use crate::*;
use core::slice;
use lakers_crypto::default_crypto;
use lakers_ead::*;

#[derive(Debug)]
#[repr(C)]
pub struct EadAuthzDevice {
    pub start: ZeroTouchDevice,
    pub wait_ead2: ZeroTouchDeviceWaitEAD2,
    pub done: ZeroTouchDeviceDone,
}

#[no_mangle]
pub unsafe extern "C" fn authz_device_new(
    device_c: *mut EadAuthzDevice,
    id_u: *const u8,
    id_u_len: usize,
    g_w: *const BytesP256ElemLen,
    loc_w: *const u8,
    loc_w_len: usize,
) -> i8 {
    let Ok(id_u) = EdhocMessageBuffer::new_from_slice(slice::from_raw_parts(id_u, id_u_len)) else {
        return -1;
    };
    let Ok(loc_w) = EdhocMessageBuffer::new_from_slice(slice::from_raw_parts(loc_w, loc_w_len))
    else {
        return -1;
    };

    (*device_c).start.id_u = id_u;
    (*device_c).start.g_w = *g_w;
    (*device_c).start.loc_w = loc_w;

    0
}

#[no_mangle]
pub unsafe extern "C" fn authz_device_prepare_ead_1(
    // input parans
    device_c: *mut EadAuthzDevice,
    secret: *const BytesP256ElemLen,
    ss: u8,
    // output parans
    ead_1_c_out: *mut EADItemC,
) -> i8 {
    let crypto = &mut default_crypto();
    let (device, ead_1) = (*device_c).start.prepare_ead_1(crypto, *secret, ss);
    (*device_c).wait_ead2 = device;
    EADItemC::copy_into_c(ead_1, ead_1_c_out);

    0
}

#[no_mangle]
pub unsafe extern "C" fn authz_device_process_ead_2(
    // input parans
    device_c: *mut EadAuthzDevice,
    ead_2_c: *mut EADItemC,
    cred_v: CredentialRPK,
) -> i8 {
    let crypto = &mut default_crypto();
    match (*device_c)
        .wait_ead2
        .process_ead_2(crypto, (*ead_2_c).to_rust(), cred_v.value.as_slice())
    {
        Ok(device) => {
            (*device_c).done = device;
            0
        }
        Err(_) => -1,
    }
}
