//! Implementation of [EDHOC] (Ephemeral Diffie-Hellman Over COSE), a lightweight authenticated key
//! exchange for the Internet of Things.
//!
//! The crate provides a high-level interface through the [EdhocInitiator] and the [EdhocResponder]
//! structs. Both these wrap the lower level [State] struct that is mainly used through internal
//! functions in the `edhoc` module. This separation is relevant because the lower level tools are
//! subject of ongoing formal verification, whereas the high-level interfaces aim for good
//! usability.
//!
//! Both [EdhocInitiator] and [EdhocResponder] are used in a type stated way. Following the EDHOC
//! protocol, they generate (or process) messages, progressively provide more information about
//! their peer, and on eventually devolve into an [EdhocInitiatorDone] and [EdhocResponderDone],
//! respectively, through which the EDHOC key material can be obtained.
//!
//! [EDHOC]: https://datatracker.ietf.org/doc/draft-ietf-lake-edhoc/
#![cfg_attr(not(test), no_std)]

pub use {lakers_shared::Crypto as CryptoTrait, lakers_shared::*};

#[cfg(any(feature = "ead-none", feature = "ead-authz"))]
pub use lakers_ead::*;

mod edhoc;
pub use edhoc::*;

// TODO: clean these structs and remove the cred_x whre they are not needed anymore
/// Starting point for performing EDHOC in the role of the Initiator.
#[derive(Debug)]
pub struct EdhocInitiator<Crypto: CryptoTrait> {
    state: InitiatorStart, // opaque state
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocInitiatorWaitM2<Crypto: CryptoTrait> {
    state: WaitM2, // opaque state
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocInitiatorProcessingM2<Crypto: CryptoTrait> {
    state: ProcessingM2, // opaque state
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocInitiatorProcessedM2<Crypto: CryptoTrait> {
    state: ProcessedM2,    // opaque state
    cred_i: CredentialRPK, // I's full credential
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocInitiatorDone<Crypto: CryptoTrait> {
    state: Completed,
    crypto: Crypto,
}

/// Starting point for performing EDHOC in the role of the Responder.
#[derive(Debug)]
pub struct EdhocResponder<'a, Crypto: CryptoTrait> {
    state: ResponderStart, // opaque state
    r: &'a [u8],           // private authentication key of R
    cred_r: CredentialRPK, // R's full credential
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocResponderProcessedM1<'a, Crypto: CryptoTrait> {
    state: ProcessingM1,   // opaque state
    r: &'a [u8],           // private authentication key of R
    cred_r: CredentialRPK, // R's full credential
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocResponderWaitM3<Crypto: CryptoTrait> {
    state: WaitM3, // opaque state
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocResponderProcessingM3<Crypto: CryptoTrait> {
    state: ProcessingM3, // opaque state
    crypto: Crypto,
}

#[derive(Debug)]
pub struct EdhocResponderDone<Crypto: CryptoTrait> {
    state: Completed,
    crypto: Crypto,
}

impl<'a, Crypto: CryptoTrait> EdhocResponder<'a, Crypto> {
    pub fn new(mut crypto: Crypto, r: &'a [u8], cred_r: CredentialRPK) -> Self {
        assert!(r.len() == P256_ELEM_LEN);
        let (y, g_y) = crypto.p256_generate_key_pair();

        EdhocResponder {
            state: ResponderStart { y, g_y },
            r,
            cred_r,
            crypto,
        }
    }

    pub fn process_message_1(
        mut self,
        message_1: &BufferMessage1,
    ) -> Result<(EdhocResponderProcessedM1<'a, Crypto>, Option<EADItem>), EDHOCError> {
        let (state, ead_1) = r_process_message_1(&self.state, &mut self.crypto, message_1)?;

        Ok((
            EdhocResponderProcessedM1 {
                state,
                r: self.r,
                cred_r: self.cred_r,
                crypto: self.crypto,
            },
            ead_1,
        ))
    }
}

impl<'a, Crypto: CryptoTrait> EdhocResponderProcessedM1<'a, Crypto> {
    pub fn prepare_message_2(
        mut self,
        cred_transfer: CredentialTransfer,
        c_r: Option<u8>,
        ead_2: &Option<EADItem>,
    ) -> Result<(EdhocResponderWaitM3<Crypto>, BufferMessage2), EDHOCError> {
        let c_r = match c_r {
            Some(c_r) => c_r,
            None => generate_connection_identifier_cbor(&mut self.crypto),
        };

        match r_prepare_message_2(
            &self.state,
            &mut self.crypto,
            self.cred_r,
            self.r.try_into().expect("Wrong length of private key"),
            c_r,
            cred_transfer,
            ead_2,
        ) {
            Ok((state, message_2)) => Ok((
                EdhocResponderWaitM3 {
                    state,
                    crypto: self.crypto,
                },
                message_2,
            )),
            Err(error) => Err(error),
        }
    }
}

impl<'a, Crypto: CryptoTrait> EdhocResponderWaitM3<Crypto> {
    pub fn parse_message_3(
        mut self,
        message_3: &'a BufferMessage3,
    ) -> Result<
        (
            EdhocResponderProcessingM3<Crypto>,
            CredentialRPK,
            Option<EADItem>,
        ),
        EDHOCError,
    > {
        match r_parse_message_3(&mut self.state, &mut self.crypto, message_3) {
            Ok((state, id_cred_i, ead_3)) => Ok((
                EdhocResponderProcessingM3 {
                    state,
                    crypto: self.crypto,
                },
                id_cred_i,
                ead_3,
            )),
            Err(error) => Err(error),
        }
    }
}

impl<'a, Crypto: CryptoTrait> EdhocResponderProcessingM3<Crypto> {
    pub fn verify_message_3(
        mut self,
        cred_i: CredentialRPK,
    ) -> Result<(EdhocResponderDone<Crypto>, [u8; SHA256_DIGEST_LEN]), EDHOCError> {
        match r_verify_message_3(&mut self.state, &mut self.crypto, cred_i) {
            Ok((state, prk_out)) => Ok((
                EdhocResponderDone {
                    state,
                    crypto: self.crypto,
                },
                prk_out,
            )),
            Err(error) => Err(error),
        }
    }
}

impl<Crypto: CryptoTrait> EdhocResponderDone<Crypto> {
    pub fn edhoc_exporter(
        &mut self,
        label: u8,
        context: &[u8],
        length: usize,
    ) -> [u8; MAX_BUFFER_LEN] {
        let mut context_buf: BytesMaxContextBuffer = [0x00u8; MAX_KDF_CONTEXT_LEN];
        context_buf[..context.len()].copy_from_slice(context);

        edhoc_exporter(
            &self.state,
            &mut self.crypto,
            label,
            &context_buf,
            context.len(),
            length,
        )
    }

    pub fn edhoc_key_update(&mut self, context: &[u8]) -> [u8; SHA256_DIGEST_LEN] {
        let mut context_buf = [0x00u8; MAX_KDF_CONTEXT_LEN];
        context_buf[..context.len()].copy_from_slice(context);

        edhoc_key_update(
            &mut self.state,
            &mut self.crypto,
            &context_buf,
            context.len(),
        )
    }
}

impl<'a, Crypto: CryptoTrait> EdhocInitiator<Crypto> {
    pub fn new(mut crypto: Crypto) -> Self {
        // we only support a single cipher suite which is already CBOR-encoded
        let mut suites_i: BytesSuites = [0x0; SUITES_LEN];
        let suites_i_len = EDHOC_SUPPORTED_SUITES.len();
        suites_i[0..suites_i_len].copy_from_slice(&EDHOC_SUPPORTED_SUITES[..]);
        let (x, g_x) = crypto.p256_generate_key_pair();

        EdhocInitiator {
            state: InitiatorStart {
                x,
                g_x,
                suites_i,
                suites_i_len,
            },
            crypto,
        }
    }

    pub fn prepare_message_1(
        mut self,
        c_i: Option<u8>,
        ead_1: &Option<EADItem>,
    ) -> Result<(EdhocInitiatorWaitM2<Crypto>, EdhocMessageBuffer), EDHOCError> {
        let c_i = match c_i {
            Some(c_i) => c_i,
            None => generate_connection_identifier_cbor(&mut self.crypto),
        };

        match i_prepare_message_1(&self.state, &mut self.crypto, c_i, ead_1) {
            Ok((state, message_1)) => Ok((
                EdhocInitiatorWaitM2 {
                    state,
                    crypto: self.crypto,
                },
                message_1,
            )),
            Err(error) => Err(error),
        }
    }

    pub fn compute_ephemeral_secret(&mut self, g_a: &BytesP256ElemLen) -> BytesP256ElemLen {
        self.crypto.p256_ecdh(&self.state.x, g_a)
    }

    pub fn selected_cipher_suite(&self) -> u8 {
        self.state.suites_i[self.state.suites_i_len - 1]
    }
}

impl<'a, Crypto: CryptoTrait> EdhocInitiatorWaitM2<Crypto> {
    pub fn parse_message_2(
        mut self,
        message_2: &'a BufferMessage2,
    ) -> Result<
        (
            EdhocInitiatorProcessingM2<Crypto>,
            u8,
            CredentialRPK,
            Option<EADItem>,
        ),
        EDHOCError,
    > {
        match i_parse_message_2(&self.state, &mut self.crypto, message_2) {
            Ok((state, c_r, id_cred_r, ead_2)) => Ok((
                EdhocInitiatorProcessingM2 {
                    state,
                    crypto: self.crypto,
                },
                c_r,
                id_cred_r,
                ead_2,
            )),
            Err(error) => Err(error),
        }
    }
}

impl<'a, Crypto: CryptoTrait> EdhocInitiatorProcessingM2<Crypto> {
    pub fn verify_message_2(
        mut self,
        i: &'a [u8],
        cred_i: CredentialRPK,
        valid_cred_r: CredentialRPK,
    ) -> Result<EdhocInitiatorProcessedM2<Crypto>, EDHOCError> {
        match i_verify_message_2(
            &self.state,
            &mut self.crypto,
            valid_cred_r,
            i.try_into().expect("Wrong length of initiator private key"),
        ) {
            Ok(state) => Ok(EdhocInitiatorProcessedM2 {
                state,
                cred_i: cred_i,
                crypto: self.crypto,
            }),
            Err(error) => Err(error),
        }
    }
}

