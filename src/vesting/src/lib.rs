use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol};

const CLIFF_REACHED: Symbol = symbol_short!("CliffReached");

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VestingSchedule {
    pub beneficiary: Address,
    pub start_time: u64,
    pub cliff_time: u64,
    pub end_time: u64,
    pub total_amount: i128,
    pub claimed_amount: i128,
}

#[contract]
pub struct VestingContract;

#[contractimpl]
impl VestingContract {
    /// Create a new vesting schedule for a beneficiary.
    ///
    /// The schedule must satisfy `start_time <= cliff_time <= end_time`.
    pub fn create_schedule(
        env: Env,
        beneficiary: Address,
        start_time: u64,
        cliff_time: u64,
        end_time: u64,
        total_amount: i128,
    ) -> VestingSchedule {
        Self::validate_schedule(start_time, cliff_time, end_time);

        VestingSchedule {
            beneficiary,
            start_time,
            cliff_time,
            end_time,
            total_amount,
            claimed_amount: 0,
        }
    }

    /// Calculate the amount of tokens vested at `current_timestamp`.
    ///
    /// Returns `0` for any timestamp strictly before the cliff. Once the cliff
    /// is reached, tokens vest linearly between `cliff_time` and `end_time`.
    pub fn calculate_vested_amount(
        env: Env,
        schedule: VestingSchedule,
        current_timestamp: u64,
    ) -> i128 {
        Self::validate_schedule(schedule.start_time, schedule.cliff_time, schedule.end_time);

        // Strict cliff enforcement: nothing is claimable before the cliff.
        if current_timestamp < schedule.cliff_time {
            return 0;
        }

        if current_timestamp >= schedule.end_time {
            return schedule.total_amount;
        }

        let elapsed = current_timestamp - schedule.cliff_time;
        let duration = schedule.end_time - schedule.cliff_time;

        if duration == 0 {
            return schedule.total_amount;
        }

        schedule.total_amount * (elapsed as i128) / (duration as i128)
    }

    /// Claim vested tokens for the beneficiary.
    ///
    /// Emits a `CliffReached` event when the claim happens at or just after the
    /// cliff timestamp (i.e. the first claimable moment).
    pub fn claim(env: Env, schedule: VestingSchedule, current_timestamp: u64) -> i128 {
        Self::validate_schedule(schedule.start_time, schedule.cliff_time, schedule.end_time);

        let vested = Self::calculate_vested_amount(env.clone(), schedule.clone(), current_timestamp);
        let claimable = vested - schedule.claimed_amount;

        if claimable <= 0 {
            return 0;
        }

        // Emit CliffReached when the claim occurs right after the cliff timestamp.
        if current_timestamp >= schedule.cliff_time
            && current_timestamp <= schedule.cliff_time.saturating_add(1)
        {
            env.events().publish(
                (CLIFF_REACHED, schedule.beneficiary.clone()),
                current_timestamp,
            );
        }

        claimable
    }

    fn validate_schedule(start_time: u64, cliff_time: u64, end_time: u64) {
        assert!(start_time <= cliff_time, "start_time must be <= cliff_time");
        assert!(cliff_time <= end_time, "cliff_time must be <= end_time");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use soroban_sdk::testutils::Address as _;
    use soroban_sdk::Env;

    fn schedule(env: &Env, start: u64, cliff: u64, end: u64) -> VestingSchedule {
        VestingSchedule {
            beneficiary: Address::generate(env),
            start_time: start,
            cliff_time: cliff,
            end_time: end,
            total_amount: 1_000,
            claimed_amount: 0,
        }
    }

    #[test]
    fn test_no_vesting_one_second_before_cliff() {
        let env = Env::default();
        let s = schedule(&env, 0, 100, 200);
        let vested = VestingContract::calculate_vested_amount(env.clone(), s, 99);
        assert_eq!(vested, 0);
    }

    #[test]
    fn test_zero_vesting_exactly_at_cliff() {
        let env = Env::default();
        let s = schedule(&env, 0, 100, 200);
        let vested = VestingContract::calculate_vested_amount(env.clone(), s, 100);
        assert_eq!(vested, 0);
    }

    #[test]
    fn test_vesting_after_cliff() {
        let env = Env::default();
        let s = schedule(&env, 0, 100, 200);
        let vested = VestingContract::calculate_vested_amount(env.clone(), s, 150);
        assert_eq!(vested, 500);
    }

    #[test]
    fn test_full_vesting_at_end() {
        let env = Env::default();
        let s = schedule(&env, 0, 100, 200);
        let vested = VestingContract::calculate_vested_amount(env.clone(), s, 200);
        assert_eq!(vested, 1_000);
    }

    #[test]
    #[should_panic(expected = "start_time must be <= cliff_time")]
    fn test_invalid_schedule_start_after_cliff() {
        let env = Env::default();
        let s = schedule(&env, 150, 100, 200);
        VestingContract::calculate_vested_amount(env.clone(), s, 150);
    }

    #[test]
    #[should_panic(expected = "cliff_time must be <= end_time")]
    fn test_invalid_schedule_cliff_after_end() {
        let env = Env::default();
        let s = schedule(&env, 0, 250, 200);
        VestingContract::calculate_vested_amount(env.clone(), s, 250);
    }
}
