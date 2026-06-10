import { invoke } from "@tauri-apps/api/core";

import type { GameStateData } from "../store/gameStore";

export type FinanceHealthLevelData =
    | "stable"
    | "watch"
    | "warning"
    | "critical";

interface BackendTeamFinanceSnapshotData {
    annual_wage_bill: number;
    weekly_wage_spend: number;
    weekly_wage_budget: number;
    weekly_recurring_income: number;
    weekly_sponsor_income: number;
    weekly_merchandise_income: number;
    weekly_loan_repayment: number;
    projected_weekly_net: number;
    cash_runway_weeks: number | null;
    wage_budget_usage_percent: number;
    currently_in_debt: boolean;
    currently_over_budget: boolean;
    wage_budget_status: FinanceHealthLevelData;
    runway_status: FinanceHealthLevelData;
    overall_status: FinanceHealthLevelData;
    marketing_campaign_cooldown_days_remaining: number;
}

interface BackendBoardSupportPreviewData {
    support_amount: number;
    transfer_budget_reduction: number;
    satisfaction_penalty: number;
}

interface BackendSponsorPitchPreviewData {
    sponsor_name: string;
    weekly_amount: number;
    duration_weeks: number;
}

interface BackendMarketingCampaignPreviewData {
    gross_revenue: number;
    campaign_cost: number;
    net_income: number;
    cooldown_days: number;
}

interface BackendBankLoanPreviewData {
    principal: number;
    interest_rate_percent: number;
    term_weeks: number;
    weekly_repayment: number;
    total_repayment: number;
}

interface BackendFinanceActionPreviewsData {
    board_support?: BackendBoardSupportPreviewData | null;
    sponsor_pitch?: BackendSponsorPitchPreviewData | null;
    marketing_campaign?: BackendMarketingCampaignPreviewData | null;
    bank_loan?: BackendBankLoanPreviewData | null;
}

interface BackendFinanceSnapshotResponseData {
    snapshot: BackendTeamFinanceSnapshotData;
    previews?: BackendFinanceActionPreviewsData | null;
}

export interface TeamFinanceSnapshotData {
    annualWageBill: number;
    weeklyWageSpend: number;
    weeklyWageBudget: number;
    weeklyRecurringIncome: number;
    weeklySponsorIncome: number;
    weeklyMerchandiseIncome: number;
    weeklyLoanRepayment: number;
    projectedWeeklyNet: number;
    cashRunwayWeeks: number | null;
    wageBudgetUsagePercent: number;
    currentlyInDebt: boolean;
    currentlyOverBudget: boolean;
    wageBudgetStatus: FinanceHealthLevelData;
    runwayStatus: FinanceHealthLevelData;
    overallStatus: FinanceHealthLevelData;
    marketingCampaignCooldownDaysRemaining: number;
}

export interface BoardSupportPreviewData {
    supportAmount: number;
    transferBudgetReduction: number;
    satisfactionPenalty: number;
}

export interface SponsorPitchPreviewData {
    sponsorName: string;
    weeklyAmount: number;
    durationWeeks: number;
}

export interface MarketingCampaignPreviewData {
    grossRevenue: number;
    campaignCost: number;
    netIncome: number;
    cooldownDays: number;
}

export interface BankLoanPreviewData {
    principal: number;
    interestRatePercent: number;
    termWeeks: number;
    weeklyRepayment: number;
    totalRepayment: number;
}

export interface FinanceRecoveryPreviewsData {
    boardSupport: BoardSupportPreviewData | null;
    sponsorPitch: SponsorPitchPreviewData | null;
    marketingCampaign: MarketingCampaignPreviewData | null;
    bankLoan: BankLoanPreviewData | null;
}

export interface FinanceSnapshotData {
    snapshot: TeamFinanceSnapshotData;
    previews: FinanceRecoveryPreviewsData;
}

function mapSnapshot(
    snapshot: BackendTeamFinanceSnapshotData,
): TeamFinanceSnapshotData {
    return {
        annualWageBill: snapshot.annual_wage_bill,
        weeklyWageSpend: snapshot.weekly_wage_spend,
        weeklyWageBudget: snapshot.weekly_wage_budget,
        weeklyRecurringIncome: snapshot.weekly_recurring_income,
        weeklySponsorIncome: snapshot.weekly_sponsor_income,
        weeklyMerchandiseIncome: snapshot.weekly_merchandise_income,
        weeklyLoanRepayment: snapshot.weekly_loan_repayment,
        projectedWeeklyNet: snapshot.projected_weekly_net,
        cashRunwayWeeks: snapshot.cash_runway_weeks,
        wageBudgetUsagePercent: snapshot.wage_budget_usage_percent,
        currentlyInDebt: snapshot.currently_in_debt,
        currentlyOverBudget: snapshot.currently_over_budget,
        wageBudgetStatus: snapshot.wage_budget_status,
        runwayStatus: snapshot.runway_status,
        overallStatus: snapshot.overall_status,
        marketingCampaignCooldownDaysRemaining:
            snapshot.marketing_campaign_cooldown_days_remaining,
    };
}