impl<'a, Crypto: CryptoTrait> EdhocInitiatorProcessedM2<Crypto> {
    pub fn prepare_message_3(
        mut self,
        cred_transfer: CredentialTransfer,
        ead_3: &Option<EADItem>,
    ) -> Result<
        (
            EdhocInitiatorDone<Crypto>,
            BufferMessage3,
            [u8; SHA256_DIGEST_LEN],
        ),
        EDHOCError,
    > {
        match i_prepare_message_3(
            &mut self.state,
            &mut self.crypto,
            self.cred_i,
            cred_transfer,
            ead_3,
        ) {
            Ok((state, message_3, prk_out)) => Ok((
                EdhocInitiatorDone {
                    state,
                    crypto: self.crypto,
                },
                message_3,
                prk_out,
            )),
            Err(error) => Err(error),
        }
    }
}

impl<Crypto: CryptoTrait> EdhocInitiatorDone<Crypto> {
    pub fn edhoc_exporter(
        &mut self,
        label: u8,
        context: &[u8],
        length: usize,
    ) -> [u8; MAX_BUFFER_LEN] {
        let mut context_buf: BytesMaxContextBuffer = [0x00u8; MAX_KDF_CONTEXT_LEN];
        context_buf[..context.len()].copy_from_slice(context);

        edhoc_exporter(
            &self.state,
            &mut self.crypto,
            label,
            &context_buf,
            context.len(),
            length,
        )
    }

