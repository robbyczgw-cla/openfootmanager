use crate::game::Game;
use chrono::{Datelike, NaiveDate};
use domain::message::*;
use domain::team::{
    BankLoan, FinancialTransaction, FinancialTransactionKind, Sponsorship,
    SponsorshipBonusCriterion, Team,
};
use rand::RngExt;
use serde::Serialize;

const BOARD_SUPPORT_MIN_AMOUNT: i64 = 150_000;
const BOARD_SUPPORT_MAX_AMOUNT: i64 = 1_000_000;
const BOARD_SUPPORT_TARGET_RUNWAY_WEEKS: i64 = 8;
const BOARD_SUPPORT_SATISFACTION_PENALTY: u8 = 12;
const FINANCE_WARNING_SATISFACTION_PENALTY: u8 = 2;
const FINANCE_CRITICAL_SATISFACTION_PENALTY: u8 = 4;
const MARKETING_CAMPAIGN_COOLDOWN_DAYS: i64 = 28;
const MARKETING_CAMPAIGN_MIN_GROSS_REVENUE: i64 = 60_000;
const MARKETING_CAMPAIGN_MAX_GROSS_REVENUE: i64 = 250_000;
const MARKETING_CAMPAIGN_MIN_COST: i64 = 15_000;
const SPONSOR_PITCH_DURATION_WEEKS: u32 = 12;
const SPONSOR_PITCH_MIN_WEEKLY_AMOUNT: i64 = 40_000;
const SPONSOR_PITCH_MAX_WEEKLY_AMOUNT: i64 = 180_000;
const SPONSOR_PITCH_REPUTATION_MULTIPLIER: i64 = 120;
const MERCHANDISE_MIN_WEEKLY_INCOME: i64 = 500;
const MERCHANDISE_MAX_WEEKLY_INCOME: i64 = 75_000;
const MERCHANDISE_REPUTATION_MULTIPLIER: i64 = 6;
const MERCHANDISE_WIN_FORM_BONUS: i64 = 400;
const MERCHANDISE_NEUTRAL_FAN_APPROVAL: u8 = 50;
const BANK_LOAN_TERM_WEEKS: u32 = 26;
const BANK_LOAN_MIN_PRINCIPAL: i64 = 100_000;
const BANK_LOAN_MAX_PRINCIPAL: i64 = 2_000_000;
const BANK_LOAN_REPUTATION_MULTIPLIER: i64 = 500;

fn marketing_campaign_activation_description() -> String {
    ["Marketing", "campaign", "activation", "spend"].join(" ")
}

fn marketing_campaign_revenue_description() -> String {
    ["Marketing", "campaign", "merchandise", "revenue"].join(" ")
}

fn merchandise_income_description() -> String {
    ["Weekly", "merchandise", "sales", "income"].join(" ")
}

fn bank_loan_drawdown_description() -> String {
    ["Bank", "loan", "drawdown"].join(" ")
}

fn bank_loan_repayment_description() -> String {
    ["Bank", "loan", "weekly", "repayment"].join(" ")
}

fn bank_loan_early_settlement_description() -> String {
    ["Bank", "loan", "early", "settlement"].join(" ")
}

