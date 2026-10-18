pub(crate) mod cancel;
pub(crate) mod create;
pub(crate) mod emergency;
pub(crate) mod transfer;
pub(crate) mod update;
pub(crate) mod withdraw;

use crate::errors::Error;

/// Computes the deterministic floor integer square root $\lfloor\sqrt{n}\rfloor$
/// for non-negative `i128` token amounts using Newton's method (#1023).
pub(crate) const fn integer_sqrt_i128(n: i128) -> Result<i128, Error> {
    if n < 0 {
        return Err(Error::ValidationFailed);
    }
    if n < 2 {
        return Ok(n);
    }

    let mut x0 = n / 2;
    let mut x1 = (x0 + n / x0) / 2;
    while x1 < x0 {
        x0 = x1;
        x1 = (x0 + n / x0) / 2;
    }
    Ok(x0)
}

/// Calculates a campaign's Quadratic Funding (QF) weight $(\sum_i \lfloor\sqrt{c_i}\rfloor)^2$
/// from a slice of positive individual contribution amounts (#1023).
///
/// Rewards broad community support (many distinct contributors) over a single
/// whale contribution of the same aggregate value.
pub(crate) const fn calculate_quadratic_funding_weight(
    contributions: &[i128],
) -> Result<i128, Error> {
    let mut sum_roots: i128 = 0;
    let mut idx = 0usize;
    while idx < contributions.len() {
        let amount = contributions[idx];
        if amount <= 0 {
            return Err(Error::ValidationFailed);
        }
        let root = match integer_sqrt_i128(amount) {
            Ok(r) => r,
            Err(e) => return Err(e),
        };
        sum_roots = match sum_roots.checked_add(root) {
            Some(s) => s,
            None => return Err(Error::Overflow),
        };
        idx += 1;
    }

    match sum_roots.checked_mul(sum_roots) {
        Some(w) => Ok(w),
        None => Err(Error::Overflow),
    }
}

/// Calculates a campaign's matching pool payout share given its quadratic weight,
/// the sum of quadratic weights across all eligible campaigns in the round, and
/// the total `matching_pool` token balance (#1023).
pub(crate) const fn calculate_quadratic_matching_payout(
    campaign_qf_weight: i128,
    total_qf_weight: i128,
    matching_pool: i128,
) -> Result<i128, Error> {
    if campaign_qf_weight < 0
        || total_qf_weight <= 0
        || matching_pool < 0
        || campaign_qf_weight > total_qf_weight
    {
        return Err(Error::ValidationFailed);
    }
    if campaign_qf_weight == 0 || matching_pool == 0 {
        return Ok(0);
    }

    match campaign_qf_weight.checked_mul(matching_pool) {
        Some(numerator) => Ok(numerator / total_qf_weight),
        None => Err(Error::Overflow),
    }
}

const _: () = {
    assert!(integer_sqrt_i128(10_000).is_ok());
    assert!(calculate_quadratic_funding_weight(&[100, 100]).is_ok());
    assert!(calculate_quadratic_matching_payout(100, 400, 1_000).is_ok());
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quadratic_funding_favors_broad_contributor_base_over_single_whale() {
        // 10 contributors each donating 100 tokens (total 1_000):
        // sum of roots = 10 * 10 = 100 -> QF weight = 10_000
        let grassroots = [100i128; 10];
        let grassroots_weight = calculate_quadratic_funding_weight(&grassroots).unwrap();
        assert_eq!(grassroots_weight, 10_000);

        // 1 whale donating 1_000 tokens:
        // floor(sqrt(1000)) = 31 -> QF weight = 961
        let whale = [1_000i128];
        let whale_weight = calculate_quadratic_funding_weight(&whale).unwrap();
        assert_eq!(whale_weight, 961);
        assert!(grassroots_weight > whale_weight);

        let total_weight = grassroots_weight + whale_weight;
        let pool = 10_961i128;
        assert_eq!(
            calculate_quadratic_matching_payout(grassroots_weight, total_weight, pool),
            Ok(10_000)
        );
        assert_eq!(
            calculate_quadratic_matching_payout(whale_weight, total_weight, pool),
            Ok(961)
        );
    }

    #[test]
    fn quadratic_funding_rejects_invalid_inputs_and_overflows() {
        assert_eq!(integer_sqrt_i128(-1), Err(Error::ValidationFailed));
        assert_eq!(
            calculate_quadratic_funding_weight(&[100, 0]),
            Err(Error::ValidationFailed)
        );
        assert_eq!(
            calculate_quadratic_matching_payout(500, 400, 1_000),
            Err(Error::ValidationFailed)
        );
        assert_eq!(
            calculate_quadratic_matching_payout(100, 0, 1_000),
            Err(Error::ValidationFailed)
        );
    }
}
