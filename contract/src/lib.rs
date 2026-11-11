//! JustInCase: an "if something happens to me" vault on NEAR (testnet demo).
//!
//! States: Active -> Alert -> PendingRelease -> Released.
//! An owner check-in at any point before release returns the vault to Active.
//! Nobody (including the AI agent) can move funds except by following these rules.

use near_sdk::{
    env, log, near, require, AccountId, Gas, NearToken, PanicOnDefault, Promise, PromiseError,
};

const NS_PER_SEC: u64 = 1_000_000_000;
const NS_PER_MS: u64 = 1_000_000;
const MAX_PEOPLE: usize = 5;
const TOTAL_BPS: u32 = 10_000;
const CALLBACK_GAS_TGAS: u64 = 10;

#[near(serializers = [borsh, json])]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum VaultState {
    Active,
    Alert,
    PendingRelease,
    Released,
}

/// A beneficiary and their share in basis points (10000 = 100%).
#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Beneficiary {
    pub account_id: AccountId,
    pub bps: u16,
}

/// A payout that failed to send and can be retried.
#[near(serializers = [borsh, json])]
#[derive(Clone)]
pub struct Unpaid {
    pub account_id: AccountId,
    pub amount: NearToken,
}

/// Everything the agent and the web page need, in one view call.
#[near(serializers = [json])]
pub struct Status {
    pub state: VaultState,
    pub owner: AccountId,
    pub agent: AccountId,
    pub guardians: Vec<AccountId>,
    pub threshold: u8,
    pub confirmations: Vec<AccountId>,
    pub beneficiaries: Vec<Beneficiary>,
    pub checkin_interval_secs: u64,
    pub cooldown_secs: u64,
    pub last_checkin_ms: u64,
    pub next_checkin_due_ms: u64,
    pub overdue: bool,
    pub release_at_ms: u64,
    pub vault_balance: NearToken,
    pub payout_pool: NearToken,
    pub unpaid: Vec<Unpaid>,
    pub now_ms: u64,
}

#[near(contract_state)]
#[derive(PanicOnDefault)]
pub struct Vault {
    owner: AccountId,
    agent: AccountId,
    guardians: Vec<AccountId>,
    threshold: u8,
    beneficiaries: Vec<Beneficiary>,
    checkin_interval_ns: u64,
    cooldown_ns: u64,
    last_checkin_ns: u64,
    state: VaultState,
    confirmations: Vec<AccountId>,
    release_at_ns: u64,
    unpaid: Vec<Unpaid>,
}

fn secs_to_ns(secs: u64) -> u64 {
    require!(secs > 0, "Durations must be at least 1 second");
    secs.checked_mul(NS_PER_SEC)
        .unwrap_or_else(|| env::panic_str("Duration too large"))
}

/// Split `pool` by basis points. The last beneficiary takes the rounding remainder.
fn split_pool(pool: u128, beneficiaries: &[Beneficiary]) -> Vec<(AccountId, u128)> {
    let mut out = Vec::new();
    let mut paid: u128 = 0;
    let last = beneficiaries.len() - 1;
    for (i, b) in beneficiaries.iter().enumerate() {
        let amount = if i == last {
            pool - paid
        } else {
            pool * (b.bps as u128) / (TOTAL_BPS as u128)
        };
        paid += amount;
        out.push((b.account_id.clone(), amount));
    }
    out
}

#[near]
impl Vault {
    /// Deploy and initialise in the same transaction.
    /// Durations are in seconds so the demo can run in about two minutes.
    #[init]
    pub fn new(
        owner: AccountId,
        agent: AccountId,
        guardians: Vec<AccountId>,
        threshold: u8,
        beneficiaries: Vec<Beneficiary>,
        checkin_interval_secs: u64,
        cooldown_secs: u64,
    ) -> Self {
        require!(
            !guardians.is_empty() && guardians.len() <= MAX_PEOPLE,
            "Need 1 to 5 guardians"
        );
        require!(
            threshold >= 1 && (threshold as usize) <= guardians.len(),
            "Threshold must be between 1 and the number of guardians"
        );
        require!(!guardians.contains(&owner), "The owner cannot be a guardian");
        for (i, g) in guardians.iter().enumerate() {
            require!(!guardians[..i].contains(g), "Duplicate guardian");
        }
        require!(
            !beneficiaries.is_empty() && beneficiaries.len() <= MAX_PEOPLE,
            "Need 1 to 5 beneficiaries"
        );
        let mut total: u32 = 0;
        for (i, b) in beneficiaries.iter().enumerate() {
            require!(b.bps > 0, "Each beneficiary needs a share above zero");
            require!(
                !beneficiaries[..i].iter().any(|x| x.account_id == b.account_id),
                "Duplicate beneficiary"
            );
            total += b.bps as u32;
        }
        require!(
            total == TOTAL_BPS,
            "Beneficiary shares must add up to 100% (10000 basis points)"
        );
        Self {
            owner,
            agent,
            guardians,
            threshold,
            beneficiaries,
            checkin_interval_ns: secs_to_ns(checkin_interval_secs),
            cooldown_ns: secs_to_ns(cooldown_secs),
            last_checkin_ns: env::block_timestamp(),
            state: VaultState::Active,
            confirmations: Vec::new(),
            release_at_ns: 0,
            unpaid: Vec::new(),
        }
    }