    pub fn edhoc_key_update(&mut self, context: &[u8]) -> [u8; SHA256_DIGEST_LEN] {
        let mut context_buf = [0x00u8; MAX_KDF_CONTEXT_LEN];
        context_buf[..context.len()].copy_from_slice(context);

        edhoc_key_update(
            &mut self.state,
            &mut self.crypto,
            &context_buf,
            context.len(),
        )
    }
}

pub fn generate_connection_identifier_cbor<Crypto: CryptoTrait>(crypto: &mut Crypto) -> u8 {
    let c_i = generate_connection_identifier(crypto);
    if c_i >= 0 && c_i <= 23 {
        c_i as u8 // verbatim encoding of single byte integer
    } else if c_i < 0 && c_i >= -24 {
        // negative single byte integer encoding
        CBOR_NEG_INT_1BYTE_START - 1 + c_i.unsigned_abs()
    } else {
        0
    }
}

/// generates an identifier that can be serialized as a single CBOR integer, i.e. -24 <= x <= 23
pub fn generate_connection_identifier<Crypto: CryptoTrait>(crypto: &mut Crypto) -> i8 {
    let mut conn_id = crypto.get_random_byte() as i8;
    while conn_id < -24 || conn_id > 23 {
        conn_id = crypto.get_random_byte() as i8;
    }
    conn_id
}

