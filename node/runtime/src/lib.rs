// Copyright 2018-2019 Parity Technologies (UK) Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! The Substrate runtime. This can be compiled with ``#[no_std]`, ready for Wasm.

#![cfg_attr(not(feature = "std"), no_std)]
// `construct_runtime!` does a lot of recursion and requires us to increase the limit to 256.
#![recursion_limit = "256"]

use client::{
    block_builder::api::{self as block_builder_api, CheckInherentsResult, InherentData},
    impl_runtime_apis, runtime_api as client_api,
};
use grandpa::fg_primitives::{self, ScheduledChange};
pub use node_primitives::{AccountId, AccountIndex, AuraId, Balance, BlockNumber, Hash, Moment, Nonce, Signature};
use rstd::prelude::*;
use runtime_primitives::traits::{BlakeTwo256, Block as BlockT, Convert, DigestFor, NumberFor, StaticLookup};
use runtime_primitives::transaction_validity::TransactionValidity;
use runtime_primitives::{create_runtime_str, generic, ApplyResult};
use substrate_primitives::u32_trait::{_1, _2, _3, _4};
use support::{
    construct_runtime, parameter_types,
    traits::{Currency, OnUnbalanced, SplitTwoWays},
};
use version::RuntimeVersion;

use finality_tracker::{DEFAULT_REPORT_LATENCY, DEFAULT_WINDOW_SIZE};
use grandpa::{AuthorityId as GrandpaId, AuthorityWeight as GrandpaWeight};
use substrate_primitives::OpaqueMetadata;
#[cfg(any(feature = "std", test))]
use version::NativeVersion;

pub use balances::Call as BalancesCall;
pub use contracts::Gas;
#[cfg(any(feature = "std", test))]
pub use runtime_primitives::BuildStorage;
pub use runtime_primitives::{impl_opaque_keys, Perbill, Permill};
pub use staking::EraIndex;
pub use staking::StakerStatus;
pub use support::StorageValue;
pub use timestamp::Call as TimestampCall;

/// Runtime version.
pub const VERSION: RuntimeVersion = RuntimeVersion {
    spec_name: create_runtime_str!("node"),
    impl_name: create_runtime_str!("darwinia-node"),
    authoring_version: 2,
    spec_version: 78,
    impl_version: 78,
    apis: RUNTIME_API_VERSIONS,
};

/// Native version.
#[cfg(any(feature = "std", test))]
pub fn native_version() -> NativeVersion {
    NativeVersion {
        runtime_version: VERSION,
        can_author_with: Default::default(),
    }
}

pub const NANO: Balance = 1;
pub const MICRO: Balance = 1_000 * NANO;
pub const MILLI: Balance = 1_000 * MICRO;
pub const COIN: Balance = 1_000 * MILLI;

type NegativeImbalance = <Balances as Currency<AccountId>>::NegativeImbalance;

pub struct Author;

impl OnUnbalanced<NegativeImbalance> for Author {
    fn on_unbalanced(amount: NegativeImbalance) {
        Balances::resolve_creating(&Authorship::author(), amount);
    }
}

pub struct MockTreasury;
impl OnUnbalanced<NegativeImbalance> for MockTreasury {
    fn on_unbalanced(amount: NegativeImbalance) {
        Balances::resolve_creating(&Sudo::key(), amount);
    }
}

pub type DealWithFees = SplitTwoWays<
    Balance,
    NegativeImbalance,
    _4,
    MockTreasury, // 4 parts (80%) goes to the treasury.
    _1,
    Author, // 1 part (20%) goes to the block author.
>;

pub const SECS_PER_BLOCK: Moment = 6;
pub const MINUTES: Moment = 60 / SECS_PER_BLOCK;
pub const HOURS: Moment = MINUTES * 60;
pub const DAYS: Moment = HOURS * 24;

pub struct CurrencyToVoteHandler;

