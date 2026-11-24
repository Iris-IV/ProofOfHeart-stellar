//! Stateful property coverage for the platform-token raised total (#1464).

extern crate std;

use proptest::prelude::*;

use super::helpers::*;
use crate::{Category, CreateCampaignParams};

proptest! {
    #![proptest_config(ProptestConfig::with_cases(20))]

    #[test]
    fn prop_global_total_matches_live_platform_token_escrow(ops in prop::collection::vec(any::<(u8, u8, u16)>(), 0..40)) {
        let (env, _admin, _, contributor1, contributor2, _token, token_admin, client) = setup_env();
        token_admin.mint(&contributor1, &10_000_000);
        token_admin.mint(&contributor2, &10_000_000);

        let contributors = [contributor1, contributor2];
        let mut campaign_ids = std::vec::Vec::new();

        for (operation, campaign_selector, amount_raw) in ops {
            match operation % 6 {
                0 => {
                    let creator = Address::generate(&env);
                    let title = std::format!("Property campaign {}", campaign_ids.len());
                    let params = CreateCampaignParams {
                        creator,
                        title: String::from_str(&env, &title),
                        description: String::from_str(&env, "Raised total property"),
                        funding_goal: 100_000,
                        duration_days: 30,
                        category: Category::Learner,
                        has_revenue_sharing: false,
                        revenue_share_percentage: 0,
                        max_contribution_per_user: 0,
                    };
                    if let Ok(Ok(id)) = client.try_create_campaign(&params) {
                        campaign_ids.push(id);
                    }
                }
                operation => {
                    if !campaign_ids.is_empty() {
                        let campaign_id =
                            campaign_ids[campaign_selector as usize % campaign_ids.len()];
                        let contributor = &contributors[campaign_selector as usize % contributors.len()];
                        match operation % 6 {
                            1 => {
                                let _ = client.try_verify_campaign(&campaign_id);
                            }
                            2 => {
                                let amount = (amount_raw as i128 % 50_000) + 1;
                                let _ = client.try_contribute(&campaign_id, contributor, &amount);
                            }
                            3 => {
                                let _ = client.try_cancel_campaign(&campaign_id);
                            }
                            4 => {
                                let _ = client.try_claim_refund(&campaign_id, contributor);
                            }
                            _ => {
                                advance_past_deadline(&env, &client, campaign_id);
                                let _ = client.try_withdraw_funds(&campaign_id);
                            }
                        }
                    }
                }
            }

            let expected: i128 = campaign_ids
                .iter()
                .map(|id| {
                    let campaign = client.get_campaign(id);
                    if campaign.funds_withdrawn() {
                        0
                    } else {
                        campaign.effective_amount_raised
                    }
                })
                .sum();
            prop_assert_eq!(
                client.get_total_raised_global(),
                expected,
                "global total diverged"
            );
        }
    }
}
