initSidebarItems({"enum":[["CryptoStoreError",""],["DecodeError","Error type for the decoding and storing of the backup key."],["DecryptionError",""],["KeyImportError",""],["MigrationError","Error type for the migration process."],["OutgoingVerificationRequest",""],["PkDecryptionError","Error type for the decryption of backed up room keys."],["Request",""],["RequestType",""],["SecretImportError",""],["SignatureError",""],["UserIdentity","Enum representing cross signing identities of our own user or some other user."],["Verification","Enum representing the different verification flows we support."]],"fn":[["migrate","Migrate a libolm based setup to a vodozemac based setup stored in a Sled store."],["set_logger","Set the logger that should be used to forward Rust logs over FFI."]],"struct":[["BackupKeys","Backup keys and information we load from the store."],["BackupRecoveryKey","The private part of the backup key, the one used for recovery."],["BootstrapCrossSigningResult",""],["CancelInfo","Information on why a verification flow has been cancelled and by whom."],["ConfirmVerificationResult","A result type for confirming verifications."],["CrossSigningKeyExport","A struct containing private cross signing keys that can be backed up or uploaded to the secret store."],["CrossSigningStatus","Struct representing the state of our private cross signing keys, it shows which private cross signing keys we have locally stored."],["DecryptedEvent","An event that was successfully decrypted."],["Device","An E2EE capable Matrix device."],["DeviceLists",""],["KeyRequestPair","A pair of outgoing room key requests, both of those are sendToDevice requests."],["KeysImportResult",""],["MegolmV1BackupKey","The public part of the backup key."],["MigrationData","Struct collecting data that is important to migrate to the rust-sdk"],["OlmMachine","A high level state machine that handles E2EE for Matrix."],["PassphraseInfo","Struct containing info about the way the backup key got derived from a passphrase."],["PickledAccount","A pickled version of an `Account`."],["PickledInboundGroupSession","A pickled version of an `InboundGroupSession`."],["PickledSession","A pickled version of a `Session`."],["QrCode","The `m.qr_code.scan.v1`, `m.qr_code.show.v1`, and `m.reciprocate.v1` verification flow."],["RequestVerificationResult","A result type for requesting verifications."],["RoomKeyCounts","Struct holding the number of room keys we have."],["Sas","The `m.sas.v1` verification flow."],["ScanResult","A result type for scanning QR codes."],["SignatureUploadRequest",""],["StartSasResult","A result type for starting SAS verifications."],["UploadSigningKeysRequest",""],["VerificationRequest","The verificatoin request object which then can transition into some concrete verification method"]],"trait":[["Logger","Trait that can be used to forward Rust logs over FFI to a language specific logger."],["ProgressListener","Callback that will be passed over the FFI to report progress"]]});