// Implements auth credential checking according to draft-tiloca-lake-implem-cons
pub fn credential_check_or_fetch<'a>(
    cred_expected: Option<CredentialRPK>,
    id_cred_received: CredentialRPK,
) -> Result<CredentialRPK, EDHOCError> {
    // Processing of auth credentials according to draft-tiloca-lake-implem-cons
    // Comments tagged with a number refer to steps in Section 4.3.1. of draft-tiloca-lake-implem-cons
    if let Some(cred_expected) = cred_expected {
        // 1. Does ID_CRED_X point to a stored authentication credential? YES
        // IMPL: compare cred_i_expected with id_cred
        //   IMPL: assume cred_i_expected is well formed
        let credentials_match = if id_cred_received.reference_only() {
            id_cred_received.kid == cred_expected.kid
        } else {
            id_cred_received.value == cred_expected.value
        };

        // 2. Is this authentication credential still valid?
        // IMPL,TODO: check cred_r_expected is still valid

        // Continue by considering CRED_X as the authentication credential of the other peer.
        // IMPL: ready to proceed, including process ead_2

        if credentials_match {
            Ok(cred_expected)
        } else {
            Err(EDHOCError::UnknownPeer)
        }
    } else {
        // 1. Does ID_CRED_X point to a stored authentication credential? NO
        // IMPL: cred_i_expected provided by application is None
        //       id_cred must be a full credential
        // 3. Is the trust model Pre-knowledge-only? NO (hardcoded to NO for now)
        // 4. Is the trust model Pre-knowledge + TOFU? YES (hardcoded to YES for now)
        // 6. Validate CRED_X. Generally a CCS has to be validated only syntactically and semantically, unlike a certificate or a CWT.
        //    Is the validation successful?
        // IMPL,NOTE: the credential has already been parsed with CredentialRPK::new in the *_parse_message_* function
        // 5. Is the authentication credential authorized for use in the context of this EDHOC session?
        // IMPL,TODO: we just skip this step for now
        // 7. Store CRED_X as valid and trusted.
        //   Pair it with consistent credential identifiers, for each supported type of credential identifier.

        assert!(!id_cred_received.reference_only());
        Ok(id_cred_received)
    }

    // 8. Is this authentication credential good to use in the context of this EDHOC session?
    // IMPL,TODO: we just skip this step for now
}

