# Authorization Matrix

This contract uses Soroban `Address::require_auth()` checks to ensure only the correct party can call state-changing methods.

> Note: Some functions take an `Address` argument (e.g. `contributor`, `voter`, `admin`) and require that address to authorize the call. Others derive the authorized address from contract storage (e.g. `get_admin`) or from campaign state (e.g. `campaign.creator`).

| Public method | Who must authorize |
| --- | --- |
| `init(admin, ...)` | `admin` |
| `create_campaign(creator, ...)` | `creator` |
| `contribute(..., contributor, ...)` | `contributor` |
| `withdraw_funds(campaign_id)` | `campaign.creator` |
| `cancel_campaign(campaign_id)` | `campaign.creator` |
| `update_campaign(campaign_id, ...)` | `campaign.creator` |
| `update_campaign_description(campaign_id, ...)` | `campaign.creator` |
| `claim_refund(..., contributor)` | `contributor` |
| `deposit_revenue(campaign_id, ...)` | `campaign.creator` |
| `claim_revenue(..., contributor)` | `contributor` |
| `claim_creator_revenue(campaign_id)` | `campaign.creator` |
| `set_voting_params(admin, ...)` | `admin` |
| `pause(admin)` | `admin` (must match stored admin) |
| `unpause(admin)` | `admin` (must match stored admin) |
| `vote_on_campaign(..., voter, ...)` | `voter` |
| `verify_campaign(campaign_id)` | stored `admin` (from `get_admin`) |
| `verify_campaign_with_votes(campaign_id)` | no explicit auth (anyone) |
| `get_campaign(...)` | no auth |
| `get_campaign_optional(...)` | no auth |
| `get_campaign_count()` | no auth |
| `get_contribution(..., contributor)` | no auth |
| `get_revenue_pool(...)` | no auth |
| `get_revenue_claimed(..., contributor)` | no auth |
| `get_version()` | no auth |
| `update_platform_fee(new_fee)` | stored `admin` (from `get_admin`) |
| `set_min_campaign_funding_goal(admin, min_goal)` | stored `admin` (and `admin` must match stored admin) |
| `update_admin(admin, new_admin)` | stored `admin` (and `admin` must match stored admin) |
| `get_approve_votes(...)` | no auth |
| `get_reject_votes(...)` | no auth |
| `has_voted(..., voter)` | no auth |
| `get_min_votes_quorum()` | no auth |
| `get_approval_threshold_bps()` | no auth |
| `get_admin()` | no auth |
| `get_token()` | no auth |
| `get_platform_fee()` | no auth |
| `get_min_campaign_funding_goal()` | no auth |
| `list_campaigns(...)` | no auth |
| `list_active_campaigns(...)` | no auth |
| `initiate_campaign_transfer(campaign_id, new_creator)` | `campaign.creator` |
| `accept_campaign_transfer(campaign_id)` | pending creator (from `campaign.pending_creator`) |
| `cancel_campaign_transfer(campaign_id)` | `campaign.creator` |