    /// Anyone can add funds until the vault is released.
    #[payable]
    pub fn deposit(&mut self) {
        require!(self.state != VaultState::Released, "Vault already released");
        require!(
            env::attached_deposit().as_yoctonear() > 0,
            "Attach a deposit"
        );
        log!(
            "deposit: {} yoctoNEAR from {}",
            env::attached_deposit().as_yoctonear(),
            env::predecessor_account_id()
        );
    }

    /// Owner proves they are alive. Cancels any alert or pending release.
    pub fn check_in(&mut self) {
        require!(
            env::predecessor_account_id() == self.owner,
            "Only the owner can check in"
        );
        require!(self.state != VaultState::Released, "Vault already released");
        self.last_checkin_ns = env::block_timestamp();
        self.state = VaultState::Active;
        self.confirmations.clear();
        self.release_at_ns = 0;
        log!("check_in: vault is ACTIVE");
    }

    /// Owner changes the timers. Counts as a check-in. Only while ACTIVE.
    pub fn update_config(&mut self, checkin_interval_secs: u64, cooldown_secs: u64) {
        require!(
            env::predecessor_account_id() == self.owner,
            "Only the owner can change settings"
        );
        require!(
            self.state == VaultState::Active,
            "Settings can only change while the vault is ACTIVE"
        );
        self.checkin_interval_ns = secs_to_ns(checkin_interval_secs);
        self.cooldown_ns = secs_to_ns(cooldown_secs);
        self.last_checkin_ns = env::block_timestamp();
        log!("update_config: timers changed, check-in recorded");
    }

    /// Owner takes funds back out. Only while ACTIVE.
    pub fn withdraw(&mut self, amount: NearToken) -> Promise {
        require!(
            env::predecessor_account_id() == self.owner,
            "Only the owner can withdraw"
        );
        require!(
            self.state == VaultState::Active,
            "Withdrawals are only allowed while the vault is ACTIVE"
        );
        require!(
            amount.as_yoctonear() <= self.payout_pool().as_yoctonear(),
            "Amount exceeds the withdrawable balance"
        );
        Promise::new(self.owner.clone()).transfer(amount)
    }

    /// The agent or any guardian can raise the alert, but only if a check-in is overdue.
    pub fn raise_alert(&mut self) {
        let caller = env::predecessor_account_id();
        require!(
            caller == self.agent || self.guardians.contains(&caller),
            "Only the agent or a guardian can raise an alert"
        );
        require!(self.state == VaultState::Active, "Vault is not ACTIVE");
        require!(self.is_overdue(), "Check-in is not overdue yet");
        self.state = VaultState::Alert;
        self.confirmations.clear();
        log!("raise_alert: vault is ALERT (raised by {})", caller);
    }

    /// A guardian confirms. At the threshold the cooldown starts.
    pub fn confirm(&mut self) {
        let caller = env::predecessor_account_id();
        require!(self.guardians.contains(&caller), "Only a guardian can confirm");
        require!(self.state == VaultState::Alert, "Vault is not in ALERT");
        require!(!self.confirmations.contains(&caller), "Already confirmed");
        self.confirmations.push(caller.clone());
        log!(
            "confirm: {} ({}/{})",
            caller,
            self.confirmations.len(),
            self.threshold
        );
        if self.confirmations.len() >= self.threshold as usize {
            self.state = VaultState::PendingRelease;
            self.release_at_ns = env::block_timestamp().saturating_add(self.cooldown_ns);
            log!("vault is PENDING_RELEASE; cooldown started");
        }
    }

    /// Anyone can call this once the cooldown has passed with no check-in.
    pub fn release(&mut self) {
        require!(
            self.state == VaultState::PendingRelease,
            "Release is not pending"
        );
        require!(
            env::block_timestamp() >= self.release_at_ns,
            "Cooldown has not finished"
        );
        let pool = self.payout_pool().as_yoctonear();
        require!(pool > 0, "Nothing to release");

        // Change state BEFORE sending any money.
        self.state = VaultState::Released;

        for (account_id, amount) in split_pool(pool, &self.beneficiaries) {
            if amount > 0 {
                let _ = self.send_payout(account_id, NearToken::from_yoctonear(amount));
            }
        }
        log!("release: vault is RELEASED");
    }