#[cfg(test)]
mod test_vectors_common {
    use hexlit::hex;

    pub const ID_CRED_I: &[u8] = &hex!("a104412b");
    pub const ID_CRED_R: &[u8] = &hex!("a104410a");
    pub const CRED_I: &[u8] = &hex!("A2027734322D35302D33312D46462D45462D33372D33322D333908A101A5010202412B2001215820AC75E9ECE3E50BFC8ED60399889522405C47BF16DF96660A41298CB4307F7EB62258206E5DE611388A4B8A8211334AC7D37ECB52A387D257E6DB3C2A93DF21FF3AFFC8");
    pub const I: &[u8] = &hex!("fb13adeb6518cee5f88417660841142e830a81fe334380a953406a1305e8706b");
    pub const R: &[u8] = &hex!("72cc4761dbd4c78f758931aa589d348d1ef874a7e303ede2f140dcf3e6aa4aac");
    pub const _G_I_Y_COORD: &[u8] =
        &hex!("6e5de611388a4b8a8211334ac7d37ecb52a387d257e6db3c2a93df21ff3affc8"); // not used
    pub const CRED_R: &[u8] = &hex!("A2026008A101A5010202410A2001215820BBC34960526EA4D32E940CAD2A234148DDC21791A12AFBCBAC93622046DD44F02258204519E257236B2A0CE2023F0931F1F386CA7AFDA64FCDE0108C224C51EABF6072");

    pub const MESSAGE_1_TV_FIRST_TIME: &str =
        "03065820741a13d7ba048fbb615e94386aa3b61bea5b3d8f65f32620b749bee8d278efa90e";
    pub const MESSAGE_1_TV: &str =
        "0382060258208af6f430ebe18d34184017a9a11bf511c8dff8f834730b96c1b7c8dbca2fc3b637";
}

#[cfg(test)]
mod test {
    use super::*;
    use lakers_crypto::default_crypto;
    use test_vectors_common::*;

    #[test]
    fn test_new_initiator() {
        let _initiator = EdhocInitiator::new(default_crypto());
    }

    #[test]
    fn test_new_responder() {
        let _responder = EdhocResponder::new(
            default_crypto(),
            R,
            CredentialRPK::new(CRED_R.try_into().unwrap()).unwrap(),
        );
    }

    #[test]
    fn test_prepare_message_1() {
        let initiator = EdhocInitiator::new(default_crypto());

        let c_i = generate_connection_identifier_cbor(&mut default_crypto());
        let result = initiator.prepare_message_1(Some(c_i), &None);
        assert!(result.is_ok());
    }

    #[test]
    fn test_process_message_1() {
        let message_1_tv_first_time = EdhocMessageBuffer::from_hex(MESSAGE_1_TV_FIRST_TIME);
        let message_1_tv = EdhocMessageBuffer::from_hex(MESSAGE_1_TV);
        let responder = EdhocResponder::new(
            default_crypto(),
            R,
            CredentialRPK::new(CRED_R.try_into().unwrap()).unwrap(),
        );

        // process message_1 first time, when unsupported suite is selected
        let error = responder.process_message_1(&message_1_tv_first_time);
        assert!(error.is_err());
        assert_eq!(error.unwrap_err(), EDHOCError::UnsupportedCipherSuite);

        // We need to create a new responder -- no message is supposed to be processed twice by a
        // responder or initiator
        let responder = EdhocResponder::new(
            default_crypto(),
            R,
            CredentialRPK::new(CRED_R.try_into().unwrap()).unwrap(),
        );

        // process message_1 second time
        let error = responder.process_message_1(&message_1_tv);
        assert!(error.is_ok());
    }

    #[test]
    fn test_generate_connection_identifier() {
        let conn_id = generate_connection_identifier(&mut default_crypto());
        assert!(conn_id >= -24 && conn_id <= 23);
    }