impl CurrencyToVoteHandler {
    fn factor() -> u128 {
        (Balances::total_issuance() / u64::max_value() as u128).max(1)
    }
}

impl Convert<u128, u64> for CurrencyToVoteHandler {
    fn convert(x: u128) -> u64 {
        (x / Self::factor()) as u64
    }
}

impl Convert<u128, u128> for CurrencyToVoteHandler {
    fn convert(x: u128) -> u128 {
        x * Self::factor()
    }
}

impl system::Trait for Runtime {
    type Origin = Origin;
    type Index = Nonce;
    type BlockNumber = BlockNumber;
    type Hash = Hash;
    type Hashing = BlakeTwo256;
    type AccountId = AccountId;
    type Lookup = Indices;
    type Header = generic::Header<BlockNumber, BlakeTwo256>;
    type Event = Event;
}

impl aura::Trait for Runtime {
    type HandleReport = aura::StakingSlasher<Runtime>;
    type AuthorityId = AuraId;
}

impl indices::Trait for Runtime {
    type AccountIndex = AccountIndex;
    type IsDeadAccount = Balances;
    type ResolveHint = indices::SimpleResolveHint<Self::AccountId, Self::AccountIndex>;
    type Event = Event;
}

impl balances::Trait for Runtime {
    type Balance = Balance;
    type OnFreeBalanceZero = ((Staking, Contracts), Session);
    type OnNewAccount = Indices;
    type Event = Event;
    type TransactionPayment = DealWithFees;
    type DustRemoval = ();
    type TransferPayment = ();
    type ExistentialDeposit = ExistentialDeposit;
    type TransferFee = TransferFee;
    type CreationFee = CreationFee;
    type TransactionBaseFee = TransactionBaseFee;
    type TransactionByteFee = TransactionByteFee;
}

impl kton::Trait for Runtime {
    type Balance = Balance;
    type Event = Event;
    type OnMinted = ();
    type OnRemoval = ();
}

impl timestamp::Trait for Runtime {
    type Moment = u64;
    type OnTimestampSet = Aura;
}

parameter_types! {
    pub const ExistentialDeposit: Balance = 1 * MICRO;
    pub const TransferFee: Balance = 1 * MILLI;
    pub const CreationFee: Balance = 1 * MILLI;
    pub const TransactionBaseFee: Balance = 1 * MILLI;
    pub const TransactionByteFee: Balance = 1 * MICRO;
}

type SessionHandlers = (Grandpa, Aura);
parameter_types! {
    pub const UncleGenerations: u64 = 0;
}

impl_opaque_keys! {
    pub struct SessionKeys(grandpa::AuthorityId, AuraId);
}

impl authorship::Trait for Runtime {
    type FindAuthor = ();
    type UncleGenerations = UncleGenerations;
    type FilterUncle = ();
    type EventHandler = ();
}

// NOTE: `SessionHandler` and `SessionKeys` are co-dependent: One key will be used for each handler.
// The number and order of items in `SessionHandler` *MUST* be the same number and order of keys in
// `SessionKeys`.
// TODO: Introduce some structure to tie these together to make it a bit less of a footgun. This
// should be easy, since OneSessionHandler trait provides the `Key` as an associated type. #2858

parameter_types! {
    pub const Period: BlockNumber = 1 * MINUTES;
    pub const Offset: BlockNumber = 0;
}

impl session::Trait for Runtime {
    type OnSessionEnding = Staking;
    type SessionHandler = SessionHandlers;
    type ShouldEndSession = session::PeriodicSessions<Period, Offset>;
    type Event = Event;
    type Keys = SessionKeys;
}

parameter_types! {
    pub const SessionsPerEra: session::SessionIndex = 5;
    // about 14 days
    pub const BondingDuration: staking::EraIndex = 4032;
    // 365 days * 24 hours * 60 minutes / 5 minutes
    pub const ErasPerEpoch: EraIndex = 105120;
}

