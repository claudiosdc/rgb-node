Change Log
==========

v0.2.1
------
- Fix to PSBT tweaking key representation for `i9n` integration interface
- README improvements

v0.2.0
------

### Core features
- LNP Node and lightning network interoperability
- RPC commands to get allocations for assets and outpoints
- Asset `validate`, `accept`, `asset_allocations` and `outpoint_assets` methods
  in integration mod
- Sync operation supports multiple data formats
- Strict encoding for asset data. Adding strict encode-based import/export.

### Breaking changes
- Changed order and types of arguments in integration module, cli & RPC

### Changes since RC5
- Fixed issue #102 with wrong PSBT decoding when deserializing from in-memory 
  data
- Fixed fungible command-line argument name (stash -> cache)
- Fixing integration config parameters
- Fixed issue #101: error message on fungible CLI API consistent with the code
- Released some of the dependency version requirements in Cargo.toml

v0.2.0-rc.5
-----------
- Fixed secondary issue rights processing in integration module and folding of
  repeated outputs holding multiple secondary issuance rights

v0.2.0-rc.4
-----------
- Added asset import and export methods into I9N (integration) mod

v0.2.0-rc.3
-----------
- Updated to LNP/BP Core Lib v0.2 release

v0.2.0-rc.2
-----------
- Updated to LNP/BP Core Lib RC2
- Fixed broken semversioning of the upstream dependencies in tokio and other
  crates

v0.2.0-rc.1
-----------
- Updating issuance to match the latest RGB20 schema changes
- Internal optimizations for RGB20 processor mod
- Typos and dependency fixes
- Using f64 instead of f32 for internal accounting amounts representations

v0.2.0-beta.4
-------------

### Fixes:
- Fixes to configuration in integration mode
- Fixes default builds when serde is not used


v0.2.0-beta.3
-------------

### Features
- Asset `validate`, `accept`, `asset_allocations` and `outpoint_assets` methods
  in integration mod


v0.2.0-beta.2
-------------

### Features:
- RPC commands to get allocations for assets and outpoints
- Sync operation supports multiple data formats
- Strict encoding for asset data. Adding strict encode-based import/export.


### Fixes:
- Fixing problem with prune right in asset issuance


v0.2.0-beta.1
-------------
Migrated to the second version of LNP/BP Core Library (v0.2, currently beta-1).

### Fixes:
- Fixed `rgb-cli export` command (now it parses Bech32-formatted asset name)
- Updated feature structure, fixed feature interdependencies


v0.1.1
------

### Fixes
- Exposed `contracts::fungible::data::invoice::Error` as `InvoiceError`
  <https://github.com/LNP-BP/rgb-node/pull/93>


v0.1.0
------

### Core features
- RGB Stash daemon operating client-validated data and managing their file 
  storage
- Fungible daemon operating RGB-20 assets and managing their storage (both file
  and SQLite)
- RGB-20 asset issuance, invoicing, trnasfer and transfer acceptance
- Command-line tool for daemons operations
- Itegration functions packed as a compiled library

### New features since RC2
- SQLite storage for assets cached data
- Storage of public key tweaking information in PSBT
- Support of Rust stable and old version up to 1.41.1
- Update to the latest public releases of upstream bitcoin and LNP/BP libraries
  (migration from self-maintained forks)

### Breaking changes
- Standard-compliant use of PSBT extension fields.
- Removed requirements to specify fee for the transaction (it is now computed 
  from PSBT data)


v0.1.0-rc.2
-----------

### Main updates
- FFI and demo apps are moved into a separate 
  [**RGB SDK**](https://github.com/LNP-BP/rgb-sdk) project
- Big update and refactoring in RGB-20 schema (fungible assets)
  * Multiple inflation rights with better control over total inflation
  * Epoch-based burn and burn-and-replace procedures; enhanced with UTXO set and
    versioned proofs of burn data, supporting up to 15 burn proof variants 
    (+"no proofs" option)
  * Asset renomination procedure, for changing asset names or splitting stock 
    shares
  * Standardization of contract text URL and commitment format
  * Rights split procedure
  * Removed dust limit
- Proposal of RGB-21 schema after prolonged discussions (not available through
  API yet)
  * Unique tokens & token-specfic data
  * Issue control
  * Generic token data, internal or external, in different formats
  * Engravings (any why Schema subclassing is required)
  * LockUTXOs and descriptors for proof of reserves
  * Renominations
  * Rights splits
- New `rgb-cli` commands and `stash` daemon operations:
  * Listing available schemata
  * Exporting and inspecting schema in multiple formats (JSON, YAML, 
    StrictEncoding)
  * Listing known contracts
  * Exporting and inspecting contract genesis in multiple formats (JSON, YAML, 
    StrictEncoding)

### Breaking changes:
- Removal of dust limit parameter from command-line, RPC API calls and 
  FFI API integration points
