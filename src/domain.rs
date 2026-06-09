use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use utoipa::ToSchema;

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct FundRecord {
    pub account_id: String,
    pub description: String,
    pub state: FundState,
    pub events: Vec<FundEvent>,
}

impl FundRecord {
    pub fn new(account_id: String, description: String) -> Self {
        Self {
            account_id: account_id.clone(),
            description: description.clone(),
            state: FundState::new(account_id, description),
            events: Vec::new(),
        }
    }

    pub fn append(&mut self, event: FundEvent) {
        self.state.apply(&event);
        self.events.push(event);
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreateFundRequest {
    pub account_id: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FundEvent {
    pub updated_at: DateTime<Utc>,
    pub comment: Option<String>,
    pub fund_equity: Option<FundEquity>,
    pub order: Option<InvestorOrder>,
    pub investor: Option<InvestorUpdate>,
    pub taxation: Option<TaxationKind>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct FundEquity {
    pub equity: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct InvestorOrder {
    pub name: String,
    pub deposit: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
pub struct InvestorUpdate {
    pub name: String,
    pub tax_rate: Option<f64>,
    pub add_tax_threshold: Option<f64>,
    pub referrer: Option<String>,
    pub referrer_rebate_rate: Option<f64>,
}

#[derive(Clone, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum TaxationKind {
    Legacy,
    PreserveFundAssets,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct FundState {
    pub account_id: String,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub description: String,
    pub total_assets: f64,
    pub total_taxed: f64,
    pub summary: FundSummary,
    pub investors: BTreeMap<String, InvestorMeta>,
    pub investor_derived: BTreeMap<String, InvestorDerived>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct FundSummary {
    pub total_deposit: f64,
    pub total_share: f64,
    pub total_tax: f64,
    pub unit_price: f64,
    pub total_profit: f64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct InvestorMeta {
    pub name: String,
    pub share: f64,
    pub tax_threshold: f64,
    pub deposit: f64,
    pub tax_rate: f64,
    pub avg_cost_price: f64,
    pub referrer: Option<String>,
    pub referrer_rebate_rate: f64,
    pub claimed_referrer_rebate: f64,
    pub taxed: f64,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct InvestorDerived {
    pub pre_tax_assets: f64,
    pub taxable: f64,
    pub tax: f64,
    pub after_tax_assets: f64,
    pub after_tax_profit: f64,
    pub after_tax_share: f64,
    pub floating_profit_rate: f64,
    pub share_ratio: f64,
}

impl FundState {
    fn new(account_id: String, description: String) -> Self {
        Self {
            account_id,
            created_at: None,
            updated_at: None,
            description,
            total_assets: 0.0,
            total_taxed: 0.0,
            summary: FundSummary {
                total_deposit: 0.0,
                total_share: 0.0,
                total_tax: 0.0,
                unit_price: 1.0,
                total_profit: 0.0,
            },
            investors: BTreeMap::new(),
            investor_derived: BTreeMap::new(),
        }
    }

    fn apply(&mut self, event: &FundEvent) {
        self.created_at.get_or_insert(event.updated_at);
        self.updated_at = Some(event.updated_at);

        if let Some(comment) = &event.comment {
            self.description = comment.clone();
        }

        if let Some(fund_equity) = &event.fund_equity {
            self.total_assets = fund_equity.equity;
        }

        if let Some(order) = &event.order {
            self.apply_order(order, event.updated_at);
        }

        if let Some(investor) = &event.investor {
            self.apply_investor_update(investor, event.updated_at);
        }

        if let Some(taxation) = &event.taxation {
            self.apply_taxation(taxation, event.updated_at);
        }

        self.recalculate();
    }

    fn apply_order(&mut self, order: &InvestorOrder, updated_at: DateTime<Utc>) {
        let unit_price = self.summary.unit_price;
        let investor = ensure_investor(&mut self.investors, &order.name, updated_at);
        let share = order.deposit / unit_price;

        if share > 0.0 {
            investor.avg_cost_price = (investor.avg_cost_price * investor.share + order.deposit)
                / (investor.share + share);
        }

        investor.deposit += order.deposit;
        investor.tax_threshold += order.deposit;
        investor.share += share;
        self.total_assets += order.deposit;
    }

    fn apply_investor_update(&mut self, update: &InvestorUpdate, updated_at: DateTime<Utc>) {
        let investor = ensure_investor(&mut self.investors, &update.name, updated_at);

        if let Some(tax_rate) = update.tax_rate {
            investor.tax_rate = tax_rate;
        }

        if let Some(add_tax_threshold) = update.add_tax_threshold {
            investor.tax_threshold += add_tax_threshold;
        }

        if let Some(referrer) = &update.referrer {
            investor.referrer = Some(referrer.clone());
        }

        if let Some(referrer_rebate_rate) = update.referrer_rebate_rate {
            investor.referrer_rebate_rate = referrer_rebate_rate;
        }
    }

    fn apply_taxation(&mut self, taxation: &TaxationKind, updated_at: DateTime<Utc>) {
        let snapshot = self.investor_derived.clone();

        match taxation {
            TaxationKind::Legacy => {
                for investor in self.investors.values_mut() {
                    let Some(derived) = snapshot.get(&investor.name) else {
                        continue;
                    };
                    investor.share = derived.after_tax_share;
                    investor.tax_threshold = derived.after_tax_assets;
                    self.total_assets -= derived.tax;
                    self.total_taxed += derived.tax;
                }
            }
            TaxationKind::PreserveFundAssets => {
                let investor_names: BTreeSet<String> = self.investors.keys().cloned().collect();
                let mut total_tax_share = 0.0;
                let mut rebates = Vec::new();

                for investor in self.investors.values_mut() {
                    let Some(derived) = snapshot.get(&investor.name) else {
                        continue;
                    };
                    let tax_share = investor.share - derived.after_tax_share;
                    let rebate_share = investor.referrer.as_ref().map_or(0.0, |referrer| {
                        calculate_rebate_share(
                            referrer,
                            &investor_names,
                            tax_share,
                            investor.referrer_rebate_rate,
                        )
                    });

                    if rebate_share != 0.0
                        && let Some(referrer) = &investor.referrer
                    {
                        rebates.push((referrer.clone(), rebate_share));
                    }

                    investor.share = derived.after_tax_share;
                    investor.tax_threshold = derived.after_tax_assets;
                    investor.taxed += derived.tax;
                    total_tax_share += tax_share - rebate_share;
                    self.total_taxed += derived.tax;
                }

                let unit_price = self.summary.unit_price;
                for (referrer, rebate_share) in rebates {
                    let rebate_value = rebate_share * unit_price;
                    let investor = ensure_investor(&mut self.investors, &referrer, updated_at);
                    investor.share += rebate_share;
                    investor.tax_threshold += rebate_value;
                    investor.claimed_referrer_rebate += rebate_value;
                }

                let tax_account = ensure_investor(&mut self.investors, "@tax", updated_at);
                tax_account.share += total_tax_share;
                tax_account.tax_threshold += total_tax_share * unit_price;
            }
        }
    }

    fn recalculate(&mut self) {
        self.summary.total_share = self.investors.values().map(|investor| investor.share).sum();
        self.summary.unit_price = unit_price(self.total_assets, self.summary.total_share);

        self.investor_derived = self
            .investors
            .values()
            .map(|investor| {
                let derived =
                    derive_investor(investor, self.summary.unit_price, self.summary.total_share);
                (investor.name.clone(), derived)
            })
            .collect();

        self.summary.total_deposit = self
            .investors
            .values()
            .map(|investor| investor.deposit)
            .sum();
        self.summary.total_tax = self
            .investor_derived
            .values()
            .map(|derived| derived.tax)
            .sum();
        self.summary.total_profit =
            self.total_assets - self.summary.total_deposit + self.total_taxed;
    }
}

fn ensure_investor<'a>(
    investors: &'a mut BTreeMap<String, InvestorMeta>,
    name: &str,
    created_at: DateTime<Utc>,
) -> &'a mut InvestorMeta {
    investors
        .entry(name.to_owned())
        .or_insert_with(|| InvestorMeta {
            name: name.to_owned(),
            share: 0.0,
            tax_threshold: 0.0,
            deposit: 0.0,
            tax_rate: 0.0,
            avg_cost_price: 1.0,
            referrer: None,
            referrer_rebate_rate: 0.0,
            claimed_referrer_rebate: 0.0,
            taxed: 0.0,
            created_at,
        })
}

fn unit_price(total_assets: f64, total_share: f64) -> f64 {
    if total_share == 0.0 {
        return 1.0;
    }

    total_assets / total_share
}

fn derive_investor(investor: &InvestorMeta, unit_price: f64, total_share: f64) -> InvestorDerived {
    let pre_tax_assets = investor.share * unit_price;
    let taxable = pre_tax_assets - investor.tax_threshold;
    let tax = taxable.max(0.0) * investor.tax_rate;
    let after_tax_assets = pre_tax_assets - tax;
    let after_tax_profit = after_tax_assets - investor.deposit;
    let after_tax_share = after_tax_assets / unit_price;

    InvestorDerived {
        pre_tax_assets,
        taxable,
        tax,
        after_tax_assets,
        after_tax_profit,
        after_tax_share,
        floating_profit_rate: unit_price / investor.avg_cost_price - 1.0,
        share_ratio: share_ratio(investor.share, total_share),
    }
}

fn share_ratio(share: f64, total_share: f64) -> f64 {
    if total_share == 0.0 {
        return 0.0;
    }

    share / total_share
}

fn calculate_rebate_share(
    referrer: &str,
    investor_names: &BTreeSet<String>,
    tax_share: f64,
    rate: f64,
) -> f64 {
    if !investor_names.contains(referrer) {
        return 0.0;
    }

    tax_share * rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn order_creates_investor_share() {
        let updated_at = "2026-06-09T00:00:00Z".parse().unwrap();
        let mut record = FundRecord::new("fund/main".to_owned(), "Main fund".to_owned());

        record.append(FundEvent {
            updated_at,
            comment: None,
            fund_equity: None,
            order: Some(InvestorOrder {
                name: "Alice".to_owned(),
                deposit: 100.0,
            }),
            investor: None,
            taxation: None,
        });

        assert_eq!(record.state.total_assets, 100.0);
        assert_eq!(record.state.summary.total_share, 100.0);
        assert_eq!(record.state.investors["Alice"].share, 100.0);
    }

    #[test]
    fn tax_event_moves_tax_to_tax_account() {
        let updated_at = "2026-06-09T00:00:00Z".parse().unwrap();
        let mut record = FundRecord::new("fund/main".to_owned(), "Main fund".to_owned());

        record.append(FundEvent {
            updated_at,
            comment: None,
            fund_equity: None,
            order: Some(InvestorOrder {
                name: "Alice".to_owned(),
                deposit: 100.0,
            }),
            investor: Some(InvestorUpdate {
                name: "Alice".to_owned(),
                tax_rate: Some(0.2),
                add_tax_threshold: None,
                referrer: None,
                referrer_rebate_rate: None,
            }),
            taxation: None,
        });
        record.append(FundEvent {
            updated_at,
            comment: None,
            fund_equity: Some(FundEquity { equity: 200.0 }),
            order: None,
            investor: None,
            taxation: None,
        });
        record.append(FundEvent {
            updated_at,
            comment: None,
            fund_equity: None,
            order: None,
            investor: None,
            taxation: Some(TaxationKind::PreserveFundAssets),
        });

        assert_eq!(record.state.total_assets, 200.0);
        assert_eq!(record.state.investors["@tax"].share, 10.0);
        assert_eq!(record.state.investors["Alice"].taxed, 20.0);
    }
}