// customed
parameter_types! {
    // decimal 9
    pub const CAP: Balance = 10_000_000_000 * COIN;
}

impl staking::Trait for Runtime {
    type Ring = Balances;
    type Kton = Kton;
    type CurrencyToVote = CurrencyToVoteHandler;
    type Event = Event;
    type RingReward = ();
    type RingSlash = ();
    type KtonReward = ();
    type KtonSlash = ();
    type SessionsPerEra = SessionsPerEra;
    type BondingDuration = BondingDuration;
    // customed
    type Cap = CAP;
    type ErasPerEpoch = ErasPerEpoch;
}

parameter_types! {
    pub const SignedClaimHandicap: BlockNumber = 2;
    pub const TombstoneDeposit: Balance = 16;
    pub const StorageSizeOffset: u32 = 8;
    pub const RentByteFee: Balance = 4;
    pub const RentDepositOffset: Balance = 1000;
    pub const SurchargeReward: Balance = 150;
    pub const ContractTransferFee: Balance = 1 * MILLI;
    pub const ContractCreationFee: Balance = 1 * MILLI;
    pub const ContractTransactionBaseFee: Balance = 1 * MILLI;
    pub const ContractTransactionByteFee: Balance = 10 * NANO;
    pub const ContractFee: Balance = 1 * MILLI;
    pub const CallBaseFee: Gas = 1000;
    pub const CreateBaseFee: Gas = 1000;
    pub const MaxDepth: u32 = 1024;
    pub const BlockGasLimit: Gas = 10_000_000;
}

impl contracts::Trait for Runtime {
    type Currency = Balances;
    type Call = Call;
    type Event = Event;
    type DetermineContractAddress = contracts::SimpleAddressDeterminator<Runtime>;
    type ComputeDispatchFee = contracts::DefaultDispatchFeeComputor<Runtime>;
    type TrieIdGenerator = contracts::TrieIdFromParentCounter<Runtime>;
    type GasPayment = ();
    type SignedClaimHandicap = SignedClaimHandicap;
    type TombstoneDeposit = TombstoneDeposit;
    type StorageSizeOffset = StorageSizeOffset;
    type RentByteFee = RentByteFee;
    type RentDepositOffset = RentDepositOffset;
    type SurchargeReward = SurchargeReward;
    type TransferFee = ContractTransferFee;
    type CreationFee = ContractCreationFee;
    type TransactionBaseFee = ContractTransactionBaseFee;
    type TransactionByteFee = ContractTransactionByteFee;
    type ContractFee = ContractFee;
    type CallBaseFee = CallBaseFee;
    type CreateBaseFee = CreateBaseFee;
    type MaxDepth = MaxDepth;
    type BlockGasLimit = BlockGasLimit;
}

impl sudo::Trait for Runtime {
    type Event = Event;
    type Proposal = Call;
}

impl grandpa::Trait for Runtime {
    type Event = Event;
}

parameter_types! {
    pub const WindowSize: BlockNumber = DEFAULT_WINDOW_SIZE.into();
    pub const ReportLatency: BlockNumber = DEFAULT_REPORT_LATENCY.into();
}
impl finality_tracker::Trait for Runtime {
    type OnFinalizationStalled = Grandpa;
    type WindowSize = WindowSize;
    type ReportLatency = ReportLatency;
}

construct_runtime!(
	pub enum Runtime where
		Block = Block,
		NodeBlock = node_primitives::Block,
		UncheckedExtrinsic = UncheckedExtrinsic
	{
		System: system::{Module, Call, Storage, Config, Event},
		Aura: aura::{Module, Config<T>, Inherent(Timestamp)},
		Timestamp: timestamp::{Module, Call, Storage, Config<T>, Inherent},
		Authorship: authorship::{Module, Call, Storage},
		Indices: indices,
		Balances: balances,
		Kton: kton,
		Session: session::{Module, Call, Storage, Event, Config<T>},
		Staking: staking::{default, OfflineWorker},
		Contracts: contracts,
		FinalityTracker: finality_tracker::{Module, Call, Inherent},
		Grandpa: grandpa::{Module, Call, Storage, Config, Event},
		Sudo: sudo,
	}
);