    /// Anyone can retry payouts that failed (for example, an account that did not exist).
    pub fn retry_payout(&mut self, account_id: AccountId) {
        require!(self.state == VaultState::Released, "Vault is not released");
        let mut retry: Vec<Unpaid> = Vec::new();
        self.unpaid.retain(|u| {
            if u.account_id == account_id {
                retry.push(u.clone());
                false
            } else {
                true
            }
        });
        require!(!retry.is_empty(), "Nothing to retry for this account");
        for u in retry {
            let _ = self.send_payout(u.account_id, u.amount);
        }
    }

    /// Only this contract can call this. Records failed payouts. Never panics.
    #[private]
    pub fn on_payout(
        &mut self,
        account_id: AccountId,
        amount: NearToken,
        #[callback_result] result: Result<(), PromiseError>,
    ) {
        if result.is_err() {
            log!(
                "payout of {} yoctoNEAR to {} FAILED; recorded as unpaid",
                amount.as_yoctonear(),
                account_id
            );
            self.unpaid.push(Unpaid { account_id, amount });
        } else {
            log!(
                "payout of {} yoctoNEAR to {} sent",
                amount.as_yoctonear(),
                account_id
            );
        }
    }

    pub fn get_status(&self) -> Status {
        let due_ns = self.last_checkin_ns.saturating_add(self.checkin_interval_ns);
        Status {
            state: self.state,
            owner: self.owner.clone(),
            agent: self.agent.clone(),
            guardians: self.guardians.clone(),
            threshold: self.threshold,
            confirmations: self.confirmations.clone(),
            beneficiaries: self.beneficiaries.clone(),
            checkin_interval_secs: self.checkin_interval_ns / NS_PER_SEC,
            cooldown_secs: self.cooldown_ns / NS_PER_SEC,
            last_checkin_ms: self.last_checkin_ns / NS_PER_MS,
            next_checkin_due_ms: due_ns / NS_PER_MS,
            overdue: self.is_overdue(),
            release_at_ms: self.release_at_ns / NS_PER_MS,
            vault_balance: env::account_balance(),
            payout_pool: self.payout_pool(),
            unpaid: self.unpaid.clone(),
            now_ms: env::block_timestamp() / NS_PER_MS,
        }
    }
}

// Helpers (not exported as contract methods).
impl Vault {
    fn is_overdue(&self) -> bool {
        env::block_timestamp() > self.last_checkin_ns.saturating_add(self.checkin_interval_ns)
    }

    /// Balance minus the storage stake the account must keep, minus a small safety buffer.
    fn payout_pool(&self) -> NearToken {
        let balance = env::account_balance().as_yoctonear();
        let storage_reserve =
            (env::storage_usage() as u128) * env::storage_byte_cost().as_yoctonear();
        let buffer = NearToken::from_millinear(10).as_yoctonear();
        NearToken::from_yoctonear(balance.saturating_sub(storage_reserve + buffer))
    }