function mapPreviews(
    previews?: BackendFinanceActionPreviewsData | null,
): FinanceRecoveryPreviewsData {
    return {
        boardSupport: previews?.board_support
            ? {
                supportAmount: previews.board_support.support_amount,
                transferBudgetReduction:
                    previews.board_support.transfer_budget_reduction,
                satisfactionPenalty:
                    previews.board_support.satisfaction_penalty,
            }
            : null,
        sponsorPitch: previews?.sponsor_pitch
            ? {
                sponsorName: previews.sponsor_pitch.sponsor_name,
                weeklyAmount: previews.sponsor_pitch.weekly_amount,
                durationWeeks: previews.sponsor_pitch.duration_weeks,
            }
            : null,
        marketingCampaign: previews?.marketing_campaign
            ? {
                grossRevenue: previews.marketing_campaign.gross_revenue,
                campaignCost: previews.marketing_campaign.campaign_cost,
                netIncome: previews.marketing_campaign.net_income,
                cooldownDays: previews.marketing_campaign.cooldown_days,
            }
            : null,
        bankLoan: previews?.bank_loan
            ? mapBankLoanPreview(previews.bank_loan)
            : null,
    };
}

function mapBankLoanPreview(
    preview: BackendBankLoanPreviewData,
): BankLoanPreviewData {
    return {
        principal: preview.principal,
        interestRatePercent: preview.interest_rate_percent,
        termWeeks: preview.term_weeks,
        weeklyRepayment: preview.weekly_repayment,
        totalRepayment: preview.total_repayment,
    };
}

export async function getFinanceSnapshot(
    teamId?: string,
): Promise<FinanceSnapshotData> {
    const response = await invoke<BackendFinanceSnapshotResponseData>(
        "get_finance_snapshot",
        {
            teamId: teamId ?? null,
        },
    );

    return {
        snapshot: mapSnapshot(response.snapshot),
        previews: mapPreviews(response.previews),
    };
}

interface BackendBankLoanResultData {
    message_id: string;
    principal: number;
    interest_rate_percent: number;
    term_weeks: number;
    weekly_repayment: number;
    total_repayment: number;
}

interface BackendBankLoanResponseData {
    game: GameStateData;
    result: BackendBankLoanResultData;
}

interface BackendBankLoanRepaymentResultData {
    message_id: string;
    amount_paid: number;
}

interface BackendBankLoanRepaymentResponseData {
    game: GameStateData;
    result: BackendBankLoanRepaymentResultData;
}

export interface BankLoanActionResultData {
    messageId: string;
    principal: number;
    interestRatePercent: number;
    termWeeks: number;
    weeklyRepayment: number;
    totalRepayment: number;
}

export interface BankLoanActionResponseData {
    game: GameStateData;
    result: BankLoanActionResultData;
}

export interface BankLoanRepaymentResultData {
    messageId: string;
    amountPaid: number;
}

export interface BankLoanRepaymentResponseData {
    game: GameStateData;
    result: BankLoanRepaymentResultData;
}

export async function requestBankLoan(): Promise<BankLoanActionResponseData> {
    const response = await invoke<BackendBankLoanResponseData>(
        "request_bank_loan",
    );

    return {
        game: response.game,
        result: {
            messageId: response.result.message_id,
            principal: response.result.principal,
            interestRatePercent: response.result.interest_rate_percent,
            termWeeks: response.result.term_weeks,
            weeklyRepayment: response.result.weekly_repayment,
            totalRepayment: response.result.total_repayment,
        },
    };
}

export async function repayBankLoan(): Promise<BankLoanRepaymentResponseData> {
    const response = await invoke<BackendBankLoanRepaymentResponseData>(
        "repay_bank_loan",
    );

    return {
        game: response.game,
        result: {
            messageId: response.result.message_id,
            amountPaid: response.result.amount_paid,
        },
    };
}