/// The address format for describing accounts.
pub type Address = <Indices as StaticLookup>::Source;
/// Block header type as expected by this runtime.
pub type Header = generic::Header<BlockNumber, BlakeTwo256>;
/// Block type as expected by this runtime.
pub type Block = generic::Block<Header, UncheckedExtrinsic>;
/// A Block signed with a Justification
pub type SignedBlock = generic::SignedBlock<Block>;
/// BlockId type as expected by this runtime.
pub type BlockId = generic::BlockId<Block>;
/// Unchecked extrinsic type as expected by this runtime.
pub type UncheckedExtrinsic = generic::UncheckedMortalCompactExtrinsic<Address, Nonce, Call, Signature>;
/// Extrinsic type that has already been checked.
pub type CheckedExtrinsic = generic::CheckedExtrinsic<AccountId, Nonce, Call>;
/// Executive: handles dispatch to the various modules.
pub type Executive = executive::Executive<Runtime, Block, system::ChainContext<Runtime>, Balances, Runtime, AllModules>;

impl_runtime_apis! {
    impl client_api::Core<Block> for Runtime {
        fn version() -> RuntimeVersion {
            VERSION
        }

        fn execute_block(block: Block) {
            Executive::execute_block(block)
        }

        fn initialize_block(header: &<Block as BlockT>::Header) {
            Executive::initialize_block(header)
        }
    }

    impl client_api::Metadata<Block> for Runtime {
        fn metadata() -> OpaqueMetadata {
            Runtime::metadata().into()
        }
    }

    impl block_builder_api::BlockBuilder<Block> for Runtime {
        fn apply_extrinsic(extrinsic: <Block as BlockT>::Extrinsic) -> ApplyResult {
            Executive::apply_extrinsic(extrinsic)
        }

        fn finalize_block() -> <Block as BlockT>::Header {
            Executive::finalize_block()
        }

        fn inherent_extrinsics(data: InherentData) -> Vec<<Block as BlockT>::Extrinsic> {
            data.create_extrinsics()
        }

        fn check_inherents(block: Block, data: InherentData) -> CheckInherentsResult {
            data.check_extrinsics(&block)
        }

        fn random_seed() -> <Block as BlockT>::Hash {
            System::random_seed()
        }
    }

    impl client_api::TaggedTransactionQueue<Block> for Runtime {
        fn validate_transaction(tx: <Block as BlockT>::Extrinsic) -> TransactionValidity {
            Executive::validate_transaction(tx)
        }
    }

    impl offchain_primitives::OffchainWorkerApi<Block> for Runtime {
        fn offchain_worker(number: NumberFor<Block>) {
            Executive::offchain_worker(number)
        }
    }

    impl fg_primitives::GrandpaApi<Block> for Runtime {
        fn grandpa_pending_change(digest: &DigestFor<Block>)
            -> Option<ScheduledChange<NumberFor<Block>>>
        {
            Grandpa::pending_change(digest)
        }

        fn grandpa_forced_change(digest: &DigestFor<Block>)
            -> Option<(NumberFor<Block>, ScheduledChange<NumberFor<Block>>)>
        {
            Grandpa::forced_change(digest)
        }

        fn grandpa_authorities() -> Vec<(GrandpaId, GrandpaWeight)> {
            Grandpa::grandpa_authorities()
        }
    }

    impl consensus_aura::AuraApi<Block, AuraId> for Runtime {
        fn slot_duration() -> u64 {
            Aura::slot_duration()
        }
        fn authorities() -> Vec<AuraId> {
            Aura::authorities()
        }
    }
}