    fn send_payout(&self, to: AccountId, amount: NearToken) -> Promise {
        Promise::new(to.clone()).transfer(amount).then(
            Self::ext(env::current_account_id())
                .with_static_gas(Gas::from_tgas(CALLBACK_GAS_TGAS))
                .on_payout(to, amount),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use near_sdk::test_utils::VMContextBuilder;
    use near_sdk::testing_env;

    fn acc(name: &str) -> AccountId {
        name.parse().unwrap()
    }

    fn set_ctx(caller: &str, secs: u64) {
        let mut b = VMContextBuilder::new();
        b.current_account_id(acc("vault.testnet"))
            .predecessor_account_id(acc(caller))
            .block_timestamp(secs * NS_PER_SEC)
            .account_balance(NearToken::from_near(10));
        testing_env!(b.build());
    }

    fn ben(name: &str, bps: u16) -> Beneficiary {
        Beneficiary {
            account_id: acc(name),
            bps,
        }
    }

    // owner, agent, guardians g1 g2 g3 (2 of 3), b1 60% / b2 40%, interval 10s, cooldown 5s
    fn setup() -> Vault {
        set_ctx("deployer.testnet", 0);
        Vault::new(
            acc("owner.testnet"),
            acc("agent.testnet"),
            vec![acc("g1.testnet"), acc("g2.testnet"), acc("g3.testnet")],
            2,
            vec![ben("b1.testnet", 6000), ben("b2.testnet", 4000)],
            10,
            5,
        )
    }

    // Brings the vault to PENDING_RELEASE (release possible from t=18).
    fn to_pending(v: &mut Vault) {
        set_ctx("agent.testnet", 11);
        v.raise_alert();
        set_ctx("g1.testnet", 12);
        v.confirm();
        set_ctx("g2.testnet", 13);
        v.confirm();
    }

    #[test]
    fn full_release_flow() {
        let mut v = setup();
        assert_eq!(v.get_status().state, VaultState::Active);
        set_ctx("agent.testnet", 11);
        v.raise_alert();
        assert_eq!(v.get_status().state, VaultState::Alert);
        set_ctx("g1.testnet", 12);
        v.confirm();
        assert_eq!(v.get_status().state, VaultState::Alert);
        set_ctx("g2.testnet", 13);
        v.confirm();
        assert_eq!(v.get_status().state, VaultState::PendingRelease);
        set_ctx("anyone.testnet", 18);
        v.release();
        assert_eq!(v.get_status().state, VaultState::Released);
    }

    #[test]
    fn guardian_can_raise_alert() {
        let mut v = setup();
        set_ctx("g3.testnet", 11);
        v.raise_alert();
        assert_eq!(v.get_status().state, VaultState::Alert);
    }

    #[test]
    #[should_panic(expected = "Cooldown has not finished")]
    fn cooldown_is_enforced() {
        let mut v = setup();
        to_pending(&mut v);
        set_ctx("anyone.testnet", 17);
        v.release();
    }

    #[test]
    #[should_panic(expected = "Release is not pending")]
    fn owner_check_in_cancels_everything() {
        let mut v = setup();
        to_pending(&mut v);
        set_ctx("owner.testnet", 14);
        v.check_in();
        let s = v.get_status();
        assert_eq!(s.state, VaultState::Active);
        assert!(s.confirmations.is_empty());
        set_ctx("anyone.testnet", 30);
        v.release();
    }

    #[test]
    #[should_panic(expected = "Check-in is not overdue yet")]
    fn alert_requires_overdue() {
        let mut v = setup();
        set_ctx("agent.testnet", 5);
        v.raise_alert();
    }

    #[test]
    #[should_panic(expected = "Only the agent or a guardian")]
    fn stranger_cannot_raise_alert() {
        let mut v = setup();
        set_ctx("stranger.testnet", 11);
        v.raise_alert();
    }

    #[test]
    #[should_panic(expected = "Already confirmed")]
    fn double_confirm_rejected() {
        let mut v = setup();
        set_ctx("agent.testnet", 11);
        v.raise_alert();
        set_ctx("g1.testnet", 12);
        v.confirm();
        v.confirm();
    }

    #[test]
    #[should_panic(expected = "Only a guardian can confirm")]
    fn non_guardian_cannot_confirm() {
        let mut v = setup();
        set_ctx("agent.testnet", 11);
        v.raise_alert();
        set_ctx("agent.testnet", 12);
        v.confirm();
    }

    #[test]
    #[should_panic(expected = "add up to 100%")]
    fn shares_must_total_100_percent() {
        set_ctx("deployer.testnet", 0);
        Vault::new(
            acc("owner.testnet"),
            acc("agent.testnet"),
            vec![acc("g1.testnet")],
            1,
            vec![ben("b1.testnet", 5000), ben("b2.testnet", 4000)],
            10,
            5,
        );
    }

    #[test]
    #[should_panic(expected = "owner cannot be a guardian")]
    fn owner_cannot_be_guardian() {
        set_ctx("deployer.testnet", 0);
        Vault::new(
            acc("owner.testnet"),
            acc("agent.testnet"),
            vec![acc("owner.testnet")],
            1,
            vec![ben("b1.testnet", 10000)],
            10,
            5,
        );
    }

    #[test]
    fn split_pays_out_everything_with_remainder_to_last() {
        let b = vec![
            ben("b1.testnet", 3333),
            ben("b2.testnet", 3333),
            ben("b3.testnet", 3334),
        ];
        let out = split_pool(1_000_000_007, &b);
        let sum: u128 = out.iter().map(|(_, a)| *a).sum();
        assert_eq!(out.len(), 3);
        assert_eq!(sum, 1_000_000_007);
    }

    #[test]
    fn payout_pool_keeps_storage_reserve() {
        let v = setup();
        let s = v.get_status();
        assert!(s.payout_pool.as_yoctonear() < s.vault_balance.as_yoctonear());
        assert!(s.payout_pool.as_yoctonear() > 0);
    }
}