    #[cfg(feature = "test-ead-none")]
    #[test]
    fn test_handshake() {
        let cred_i = CredentialRPK::new(CRED_I.try_into().unwrap()).unwrap();
        let cred_r = CredentialRPK::new(CRED_R.try_into().unwrap()).unwrap();

        let initiator = EdhocInitiator::new(default_crypto()); // can choose which identity to use after learning R's identity
        let responder = EdhocResponder::new(default_crypto(), R, cred_r.clone()); // has to select an identity before learning who is I

        // ---- begin initiator handling
        // if needed: prepare ead_1
        let (initiator, message_1) = initiator.prepare_message_1(None, &None).unwrap();
        // ---- end initiator handling

        // ---- begin responder handling
        let (responder, _ead_1) = responder.process_message_1(&message_1).unwrap();
        // if ead_1: process ead_1
        // if needed: prepare ead_2
        let (responder, message_2) = responder
            .prepare_message_2(CredentialTransfer::ByReference, None, &None)
            .unwrap();
        // ---- end responder handling

        // ---- being initiator handling
        let (initiator, _c_r, id_cred_r, _ead_2) = initiator.parse_message_2(&message_2).unwrap();
        let valid_cred_r = credential_check_or_fetch(Some(cred_r), id_cred_r).unwrap();
        let initiator = initiator.verify_message_2(I, cred_i, valid_cred_r).unwrap();

        // if needed: prepare ead_3
        let (mut initiator, message_3, i_prk_out) = initiator
            .prepare_message_3(CredentialTransfer::ByReference, &None)
            .unwrap();
        // ---- end initiator handling

        // ---- begin responder handling
        let (responder, id_cred_i, _ead_3) = responder.parse_message_3(&message_3).unwrap();
        let valid_cred_i = credential_check_or_fetch(Some(cred_i), id_cred_i).unwrap();
        // if ead_3: process ead_3
        let (mut responder, r_prk_out) = responder.verify_message_3(valid_cred_i).unwrap();
        // ---- end responder handling

        // check that prk_out is equal at initiator and responder side
        assert_eq!(i_prk_out, r_prk_out);

        // derive OSCORE secret and salt at both sides and compare
        let i_oscore_secret = initiator.edhoc_exporter(0u8, &[], 16); // label is 0
        let i_oscore_salt = initiator.edhoc_exporter(1u8, &[], 8); // label is 1

        let r_oscore_secret = responder.edhoc_exporter(0u8, &[], 16); // label is 0
        let r_oscore_salt = responder.edhoc_exporter(1u8, &[], 8); // label is 1

        assert_eq!(i_oscore_secret, r_oscore_secret);
        assert_eq!(i_oscore_salt, r_oscore_salt);

        // test key update with context from draft-ietf-lake-traces
        let i_prk_out_new = initiator.edhoc_key_update(&[
            0xa0, 0x11, 0x58, 0xfd, 0xb8, 0x20, 0x89, 0x0c, 0xd6, 0xbe, 0x16, 0x96, 0x02, 0xb8,
            0xbc, 0xea,
        ]);
        let r_prk_out_new = responder.edhoc_key_update(&[
            0xa0, 0x11, 0x58, 0xfd, 0xb8, 0x20, 0x89, 0x0c, 0xd6, 0xbe, 0x16, 0x96, 0x02, 0xb8,
            0xbc, 0xea,
        ]);

        assert_eq!(i_prk_out_new, r_prk_out_new);
    }
}

#[cfg(feature = "test-ead-authz")]
#[cfg(test)]
mod test_authz {
    use super::*;
    use hexlit::hex;
    use lakers_crypto::default_crypto;
    use lakers_ead::*;
    use test_vectors_common::*;

    // U
    const ID_U_TV: &[u8] = &hex!("a104412b");

    // V -- nothing to do, will reuse CRED_R from above to act as CRED_V

