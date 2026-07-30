// Inside create_campaign function or validation module
let category_cap_key = DataKey::CategoryMaxGoalCap(campaign_category.clone());
if let Some(max_cap) = env.storage().persistent().get::<DataKey, i128>(&category_cap_key) {
    if funding_goal > max_cap {
        return Err(Error::FundingGoalExceedsCategoryCap);
    }
}