fn board_support_description(season: u32) -> String {
    format!(
        "{} {}",
        ["Board", "support", "package", "for", "season"].join(" "),
        season
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FinanceHealthLevel {
    Stable,
    Watch,
    Warning,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct TeamFinanceSnapshot {
    pub annual_wage_bill: i64,
    pub weekly_wage_spend: i64,
    pub weekly_wage_budget: i64,
    pub weekly_recurring_income: i64,
    pub weekly_sponsor_income: i64,
    pub weekly_merchandise_income: i64,
    pub weekly_loan_repayment: i64,
    pub projected_weekly_net: i64,
    pub cash_runway_weeks: Option<i64>,
    pub wage_budget_usage_percent: u32,
    pub currently_in_debt: bool,
    pub currently_over_budget: bool,
    pub wage_budget_status: FinanceHealthLevel,
    pub runway_status: FinanceHealthLevel,
    pub overall_status: FinanceHealthLevel,
    pub marketing_campaign_cooldown_days_remaining: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BoardSupportResult {
    pub support_amount: i64,
    pub transfer_budget_reduction: i64,
    pub satisfaction_penalty: u8,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SponsorPitchPreview {
    pub sponsor_name: String,
    pub weekly_amount: i64,
    pub duration_weeks: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct MarketingCampaignPreview {
    pub gross_revenue: i64,
    pub campaign_cost: i64,
    pub net_income: i64,
    pub cooldown_days: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct BankLoanPreview {
    pub principal: i64,
    pub interest_rate_percent: u32,
    pub term_weeks: u32,
    pub weekly_repayment: i64,
    pub total_repayment: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct FinanceActionPreviews {
    pub board_support: Option<BoardSupportResult>,
    pub sponsor_pitch: Option<SponsorPitchPreview>,
    pub marketing_campaign: Option<MarketingCampaignPreview>,
    pub bank_loan: Option<BankLoanPreview>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SponsorPitchResult {
    pub message_id: String,
    pub sponsor_name: String,
    pub weekly_amount: i64,
    pub duration_weeks: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct MarketingCampaignResult {
    pub message_id: String,
    pub gross_revenue: i64,
    pub campaign_cost: i64,
    pub net_income: i64,
    pub cooldown_days: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BankLoanResult {
    pub message_id: String,
    pub principal: i64,
    pub interest_rate_percent: u32,
    pub term_weeks: u32,
    pub weekly_repayment: i64,
    pub total_repayment: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BankLoanRepaymentResult {
    pub message_id: String,
    pub amount_paid: i64,
}

fn wage_budget_status(usage_percent: u32) -> FinanceHealthLevel {
    if usage_percent > 110 {
        return FinanceHealthLevel::Critical;
    }

    if usage_percent > 100 {
        return FinanceHealthLevel::Warning;
    }

    if usage_percent >= 85 {
        return FinanceHealthLevel::Watch;
    }

    FinanceHealthLevel::Stable
}

fn runway_status(balance: i64, runway_weeks: Option<i64>) -> FinanceHealthLevel {
    if balance < 0 {
        return FinanceHealthLevel::Critical;
    }

    let Some(runway_weeks) = runway_weeks else {
        return FinanceHealthLevel::Stable;
    };

    if runway_weeks <= 4 {
        return FinanceHealthLevel::Critical;
    }

    if runway_weeks <= 8 {
        return FinanceHealthLevel::Warning;
    }

    if runway_weeks <= 12 {
        return FinanceHealthLevel::Watch;
    }

    FinanceHealthLevel::Stable
}

fn most_severe_level(left: FinanceHealthLevel, right: FinanceHealthLevel) -> FinanceHealthLevel {
    fn severity(level: FinanceHealthLevel) -> u8 {
        match level {
            FinanceHealthLevel::Stable => 0,
            FinanceHealthLevel::Watch => 1,
            FinanceHealthLevel::Warning => 2,
            FinanceHealthLevel::Critical => 3,
        }
    }

    if severity(left) >= severity(right) {
        left
    } else {
        right
    }
}

fn action(id: &str, label: &str, label_key: &str, action_type: ActionType) -> MessageAction {
    MessageAction {
        id: id.to_string(),
        label: label.to_string(),
        action_type,
        resolved: false,
        label_key: Some(label_key.to_string()),
    }
}

pub fn calc_wages(game: &Game, team_id: &str) -> i64 {
    let player_wages: i64 = game
        .players
        .iter()
        .filter(|player| player.team_id.as_deref() == Some(team_id))
        .map(|player| player.wage as i64 / 52)
        .sum();

    let staff_wages: i64 = game
        .staff
        .iter()
        .filter(|staff_member| staff_member.team_id.as_deref() == Some(team_id))
        .map(|staff_member| staff_member.wage as i64 / 52)
        .sum();

    player_wages + staff_wages
}

pub fn calc_annual_wages(game: &Game, team_id: &str) -> i64 {
    let player_wages: i64 = game
        .players
        .iter()
        .filter(|player| player.team_id.as_deref() == Some(team_id))
        .map(|player| player.wage as i64)
        .sum();

    let staff_wages: i64 = game
        .staff
        .iter()
        .filter(|staff_member| staff_member.team_id.as_deref() == Some(team_id))
        .map(|staff_member| staff_member.wage as i64)
        .sum();

    player_wages + staff_wages
}

pub fn calc_cash_runway_weeks(balance: i64, projected_weekly_net: i64) -> Option<i64> {
    if projected_weekly_net >= 0 {
        return None;
    }

    Some(std::cmp::max(0, balance / projected_weekly_net.abs()))
}

pub fn calc_matchday(
    stadium_capacity: u32,
    home_match_count: i64,
    attendance_pct: f64,
    avg_ticket: f64,
) -> i64 {
    let revenue_per_match = (stadium_capacity as f64 * attendance_pct * avg_ticket) as i64;

    revenue_per_match * home_match_count
}

pub fn calc_upkeep(_team: &Team) -> i64 {
    0
}

/// Weekly merchandise income for a club, scaled by reputation, fan approval,
/// league position, and recent winning form.
pub fn weekly_merchandise_income(
    team: &Team,
    current_position: Option<u32>,
    fan_approval: u8,
) -> i64 {
    let reputation_component = team.reputation as i64 * MERCHANDISE_REPUTATION_MULTIPLIER;
    let league_position_component = match current_position {
        Some(1) => 3_000,
        Some(2..=4) => 2_000,
        Some(5..=8) => 1_000,
        _ => 0,
    };
    let form_component = team
        .form
        .iter()
        .filter(|result| result.as_str() == "W")
        .count() as i64
        * MERCHANDISE_WIN_FORM_BONUS;
    let fan_multiplier = 50 + fan_approval.min(100) as i64;

    ((reputation_component + league_position_component + form_component) * fan_multiplier / 100)
        .clamp(MERCHANDISE_MIN_WEEKLY_INCOME, MERCHANDISE_MAX_WEEKLY_INCOME)
}

fn team_fan_approval(game: &Game, team_id: &str) -> u8 {
    if game.manager.team_id.as_deref() == Some(team_id) {
        game.manager.fan_approval
    } else {
        MERCHANDISE_NEUTRAL_FAN_APPROVAL
    }
}

fn weekly_loan_repayment_due(team: &Team) -> i64 {
    team.bank_loan
        .as_ref()
        .map(|loan| loan.weekly_repayment.min(loan.remaining_balance))
        .unwrap_or(0)
}

fn estimated_weekly_matchday_income(game: &Game, team: &Team) -> i64 {
    let recent_home_match_count = count_recent_home_matches(game, &team.id);
    if recent_home_match_count == 0 {
        return 0;
    }

    calc_matchday(team.stadium_capacity, recent_home_match_count, 0.76, 20.0)
}

pub fn team_finance_snapshot(game: &Game, team_id: &str) -> Option<TeamFinanceSnapshot> {
    let team = game.teams.iter().find(|team| team.id == team_id)?;
    let annual_wage_bill = calc_annual_wages(game, team_id);
    let weekly_wage_spend = calc_wages(game, team_id);
    let weekly_wage_budget = team.wage_budget / 52;
    let current_position = current_league_position(game, team_id);
    let weekly_sponsor_income = team
        .sponsorship
        .as_ref()
        .map(|sponsorship| {
            sponsorship.base_value
                + evaluate_sponsorship_bonus(current_position, &team.form, sponsorship)
        })
        .unwrap_or(0);
    let weekly_matchday_income = estimated_weekly_matchday_income(game, team);
    let weekly_merchandise_income =
        weekly_merchandise_income(team, current_position, team_fan_approval(game, team_id));
    let weekly_loan_repayment = weekly_loan_repayment_due(team);
    let weekly_recurring_income =
        weekly_sponsor_income + weekly_matchday_income + weekly_merchandise_income;
    let projected_weekly_net = weekly_recurring_income - weekly_wage_spend - weekly_loan_repayment;
    let cash_runway_weeks = calc_cash_runway_weeks(team.finance, projected_weekly_net);
    let wage_budget_usage_percent = ((annual_wage_bill * 100) / std::cmp::max(1, team.wage_budget))
        .clamp(0, u32::MAX as i64) as u32;
    let wage_budget_status = wage_budget_status(wage_budget_usage_percent);
    let runway_status = runway_status(team.finance, cash_runway_weeks);

    Some(TeamFinanceSnapshot {
        annual_wage_bill,
        weekly_wage_spend,
        weekly_wage_budget,
        weekly_recurring_income,
        weekly_sponsor_income,
        weekly_merchandise_income,
        weekly_loan_repayment,
        projected_weekly_net,
        cash_runway_weeks,
        wage_budget_usage_percent,
        currently_in_debt: team.finance < 0,
        currently_over_budget: annual_wage_bill > team.wage_budget,
        wage_budget_status,
        runway_status,
        overall_status: most_severe_level(wage_budget_status, runway_status),
        marketing_campaign_cooldown_days_remaining: marketing_campaign_cooldown_days_remaining(
            team,
            game.clock.current_date.date_naive(),
        ),
    })
}

fn weekly_finance_satisfaction_penalty(snapshot: &TeamFinanceSnapshot) -> u8 {
    match snapshot.overall_status {
        FinanceHealthLevel::Critical => FINANCE_CRITICAL_SATISFACTION_PENALTY,
        FinanceHealthLevel::Warning => FINANCE_WARNING_SATISFACTION_PENALTY,
        FinanceHealthLevel::Stable | FinanceHealthLevel::Watch => 0,
    }
}

fn apply_weekly_finance_satisfaction_pressure(game: &mut Game) {
    let Some(user_team_id) = game.manager.team_id.clone() else {
        return;
    };

    let Some(snapshot) = team_finance_snapshot(game, &user_team_id) else {
        return;
    };

    let penalty = weekly_finance_satisfaction_penalty(&snapshot);
    if penalty == 0 {
        return;
    }

    game.manager.satisfaction = game.manager.satisfaction.saturating_sub(penalty);
}

fn finance_board_pressure_message(
    today: &str,
    severity: FinanceHealthLevel,
    penalty: u8,
) -> InboxMessage {
    let (body_key, priority) = match severity {
        FinanceHealthLevel::Critical => (
            "be.msg.financeBoardPressure.bodyCritical",
            MessagePriority::Urgent,
        ),
        FinanceHealthLevel::Warning => (
            "be.msg.financeBoardPressure.bodyWarning",
            MessagePriority::High,
        ),
        FinanceHealthLevel::Stable | FinanceHealthLevel::Watch => unreachable!(),
    };

    InboxMessage::new(
        format!("finance_board_pressure_{}", today),
        String::new(),
        String::new(),
        String::new(),
        today.to_string(),
    )
    .with_category(MessageCategory::BoardDirective)
    .with_priority(priority)
    .with_sender_role("")
    .with_i18n("be.msg.financeBoardPressure.subject", body_key, {
        let mut p = std::collections::HashMap::new();
        p.insert("penalty".to_string(), penalty.to_string());
        p
    })
    .with_sender_i18n("be.sender.boardOfDirectors", "be.role.chairman")
    .with_action(action(
        "view_finances",
        "",
        "be.msg.action.viewFinances",
        ActionType::NavigateTo {
            route: "/dashboard?tab=Finances".to_string(),
        },
    ))
}

fn marketing_campaign_message(
    today: &str,
    gross_revenue: i64,
    campaign_cost: i64,
    net_income: i64,
    cooldown_days: u32,
) -> InboxMessage {
    InboxMessage::new(
        format!("marketing_campaign_{}", today),
        String::new(),
        String::new(),
        String::new(),
        today.to_string(),
    )
    .with_category(MessageCategory::Finance)
    .with_priority(MessagePriority::Normal)
    .with_sender_role("")
    .with_i18n(
        "be.msg.marketingCampaign.subject",
        "be.msg.marketingCampaign.body",
        {
            let mut p = std::collections::HashMap::new();
            p.insert("grossRevenue".to_string(), gross_revenue.to_string());
            p.insert("campaignCost".to_string(), campaign_cost.to_string());
            p.insert("netIncome".to_string(), net_income.to_string());
            p.insert("days".to_string(), cooldown_days.to_string());
            p
        },
    )
    .with_sender_i18n("be.sender.commercialDirector", "be.role.commercialDirector")
    .with_action(action(
        "ack",
        "",
        "be.msg.event.ack",
        ActionType::Acknowledge,
    ))
}

fn bank_loan_approved_message(today: &str, preview: &BankLoanPreview) -> InboxMessage {
    InboxMessage::new(
        format!("bank_loan_approved_{}", today),
        String::new(),
        String::new(),
        String::new(),
        today.to_string(),
    )
    .with_category(MessageCategory::Finance)
    .with_priority(MessagePriority::Normal)
    .with_sender_role("")
    .with_i18n(
        "be.msg.bankLoanApproved.subject",
        "be.msg.bankLoanApproved.body",
        {
            let mut p = std::collections::HashMap::new();
            p.insert(
                "principal".to_string(),
                format_money(preview.principal as u64),
            );
            p.insert(
                "weeklyRepayment".to_string(),
                format_money(preview.weekly_repayment as u64),
            );
            p.insert("weeks".to_string(), preview.term_weeks.to_string());
            p.insert(
                "interestRate".to_string(),
                preview.interest_rate_percent.to_string(),
            );
            p
        },
    )
    .with_sender_i18n("be.sender.financialDirector", "be.role.financialDirector")
    .with_action(action(
        "view_finances",
        "",
        "be.msg.action.viewFinances",
        ActionType::NavigateTo {
            route: "/dashboard?tab=Finances".to_string(),
        },
    ))
}

fn bank_loan_repaid_message(today: &str, principal: i64, total_paid: i64) -> InboxMessage {
    InboxMessage::new(
        format!("bank_loan_repaid_{}", today),
        String::new(),
        String::new(),
        String::new(),
        today.to_string(),
    )
    .with_category(MessageCategory::Finance)
    .with_priority(MessagePriority::Normal)
    .with_sender_role("")
    .with_i18n(
        "be.msg.bankLoanRepaid.subject",
        "be.msg.bankLoanRepaid.body",
        {
            let mut p = std::collections::HashMap::new();
            p.insert("principal".to_string(), format_money(principal as u64));
            p.insert("totalPaid".to_string(), format_money(total_paid as u64));
            p
        },
    )
    .with_sender_i18n("be.sender.financialDirector", "be.role.financialDirector")
    .with_action(action(
        "ack",
        "",
        "be.msg.event.ack",
        ActionType::Acknowledge,
    ))
}

fn board_support_season(game: &Game) -> u32 {
    game.league
        .as_ref()
        .map(|league| league.season)
        .unwrap_or(game.clock.current_date.year().max(0) as u32)
}

fn sponsor_pitch_available(snapshot: &TeamFinanceSnapshot) -> bool {
    snapshot.currently_over_budget
        || snapshot.currently_in_debt
        || matches!(
            snapshot.wage_budget_status,
            FinanceHealthLevel::Warning | FinanceHealthLevel::Critical
        )
        || matches!(
            snapshot.runway_status,
            FinanceHealthLevel::Warning | FinanceHealthLevel::Critical
        )
}

fn board_support_available(snapshot: &TeamFinanceSnapshot) -> bool {
    snapshot.currently_in_debt
        || matches!(
            snapshot.runway_status,
            FinanceHealthLevel::Warning | FinanceHealthLevel::Critical
        )
}

fn marketing_campaign_available(snapshot: &TeamFinanceSnapshot) -> bool {
    snapshot.currently_over_budget
        || snapshot.currently_in_debt
        || matches!(
            snapshot.wage_budget_status,
            FinanceHealthLevel::Warning | FinanceHealthLevel::Critical
        )
        || matches!(
            snapshot.runway_status,
            FinanceHealthLevel::Warning | FinanceHealthLevel::Critical
        )
}

fn most_recent_marketing_campaign_date(team: &Team) -> Option<NaiveDate> {
    team.financial_ledger
        .iter()
        .filter(|entry| entry.kind == FinancialTransactionKind::CommercialCampaign)
        .filter_map(|entry| NaiveDate::parse_from_str(&entry.date, "%Y-%m-%d").ok())
        .max()
}

fn marketing_campaign_cooldown_days_remaining(team: &Team, today: NaiveDate) -> u32 {
    let Some(last_campaign) = most_recent_marketing_campaign_date(team) else {
        return 0;
    };

    let days_since = (today - last_campaign).num_days();
    if days_since >= MARKETING_CAMPAIGN_COOLDOWN_DAYS {
        0
    } else {
        (MARKETING_CAMPAIGN_COOLDOWN_DAYS - days_since) as u32
    }
}

fn marketing_campaign_gross_revenue(team: &Team, snapshot: &TeamFinanceSnapshot) -> i64 {
    let reputation_component = (team.reputation as i64) * 250;
    let stadium_component = (team.stadium_capacity as i64) * 3;
    let pressure_component = match snapshot.overall_status {
        FinanceHealthLevel::Stable => 0,
        FinanceHealthLevel::Watch => 10_000,
        FinanceHealthLevel::Warning => 25_000,
        FinanceHealthLevel::Critical => 40_000,
    };
    let debt_bonus = if snapshot.currently_in_debt {
        20_000
    } else {
        0
    };
    let wage_pressure_bonus = if snapshot.currently_over_budget {
        15_000
    } else {
        0
    };

    (reputation_component
        + stadium_component
        + pressure_component
        + debt_bonus
        + wage_pressure_bonus)
        .clamp(
            MARKETING_CAMPAIGN_MIN_GROSS_REVENUE,
            MARKETING_CAMPAIGN_MAX_GROSS_REVENUE,
        )
}

fn marketing_campaign_cost(gross_revenue: i64) -> i64 {
    (gross_revenue / 4).max(MARKETING_CAMPAIGN_MIN_COST)
}

fn bank_loan_interest_rate_percent(snapshot: &TeamFinanceSnapshot) -> u32 {
    let base = match snapshot.overall_status {
        FinanceHealthLevel::Stable => 6,
        FinanceHealthLevel::Watch => 9,
        FinanceHealthLevel::Warning => 14,
        FinanceHealthLevel::Critical => 20,
    };

    if snapshot.currently_in_debt {
        base + 4
    } else {
        base
    }
}

fn bank_loan_principal(team: &Team, snapshot: &TeamFinanceSnapshot) -> i64 {
    let base = team.wage_budget / 2 + team.reputation as i64 * BANK_LOAN_REPUTATION_MULTIPLIER;
    let health_percent = match snapshot.overall_status {
        FinanceHealthLevel::Stable => 100,
        FinanceHealthLevel::Watch => 80,
        FinanceHealthLevel::Warning => 60,
        FinanceHealthLevel::Critical => 40,
    };

    (base * health_percent / 100).clamp(BANK_LOAN_MIN_PRINCIPAL, BANK_LOAN_MAX_PRINCIPAL)
}

fn bank_loan_weekly_repayment(total_repayment: i64, term_weeks: u32) -> i64 {
    (total_repayment + term_weeks as i64 - 1) / term_weeks as i64
}

fn bank_loan_is_affordable(
    team: &Team,
    snapshot: &TeamFinanceSnapshot,
    preview: &BankLoanPreview,
) -> bool {
    let projected_net_with_loan = snapshot.projected_weekly_net - preview.weekly_repayment;
    if projected_net_with_loan >= 0 {
        return true;
    }

    let balance_with_principal = team.finance + preview.principal;
    balance_with_principal / projected_net_with_loan.abs() >= preview.term_weeks as i64
}

fn has_pending_sponsor_offer(game: &Game) -> bool {
    game.messages.iter().any(|message| {
        message.id.starts_with("sponsor_") && message.actions.iter().any(|action| !action.resolved)
    })
}

fn sponsor_pitch_message_id(game: &Game) -> String {
    format!(
        "sponsor_pitch_{}",
        game.clock.current_date.format("%Y-%m-%d")
    )
}

fn sponsor_pitch_partner(team_id: &str, current_date: chrono::DateTime<chrono::Utc>) -> String {
    const SPONSORS: [&[&str]; 8] = [
        &["Northstar", "Logistics"],
        &["Harbor", "Bank"],
        &["Crest", "Mobile"],
        &["Vertex", "Nutrition"],
        &["Iron", "Peak", "Tools"],
        &["Brightline", "Energy"],
        &["Summit", "Capital"],
        &["Evergreen", "Foods"],
    ];

    let seed = team_id
        .bytes()
        .fold(current_date.ordinal() as usize, |acc, byte| {
            acc.wrapping_mul(31).wrapping_add(byte as usize)
        });

    SPONSORS[seed % SPONSORS.len()].join(" ")
}

fn sponsor_pitch_weekly_amount(
    team: &Team,
    snapshot: &TeamFinanceSnapshot,
    current_position: Option<u32>,
) -> i64 {
    let reputation_component = team.reputation as i64 * SPONSOR_PITCH_REPUTATION_MULTIPLIER;
    let league_position_component = match current_position {
        Some(1) => 18_000,
        Some(2..=4) => 12_000,
        Some(5..=8) => 6_000,
        _ => 0,
    };
    let pressure_component = match snapshot.overall_status {
        FinanceHealthLevel::Stable => 0,
        FinanceHealthLevel::Watch => 5_000,
        FinanceHealthLevel::Warning => 15_000,
        FinanceHealthLevel::Critical => 25_000,
    };
    let wage_pressure_bonus = if snapshot.currently_over_budget {
        15_000
    } else {
        0
    };
    let debt_bonus = if snapshot.currently_in_debt {
        20_000
    } else {
        0
    };

    (SPONSOR_PITCH_MIN_WEEKLY_AMOUNT
        + reputation_component
        + league_position_component
        + pressure_component
        + wage_pressure_bonus
        + debt_bonus)
        .clamp(
            SPONSOR_PITCH_MIN_WEEKLY_AMOUNT,
            SPONSOR_PITCH_MAX_WEEKLY_AMOUNT,
        )
}

pub fn preview_board_support(game: &Game, team_id: &str) -> Result<BoardSupportResult, String> {
    let snapshot =
        team_finance_snapshot(game, team_id).ok_or("be.error.managedTeamNotFound".to_string())?;

    if !board_support_available(&snapshot) {
        return Err("be.error.finance.boardSupportUnavailable".to_string());
    }

    let season = board_support_season(game);
    let team = game
        .teams
        .iter()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    if team.financial_ledger.iter().any(|entry| {
        entry.kind == FinancialTransactionKind::BoardSupport
            && entry.description == board_support_description(season)
    }) {
        return Err("be.error.finance.boardSupportAlreadyUsed".to_string());
    }

    let reserve_target = std::cmp::max(
        snapshot.weekly_wage_spend * BOARD_SUPPORT_TARGET_RUNWAY_WEEKS,
        BOARD_SUPPORT_MIN_AMOUNT,
    );
    let support_amount = (reserve_target - team.finance)
        .max(BOARD_SUPPORT_MIN_AMOUNT)
        .min(BOARD_SUPPORT_MAX_AMOUNT);
    let transfer_budget_reduction = std::cmp::min(team.transfer_budget.max(0), support_amount / 2);

    Ok(BoardSupportResult {
        support_amount,
        transfer_budget_reduction,
        satisfaction_penalty: BOARD_SUPPORT_SATISFACTION_PENALTY,
    })
}

pub fn preview_sponsor_pitch(game: &Game, team_id: &str) -> Result<SponsorPitchPreview, String> {
    let snapshot =
        team_finance_snapshot(game, team_id).ok_or("be.error.managedTeamNotFound".to_string())?;

    if !sponsor_pitch_available(&snapshot) {
        return Err("be.error.finance.sponsorPitchUnavailable".to_string());
    }

    if has_pending_sponsor_offer(game) {
        return Err("be.error.finance.sponsorPitchPendingOffer".to_string());
    }

    let message_id = sponsor_pitch_message_id(game);
    if game.messages.iter().any(|message| message.id == message_id) {
        return Err("be.error.finance.sponsorPitchAlreadyAttemptedToday".to_string());
    }

    let team = game
        .teams
        .iter()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    if team
        .sponsorship
        .as_ref()
        .is_some_and(|sponsorship| sponsorship.remaining_weeks > 0 && sponsorship.base_value > 0)
    {
        return Err("be.error.finance.sponsorPitchActiveSponsor".to_string());
    }

    let current_position = current_league_position(game, team_id);

    Ok(SponsorPitchPreview {
        sponsor_name: sponsor_pitch_partner(team_id, game.clock.current_date),
        weekly_amount: sponsor_pitch_weekly_amount(team, &snapshot, current_position),
        duration_weeks: SPONSOR_PITCH_DURATION_WEEKS,
    })
}

pub fn preview_marketing_campaign(
    game: &Game,
    team_id: &str,
) -> Result<MarketingCampaignPreview, String> {
    let snapshot =
        team_finance_snapshot(game, team_id).ok_or("be.error.managedTeamNotFound".to_string())?;

    if !marketing_campaign_available(&snapshot) {
        return Err("be.error.finance.marketingCampaignUnavailable".to_string());
    }

    let today = game.clock.current_date.date_naive();
    let team = game
        .teams
        .iter()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    if marketing_campaign_cooldown_days_remaining(team, today) > 0 {
        return Err("be.error.finance.marketingCampaignCoolingDown".to_string());
    }

    let gross_revenue = marketing_campaign_gross_revenue(team, &snapshot);
    let campaign_cost = marketing_campaign_cost(gross_revenue);

    Ok(MarketingCampaignPreview {
        gross_revenue,
        campaign_cost,
        net_income: gross_revenue - campaign_cost,
        cooldown_days: MARKETING_CAMPAIGN_COOLDOWN_DAYS as u32,
    })
}

pub fn preview_bank_loan(game: &Game, team_id: &str) -> Result<BankLoanPreview, String> {
    let snapshot =
        team_finance_snapshot(game, team_id).ok_or("be.error.managedTeamNotFound".to_string())?;
    let team = game
        .teams
        .iter()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    if team.bank_loan.is_some() {
        return Err("be.error.finance.loanAlreadyActive".to_string());
    }

    let principal = bank_loan_principal(team, &snapshot);
    let interest_rate_percent = bank_loan_interest_rate_percent(&snapshot);
    let total_repayment = principal * (100 + interest_rate_percent as i64) / 100;
    let weekly_repayment = bank_loan_weekly_repayment(total_repayment, BANK_LOAN_TERM_WEEKS);
    let preview = BankLoanPreview {
        principal,
        interest_rate_percent,
        term_weeks: BANK_LOAN_TERM_WEEKS,
        weekly_repayment,
        total_repayment,
    };

    if !bank_loan_is_affordable(team, &snapshot, &preview) {
        return Err("be.error.finance.loanUnaffordable".to_string());
    }

    Ok(preview)
}

pub fn finance_action_previews(game: &Game, team_id: &str) -> Option<FinanceActionPreviews> {
    game.teams.iter().find(|team| team.id == team_id)?;

    Some(FinanceActionPreviews {
        board_support: preview_board_support(game, team_id).ok(),
        sponsor_pitch: preview_sponsor_pitch(game, team_id).ok(),
        marketing_campaign: preview_marketing_campaign(game, team_id).ok(),
        bank_loan: preview_bank_loan(game, team_id).ok(),
    })
}

pub fn request_sponsor_pitch(game: &mut Game, team_id: &str) -> Result<SponsorPitchResult, String> {
    let preview = preview_sponsor_pitch(game, team_id)?;
    let message_id = sponsor_pitch_message_id(game);
    let weekly_amount = preview.weekly_amount;
    let sponsor_name = preview.sponsor_name;
    let team = game
        .teams
        .iter()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;
    let team_name = team.name.clone();
    let date = game.clock.current_date.format("%Y-%m-%d").to_string();

    game.messages
        .push(crate::random_events::sponsor_offer_message(
            &message_id,
            &team_name,
            &sponsor_name,
            weekly_amount as u64,
            &date,
        ));

    Ok(SponsorPitchResult {
        message_id,
        sponsor_name,
        weekly_amount,
        duration_weeks: preview.duration_weeks,
    })
}

pub fn request_marketing_campaign(
    game: &mut Game,
    team_id: &str,
) -> Result<MarketingCampaignResult, String> {
    let preview = preview_marketing_campaign(game, team_id)?;
    let today = game.clock.current_date.date_naive();
    let gross_revenue = preview.gross_revenue;
    let campaign_cost = preview.campaign_cost;
    let net_income = preview.net_income;
    let today_label = today.format("%Y-%m-%d").to_string();
    let message = marketing_campaign_message(
        &today_label,
        gross_revenue,
        campaign_cost,
        net_income,
        preview.cooldown_days,
    );
    let message_id = message.id.clone();
    let team = game
        .teams
        .iter_mut()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    team.finance += net_income;
    team.season_income += gross_revenue;
    team.season_expenses += campaign_cost;
    team.financial_ledger.push(FinancialTransaction {
        date: today_label.clone(),
        description: marketing_campaign_activation_description(),
        amount: -campaign_cost,
        kind: FinancialTransactionKind::CommercialCampaign,
    });
    team.financial_ledger.push(FinancialTransaction {
        date: today_label,
        description: marketing_campaign_revenue_description(),
        amount: gross_revenue,
        kind: FinancialTransactionKind::CommercialCampaign,
    });
    game.messages.push(message);

    Ok(MarketingCampaignResult {
        message_id,
        gross_revenue,
        campaign_cost,
        net_income,
        cooldown_days: preview.cooldown_days,
    })
}

pub fn request_bank_loan(game: &mut Game, team_id: &str) -> Result<BankLoanResult, String> {
    let preview = preview_bank_loan(game, team_id)?;
    let today_label = game.clock.current_date.format("%Y-%m-%d").to_string();
    let message = bank_loan_approved_message(&today_label, &preview);
    let message_id = message.id.clone();

    if game.messages.iter().any(|message| message.id == message_id) {
        return Err("be.error.finance.loanAlreadyRequestedToday".to_string());
    }

    let team = game
        .teams
        .iter_mut()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    team.finance += preview.principal;
    team.season_income += preview.principal;
    team.bank_loan = Some(BankLoan {
        principal: preview.principal,
        remaining_balance: preview.total_repayment,
        weekly_repayment: preview.weekly_repayment,
        remaining_weeks: preview.term_weeks,
        interest_rate_percent: preview.interest_rate_percent,
        start_date: today_label.clone(),
    });
    team.financial_ledger.push(FinancialTransaction {
        date: today_label,
        description: bank_loan_drawdown_description(),
        amount: preview.principal,
        kind: FinancialTransactionKind::BankLoan,
    });
    game.messages.push(message);

    Ok(BankLoanResult {
        message_id,
        principal: preview.principal,
        interest_rate_percent: preview.interest_rate_percent,
        term_weeks: preview.term_weeks,
        weekly_repayment: preview.weekly_repayment,
        total_repayment: preview.total_repayment,
    })
}

pub fn repay_bank_loan(game: &mut Game, team_id: &str) -> Result<BankLoanRepaymentResult, String> {
    let today_label = game.clock.current_date.format("%Y-%m-%d").to_string();
    let team = game
        .teams
        .iter_mut()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;

    let Some(loan) = team.bank_loan.clone() else {
        return Err("be.error.finance.loanNotActive".to_string());
    };

    let amount_due = loan.remaining_balance;
    if team.finance < amount_due {
        return Err("be.error.finance.loanRepaymentInsufficientFunds".to_string());
    }

    team.finance -= amount_due;
    team.season_expenses += amount_due;
    team.bank_loan = None;
    team.financial_ledger.push(FinancialTransaction {
        date: today_label.clone(),
        description: bank_loan_early_settlement_description(),
        amount: -amount_due,
        kind: FinancialTransactionKind::BankLoan,
    });

    let total_paid = loan.principal * (100 + loan.interest_rate_percent as i64) / 100;
    let message = bank_loan_repaid_message(&today_label, loan.principal, total_paid);
    let message_id = message.id.clone();
    game.messages.push(message);

    Ok(BankLoanRepaymentResult {
        message_id,
        amount_paid: amount_due,
    })
}

pub fn request_board_support(game: &mut Game, team_id: &str) -> Result<BoardSupportResult, String> {
    let preview = preview_board_support(game, team_id)?;
    let season = board_support_season(game);
    let team = game
        .teams
        .iter_mut()
        .find(|team| team.id == team_id)
        .ok_or("be.error.managedTeamNotFound".to_string())?;
    let support_amount = preview.support_amount;
    let transfer_budget_reduction = preview.transfer_budget_reduction;

    team.finance += support_amount;
    team.season_income += support_amount;
    team.transfer_budget = (team.transfer_budget - transfer_budget_reduction).max(0);
    team.financial_ledger.push(FinancialTransaction {
        date: game.clock.current_date.format("%Y-%m-%d").to_string(),
        description: board_support_description(season),
        amount: support_amount,
        kind: FinancialTransactionKind::BoardSupport,
    });

    game.manager.satisfaction = game
        .manager
        .satisfaction
        .saturating_sub(preview.satisfaction_penalty);

    Ok(preview)
}

pub fn evaluate_sponsorship_bonus(
    current_position: Option<u32>,
    recent_form: &[String],
    sponsorship: &Sponsorship,
) -> i64 {
    sponsorship
        .bonus_criteria
        .iter()
        .map(|criterion| match criterion {
            SponsorshipBonusCriterion::LeaguePosition {
                max_position,
                bonus_amount,
            } => {
                if current_position.is_some_and(|position| position <= *max_position) {
                    *bonus_amount
                } else {
                    0
                }
            }
            SponsorshipBonusCriterion::UnbeatenRun {
                required_matches,
                bonus_amount,
            } => {
                if recent_form.len() >= *required_matches
                    && recent_form
                        .iter()
                        .rev()
                        .take(*required_matches)
                        .all(|result| result != "L")
                {
                    *bonus_amount
                } else {
                    0
                }
            }
        })
        .sum()
}

fn current_league_position(game: &Game, team_id: &str) -> Option<u32> {
    let league = game.league.as_ref()?;

    league
        .sorted_standings()
        .iter()
        .position(|standing| standing.team_id == team_id)
        .map(|index| index as u32 + 1)
}

fn count_recent_home_matches(game: &Game, team_id: &str) -> i64 {
    let Some(league) = &game.league else {
        return 0;
    };

    let current = game.clock.current_date.date_naive();
    let week_ago = current - chrono::Duration::days(7);

    league
        .fixtures
        .iter()
        .filter(|fixture| {
            fixture.status == domain::league::FixtureStatus::Completed
                && fixture.home_team_id == team_id
                && fixture.result.is_some()
        })
        .filter(|fixture| {
            if let Ok(date) = chrono::NaiveDate::parse_from_str(&fixture.date, "%Y-%m-%d") {
                date > week_ago && date <= current
            } else {
                false
            }
        })
        .count() as i64
}

/// Process weekly financial operations (called every Monday = weekday 0).
/// - Deduct player wages (weekly = annual / 52)
/// - Deduct staff wages
/// - Add merchandise sales income for every club
/// - Add matchday revenue for home matches played that week
/// - Collect scheduled bank loan repayments
/// - Check financial health and generate warnings
pub fn process_weekly_finances(game: &mut Game) {
    let weekday = game.clock.current_date.weekday().num_days_from_monday();
    if weekday != 0 {
        return; // Only process on Mondays
    }

    let today = game.clock.current_date.format("%Y-%m-%d").to_string();
    let user_team_id = game.manager.team_id.clone();
    let user_fan_approval = game.manager.fan_approval;
    let team_expenses: Vec<(String, i64)> = game
        .teams
        .iter()
        .map(|team| {
            let wages = calc_wages(game, &team.id);
            let upkeep = calc_upkeep(team);

            (team.id.clone(), wages + upkeep)
        })
        .collect();
    let team_positions: Vec<(String, Option<u32>)> = game
        .teams
        .iter()
        .map(|team| (team.id.clone(), current_league_position(game, &team.id)))
        .collect();

    for team in game.teams.iter_mut() {
        let total_expenses = team_expenses
            .iter()
            .find(|(team_id, _)| team_id == &team.id)
            .map(|(_, total)| *total)
            .unwrap_or(0);

        team.finance -= total_expenses;
        team.season_expenses += total_expenses;

        let current_position = team_positions
            .iter()
            .find(|(team_id, _)| team_id == &team.id)
            .and_then(|(_, position)| *position);

        let sponsorship_income = team
            .sponsorship
            .as_ref()
            .map(|sponsorship| {
                sponsorship.base_value
                    + evaluate_sponsorship_bonus(current_position, &team.form, sponsorship)
            })
            .unwrap_or(0);

        if sponsorship_income > 0 {
            team.finance += sponsorship_income;
            team.season_income += sponsorship_income;
        }

        if let Some(sponsorship) = team.sponsorship.as_mut() {
            sponsorship.remaining_weeks = sponsorship.remaining_weeks.saturating_sub(1);
            if sponsorship.remaining_weeks == 0 {
                team.sponsorship = None;
            }
        }

        let is_user_team = user_team_id.as_deref() == Some(team.id.as_str());
        let fan_approval = if is_user_team {
            user_fan_approval
        } else {
            MERCHANDISE_NEUTRAL_FAN_APPROVAL
        };
        let merchandise_income = weekly_merchandise_income(team, current_position, fan_approval);
        team.finance += merchandise_income;
        team.season_income += merchandise_income;
        if is_user_team {
            team.financial_ledger.push(FinancialTransaction {
                date: today.clone(),
                description: merchandise_income_description(),
                amount: merchandise_income,
                kind: FinancialTransactionKind::Merchandise,
            });
        }
    }

    // --- Bank loan repayments ---
    let mut user_loan_repaid: Option<(i64, i64)> = None;
    for team in game.teams.iter_mut() {
        let Some(mut loan) = team.bank_loan.take() else {
            continue;
        };

        let installment = loan.weekly_repayment.min(loan.remaining_balance);
        team.finance -= installment;
        team.season_expenses += installment;
        loan.remaining_balance -= installment;
        loan.remaining_weeks = loan.remaining_weeks.saturating_sub(1);

        let is_user_team = user_team_id.as_deref() == Some(team.id.as_str());
        if is_user_team {
            team.financial_ledger.push(FinancialTransaction {
                date: today.clone(),
                description: bank_loan_repayment_description(),
                amount: -installment,
                kind: FinancialTransactionKind::BankLoan,
            });
        }

        if loan.remaining_balance > 0 {
            team.bank_loan = Some(loan);
        } else if is_user_team {
            let total_paid = loan.principal * (100 + loan.interest_rate_percent as i64) / 100;
            user_loan_repaid = Some((loan.principal, total_paid));
        }
    }
    if let Some((principal, total_paid)) = user_loan_repaid {
        game.messages
            .push(bank_loan_repaid_message(&today, principal, total_paid));
    }

    // --- Matchday income for home matches completed in last 7 days ---
    if game.league.is_some() {
        let home_match_counts: Vec<(String, i64)> = game
            .teams
            .iter()
            .map(|team| (team.id.clone(), count_recent_home_matches(game, &team.id)))
            .collect();

        for team in game.teams.iter_mut() {
            let home_count = home_match_counts
                .iter()
                .find(|(team_id, _)| team_id == &team.id)
                .map(|(_, count)| *count)
                .unwrap_or(0);

            if home_count > 0 {
                let mut rng = rand::rng();
                let attendance_pct = rng.random_range(60..=92) as f64 / 100.0;
                let avg_ticket = rng.random_range(15..=25) as f64;
                let total_revenue = calc_matchday(
                    team.stadium_capacity,
                    home_count,
                    attendance_pct,
                    avg_ticket,
                );

                team.finance += total_revenue;
                team.season_income += total_revenue;
            }
        }
    }

    // --- Financial health warnings for user's team ---
    generate_financial_warnings(game, &today);
    apply_weekly_finance_satisfaction_pressure(game);
}

/// Generate inbox messages warning about financial issues.
fn generate_financial_warnings(game: &mut Game, today: &str) {
    let user_team_id = match &game.manager.team_id {
        Some(id) => id.clone(),
        None => return,
    };

    let team = match game.teams.iter().find(|t| t.id == user_team_id) {
        Some(t) => t,
        None => return,
    };

    let existing_ids: std::collections::HashSet<String> =
        game.messages.iter().map(|m| m.id.clone()).collect();

    let mut new_messages: Vec<InboxMessage> = Vec::new();

    let snapshot = match team_finance_snapshot(game, &user_team_id) {
        Some(snapshot) => snapshot,
        None => return,
    };

    let weekly_wages = calc_wages(game, &user_team_id);
    let annual_wages = calc_annual_wages(game, &user_team_id);
    let weeks_left = snapshot.cash_runway_weeks.unwrap_or(999);

    // Critical: finances negative
    if team.finance < 0 {
        let msg_id = format!("finance_critical_{}", today);
        if !existing_ids.contains(&msg_id) {
            new_messages.push(
                InboxMessage::new(
                    msg_id,
                    String::new(),
                    String::new(),
                    String::new(),
                    today.to_string(),
                )
                .with_category(MessageCategory::Finance)
                .with_priority(MessagePriority::Urgent)
                .with_sender_role("")
                .with_i18n(
                    "be.msg.financeCritical.subject",
                    "be.msg.financeCritical.body",
                    {
                        let mut p = std::collections::HashMap::new();
                        p.insert("amount".to_string(), format_money((-team.finance) as u64));
                        p
                    },
                )
                .with_sender_i18n("be.sender.boardOfDirectors", "be.role.chairman")
                .with_action(action(
                    "view_finances",
                    "",
                    "be.msg.action.viewFinances",
                    ActionType::NavigateTo {
                        route: "/dashboard?tab=Finances".to_string(),
                    },
                )),
            );
        }
    }
    // Warning: less than 4 weeks of runway
    else if (0..4).contains(&weeks_left) {
        let msg_id = format!("finance_warning_{}", today);
        if !existing_ids.contains(&msg_id) {
            new_messages.push(
                InboxMessage::new(
                    msg_id,
                    String::new(),
                    String::new(),
                    String::new(),
                    today.to_string(),
                )
                .with_category(MessageCategory::Finance)
                .with_priority(MessagePriority::High)
                .with_sender_role("")
                .with_i18n(
                    "be.msg.financeWarning.subject",
                    "be.msg.financeWarning.body",
                    {
                        let mut p = std::collections::HashMap::new();
                        p.insert("weeklyWages".to_string(), format_money(weekly_wages as u64));
                        p.insert("weeksLeft".to_string(), weeks_left.to_string());
                        p
                    },
                )
                .with_sender_i18n("be.sender.financialDirector", "be.role.financialDirector")
                .with_action(action(
                    "view_finances",
                    "",
                    "be.msg.action.viewFinances",
                    ActionType::NavigateTo {
                        route: "/dashboard?tab=Finances".to_string(),
                    },
                )),
            );
        }
    }
    // Over budget warning: wages exceed budget
    else if annual_wages > team.wage_budget {
        let msg_id = format!("wage_over_budget_{}", today);
        if !existing_ids.contains(&msg_id) {
            new_messages.push(
                InboxMessage::new(
                    msg_id,
                    String::new(),
                    String::new(),
                    String::new(),
                    today.to_string(),
                )
                .with_category(MessageCategory::Finance)
                .with_priority(MessagePriority::Normal)
                .with_sender_role("")
                .with_i18n(
                    "be.msg.wageOverBudget.subject",
                    "be.msg.wageOverBudget.body",
                    {
                        let mut p = std::collections::HashMap::new();
                        p.insert("annualWages".to_string(), format_money(annual_wages as u64));
                        p.insert(
                            "wageBudget".to_string(),
                            format_money(team.wage_budget as u64),
                        );
                        p
                    },
                )
                .with_sender_i18n("be.sender.financialDirector", "be.role.financialDirector")
                .with_action(action(
                    "view_finances",
                    "",
                    "be.msg.action.viewFinances",
                    ActionType::NavigateTo {
                        route: "/dashboard?tab=Finances".to_string(),
                    },
                )),
            );
        }
    }

    let penalty = weekly_finance_satisfaction_penalty(&snapshot);
    if penalty > 0 {
        let msg_id = format!("finance_board_pressure_{}", today);
        if !existing_ids.contains(&msg_id) {
            new_messages.push(finance_board_pressure_message(
                today,
                snapshot.overall_status,
                penalty,
            ));
        }
    }

    game.messages.extend(new_messages);
}

fn format_money(amount: u64) -> String {
    crate::currency::format_compact_number(amount, crate::currency::DEFAULT_CURRENCY_CODE)
        .unwrap_or_else(|| amount.to_string())
}

#[cfg(test)]
mod tests {
    use super::{
        preview_bank_loan, preview_sponsor_pitch, process_weekly_finances, repay_bank_loan,
        request_bank_loan, weekly_merchandise_income,
    };
    use crate::clock::GameClock;
    use crate::game::Game;
    use chrono::{TimeZone, Utc};
    use domain::league::League;
    use domain::manager::Manager;
    use domain::team::{FinancialTransactionKind, Team};

    fn make_team(id: &str, name: &str) -> Team {
        let mut team = Team::new(
            id.to_string(),
            name.to_string(),
            name[..3].to_string(),
            "England".to_string(),
            "Testville".to_string(),
            format!("{} Ground", name),
            22_000,
        );
        team.reputation = 650;
        team.finance = -25_000;
        team.wage_budget = 400_000;
        team.transfer_budget = 500_000;
        team
    }

    fn make_game() -> Game {
        let clock = GameClock::new(Utc.with_ymd_and_hms(2026, 2, 16, 12, 0, 0).unwrap());
        let mut manager = Manager::new(
            "mgr-user".to_string(),
            "Alex".to_string(),
            "Boss".to_string(),
            "1980-01-01".to_string(),
            "England".to_string(),
        );
        manager.hire("team1".to_string());

        let mut game = Game::new(
            clock,
            manager,
            vec![
                make_team("team1", "Alpha FC"),
                make_team("team2", "Beta FC"),
            ],
            vec![],
            vec![],
            vec![],
        );

        let mut league = League::new(
            "league-1".to_string(),
            "Premier Division".to_string(),
            2026,
            &["team1".to_string(), "team2".to_string()],
        );
        league
            .standings
            .iter_mut()
            .find(|entry| entry.team_id == "team1")
            .unwrap()
            .points = 32;
        league
            .standings
            .iter_mut()
            .find(|entry| entry.team_id == "team2")
            .unwrap()
            .points = 18;
        game.league = Some(league);
        game
    }

    #[test]
    fn preview_sponsor_pitch_rewards_stronger_league_position() {
        let game = make_game();

        let leader_pitch = preview_sponsor_pitch(&game, "team1").expect("leader pitch");
        let trailing_pitch = preview_sponsor_pitch(&game, "team2").expect("trailing pitch");

        assert!(
            leader_pitch.weekly_amount > trailing_pitch.weekly_amount,
            "A stronger league position should improve sponsor pitch value when other club factors are equal"
        );
    }

    #[test]
    fn weekly_merchandise_income_scales_with_fan_approval() {
        let team = make_team("team1", "Alpha FC");

        let low_approval = weekly_merchandise_income(&team, None, 20);
        let high_approval = weekly_merchandise_income(&team, None, 90);

        assert!(
            high_approval > low_approval,
            "Happier fans should buy more merchandise"
        );
    }

    #[test]
    fn weekly_merchandise_income_scales_with_reputation_position_and_form() {
        let mut small_club = make_team("team1", "Alpha FC");
        small_club.reputation = 200;
        let mut big_club = make_team("team2", "Betas FC");
        big_club.reputation = 900;

        assert!(
            weekly_merchandise_income(&big_club, None, 50)
                > weekly_merchandise_income(&small_club, None, 50),
            "Reputation should drive merchandise income"
        );
        assert!(
            weekly_merchandise_income(&small_club, Some(1), 50)
                > weekly_merchandise_income(&small_club, Some(12), 50),
            "Leading the league should drive merchandise income"
        );

        let mut in_form_club = small_club.clone();
        in_form_club.form = vec!["W".to_string(), "W".to_string(), "W".to_string()];
        assert!(
            weekly_merchandise_income(&in_form_club, None, 50)
                > weekly_merchandise_income(&small_club, None, 50),
            "A winning run should drive merchandise income"
        );
    }

    #[test]
    fn weekly_merchandise_income_is_clamped_to_a_floor() {
        let mut team = make_team("team1", "Alpha FC");
        team.reputation = 0;

        assert_eq!(weekly_merchandise_income(&team, None, 0), 500);
    }

    #[test]
    fn process_weekly_finances_pays_merchandise_income_to_all_clubs() {
        let mut game = make_game();
        game.teams[0].finance = 1_000_000;
        game.teams[1].finance = 1_000_000;
        let user_position = Some(1);
        let rival_position = Some(2);
        let user_income =
            weekly_merchandise_income(&game.teams[0], user_position, game.manager.fan_approval);
        let rival_income = weekly_merchandise_income(&game.teams[1], rival_position, 50);

        process_weekly_finances(&mut game);

        assert_eq!(game.teams[0].finance, 1_000_000 + user_income);
        assert_eq!(game.teams[1].finance, 1_000_000 + rival_income);
        let user_ledger_entry = game.teams[0]
            .financial_ledger
            .iter()
            .find(|entry| entry.kind == FinancialTransactionKind::Merchandise)
            .expect("merchandise ledger entry for the user club");
        assert_eq!(user_ledger_entry.amount, user_income);
        assert!(
            !game.teams[1]
                .financial_ledger
                .iter()
                .any(|entry| entry.kind == FinancialTransactionKind::Merchandise),
            "AI clubs should not accumulate weekly ledger entries"
        );
    }

    #[test]
    fn preview_bank_loan_charges_higher_interest_under_financial_pressure() {
        let mut healthy_game = make_game();
        healthy_game.teams[0].finance = 2_000_000;
        let mut pressured_game = make_game();
        pressured_game.teams[0].finance = -25_000;

        let healthy_preview = preview_bank_loan(&healthy_game, "team1").expect("healthy preview");
        let pressured_preview =
            preview_bank_loan(&pressured_game, "team1").expect("pressured preview");

        assert!(
            pressured_preview.interest_rate_percent > healthy_preview.interest_rate_percent,
            "Poor financial health should raise the interest rate"
        );
        assert!(
            pressured_preview.principal < healthy_preview.principal,
            "Poor financial health should reduce the available principal"
        );
    }

    #[test]
    fn preview_bank_loan_rejects_clubs_with_an_active_loan() {
        let mut game = make_game();
        game.teams[0].finance = 2_000_000;
        request_bank_loan(&mut game, "team1").expect("first loan");

        let error = preview_bank_loan(&game, "team1").expect_err("second loan should fail");

        assert_eq!(error, "be.error.finance.loanAlreadyActive");
    }

    #[test]
    fn preview_bank_loan_rejects_unaffordable_repayments() {
        let mut game = make_game();
        game.teams[0].finance = -2_500_000;
        game.teams[0].reputation = 0;
        game.teams[0].wage_budget = 50_000_000;

        let error = preview_bank_loan(&game, "team1").expect_err("unaffordable loan should fail");

        assert_eq!(error, "be.error.finance.loanUnaffordable");
    }

    #[test]
    fn request_bank_loan_credits_principal_and_records_state() {
        let mut game = make_game();
        game.teams[0].finance = 2_000_000;

        let result = request_bank_loan(&mut game, "team1").expect("loan");

        assert_eq!(game.teams[0].finance, 2_000_000 + result.principal);
        assert_eq!(game.teams[0].season_income, result.principal);
        let loan = game.teams[0].bank_loan.as_ref().expect("active loan");
        assert_eq!(loan.principal, result.principal);
        assert_eq!(loan.remaining_balance, result.total_repayment);
        assert_eq!(loan.weekly_repayment, result.weekly_repayment);
        assert_eq!(loan.remaining_weeks, result.term_weeks);
        assert_eq!(
            game.teams[0].financial_ledger.last().expect("ledger").kind,
            FinancialTransactionKind::BankLoan
        );
        let message = game
            .messages
            .iter()
            .find(|message| message.id == result.message_id)
            .expect("loan approval message");
        assert_eq!(
            message.subject_key.as_deref(),
            Some("be.msg.bankLoanApproved.subject")
        );
    }

    #[test]
    fn weekly_loan_repayments_follow_the_schedule_until_settled() {
        let mut game = make_game();
        game.teams[0].finance = 2_000_000;
        let result = request_bank_loan(&mut game, "team1").expect("loan");
        let finance_after_loan = game.teams[0].finance;
        let merchandise_income =
            weekly_merchandise_income(&game.teams[0], Some(1), game.manager.fan_approval);

        process_weekly_finances(&mut game);

        let loan = game.teams[0].bank_loan.as_ref().expect("loan still open");
        assert_eq!(
            loan.remaining_balance,
            result.total_repayment - result.weekly_repayment
        );
        assert_eq!(loan.remaining_weeks, result.term_weeks - 1);
        assert_eq!(
            game.teams[0].finance,
            finance_after_loan + merchandise_income - result.weekly_repayment
        );

        // Fast-forward the remaining schedule.
        for _ in 1..result.term_weeks {
            game.clock.current_date += chrono::Duration::days(7);
            process_weekly_finances(&mut game);
        }

        assert!(
            game.teams[0].bank_loan.is_none(),
            "Loan should be settled after the full term"
        );
        assert!(
            game.messages
                .iter()
                .any(|message| message.subject_key.as_deref()
                    == Some("be.msg.bankLoanRepaid.subject")),
            "Settling the loan should notify the manager"
        );
        let repayment_total: i64 = game.teams[0]
            .financial_ledger
            .iter()
            .filter(|entry| entry.kind == FinancialTransactionKind::BankLoan && entry.amount < 0)
            .map(|entry| -entry.amount)
            .sum();
        assert_eq!(repayment_total, result.total_repayment);
    }

    #[test]
    fn repay_bank_loan_settles_the_balance_early() {
        let mut game = make_game();
        game.teams[0].finance = 2_000_000;
        let result = request_bank_loan(&mut game, "team1").expect("loan");
        let finance_after_loan = game.teams[0].finance;

        let repayment = repay_bank_loan(&mut game, "team1").expect("early repayment");

        assert_eq!(repayment.amount_paid, result.total_repayment);
        assert_eq!(
            game.teams[0].finance,
            finance_after_loan - result.total_repayment
        );
        assert!(game.teams[0].bank_loan.is_none());
        assert!(
            game.messages
                .iter()
                .any(|message| message.id == repayment.message_id)
        );
    }

    #[test]
    fn repay_bank_loan_requires_an_active_loan_and_sufficient_funds() {
        let mut game = make_game();
        game.teams[0].finance = 2_000_000;

        let error = repay_bank_loan(&mut game, "team1").expect_err("no loan to repay");
        assert_eq!(error, "be.error.finance.loanNotActive");

        request_bank_loan(&mut game, "team1").expect("loan");
        game.teams[0].finance = 0;

        let error = repay_bank_loan(&mut game, "team1").expect_err("cannot afford settlement");
        assert_eq!(error, "be.error.finance.loanRepaymentInsufficientFunds");
    }
}