    // W
    pub const W_TV: &[u8] =
        &hex!("4E5E15AB35008C15B89E91F9F329164D4AACD53D9923672CE0019F9ACD98573F");
    const G_W_TV: &[u8] = &hex!("FFA4F102134029B3B156890B88C9D9619501196574174DCB68A07DB0588E4D41");
    const LOC_W_TV: &[u8] = &hex!("636F61703A2F2F656E726F6C6C6D656E742E736572766572");

    // TODO: have a setup_test function that prepares the common objects for the ead tests
    #[test]
    fn test_handshake_authz() {
        let cred_i = CredentialRPK::new(CRED_I.try_into().unwrap()).unwrap();
        let cred_r = CredentialRPK::new(CRED_R.try_into().unwrap()).unwrap();

        // ==== initialize edhoc ====
        let mut initiator = EdhocInitiator::new(default_crypto());
        let responder = EdhocResponder::new(default_crypto(), R, cred_r);

        // ==== initialize ead-authz ====
        let device = ZeroTouchDevice::new(
            ID_U_TV.try_into().unwrap(),
            G_W_TV.try_into().unwrap(),
            LOC_W_TV.try_into().unwrap(),
        );
        let authenticator = ZeroTouchAuthenticator::default();

        let acl = EdhocMessageBuffer::new_from_slice(&[cred_i.kid]).unwrap();
        let server = ZeroTouchServer::new(
            W_TV.try_into().unwrap(),
            CRED_R.try_into().unwrap(),
            Some(acl),
        );

        // ==== begin edhoc with ead-authz ====

        let (mut device, ead_1) = device.prepare_ead_1(
            &mut default_crypto(),
            initiator.compute_ephemeral_secret(&device.g_w),
            initiator.selected_cipher_suite(),
        );
        let (initiator, message_1) = initiator.prepare_message_1(None, &Some(ead_1)).unwrap();
        device.set_h_message_1(initiator.state.h_message_1.clone());

        let (responder, ead_1) = responder.process_message_1(&message_1).unwrap();
        let ead_2 = if let Some(ead_1) = ead_1 {
            let (authenticator, _loc_w, voucher_request) =
                authenticator.process_ead_1(&ead_1, &message_1).unwrap();

            // the line below mocks a request to the server: let voucher_response = auth_client.post(loc_w, voucher_request)?
            let voucher_response = server
                .handle_voucher_request(&mut default_crypto(), &voucher_request)
                .unwrap();

            let res = authenticator.prepare_ead_2(&voucher_response);
            assert!(res.is_ok());
            authenticator.prepare_ead_2(&voucher_response).ok()
        } else {
            None
        };
        let (responder, message_2) = responder
            .prepare_message_2(CredentialTransfer::ByValue, None, &ead_2)
            .unwrap();

        let (initiator, _c_r, id_cred_r, ead_2) = initiator.parse_message_2(&message_2).unwrap();
        let valid_cred_r = credential_check_or_fetch(None, id_cred_r).unwrap();
        if let Some(ead_2) = ead_2 {
            let result = device.process_ead_2(&mut default_crypto(), ead_2, CRED_R);
            assert!(result.is_ok());
        }
        let initiator = initiator.verify_message_2(I, cred_i, valid_cred_r).unwrap();

        let (mut _initiator, message_3, i_prk_out) = initiator
            .prepare_message_3(CredentialTransfer::ByReference, &None)
            .unwrap();

        let (responder, id_cred_i, _ead_3) = responder.parse_message_3(&message_3).unwrap();
        let valid_cred_i = credential_check_or_fetch(Some(cred_i), id_cred_i).unwrap();
        let (mut _responder, r_prk_out) = responder.verify_message_3(valid_cred_i).unwrap();

        // check that prk_out is equal at initiator and responder side
        assert_eq!(i_prk_out, r_prk_out);
    }
}
