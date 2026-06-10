import { beforeEach, describe, expect, it, vi } from "vitest";
import { invoke } from "@tauri-apps/api/core";

import {
    getFinanceSnapshot,
    repayBankLoan,
    requestBankLoan,
} from "./financeService";

vi.mock("@tauri-apps/api/core", () => ({
    invoke: vi.fn(),
}));

const mockedInvoke = vi.mocked(invoke);

function backendSnapshot() {
    return {
        annual_wage_bill: 88400,
        weekly_wage_spend: 1700,
        weekly_wage_budget: 38461,
        weekly_recurring_income: 5000,
        weekly_sponsor_income: 2000,
        weekly_merchandise_income: 3000,
        weekly_loan_repayment: 0,
        projected_weekly_net: 3300,
        cash_runway_weeks: null,
        wage_budget_usage_percent: 4,
        currently_in_debt: false,
        currently_over_budget: false,
        wage_budget_status: "stable",
        runway_status: "stable",
        overall_status: "stable",
        marketing_campaign_cooldown_days_remaining: 0,
    };
}

describe("financeService", () => {
    beforeEach(() => {
        mockedInvoke.mockReset();
    });

    it("maps the finance snapshot including merchandise and loan figures", async () => {
        mockedInvoke.mockResolvedValue({
            snapshot: backendSnapshot(),
            previews: {
                bank_loan: {
                    principal: 525000,
                    interest_rate_percent: 6,
                    term_weeks: 26,
                    weekly_repayment: 21404,
                    total_repayment: 556500,
                },
            },
        });

        const response = await getFinanceSnapshot("team-1");

        expect(mockedInvoke).toHaveBeenCalledWith("get_finance_snapshot", {
            teamId: "team-1",
        });
        expect(response.snapshot.weeklySponsorIncome).toBe(2000);
        expect(response.snapshot.weeklyMerchandiseIncome).toBe(3000);
        expect(response.snapshot.weeklyLoanRepayment).toBe(0);
        expect(response.snapshot.projectedWeeklyNet).toBe(3300);
        expect(response.previews.boardSupport).toBeNull();
        expect(response.previews.sponsorPitch).toBeNull();
        expect(response.previews.marketingCampaign).toBeNull();
        expect(response.previews.bankLoan).toEqual({
            principal: 525000,
            interestRatePercent: 6,
            termWeeks: 26,
            weeklyRepayment: 21404,
            totalRepayment: 556500,
        });
    });

    it("maps a missing bank loan preview to null", async () => {
        mockedInvoke.mockResolvedValue({
            snapshot: backendSnapshot(),
            previews: {},
        });

        const response = await getFinanceSnapshot();

        expect(mockedInvoke).toHaveBeenCalledWith("get_finance_snapshot", {
            teamId: null,
        });
        expect(response.previews.bankLoan).toBeNull();
    });

    it("requests a bank loan and maps the result", async () => {
        const game = { teams: [] };
        mockedInvoke.mockResolvedValue({
            game,
            result: {
                message_id: "bank_loan_approved_2026-02-16",
                principal: 525000,
                interest_rate_percent: 6,
                term_weeks: 26,
                weekly_repayment: 21404,
                total_repayment: 556500,
            },
        });

        const response = await requestBankLoan();

        expect(mockedInvoke).toHaveBeenCalledWith("request_bank_loan");
        expect(response.game).toBe(game);
        expect(response.result).toEqual({
            messageId: "bank_loan_approved_2026-02-16",
            principal: 525000,
            interestRatePercent: 6,
            termWeeks: 26,
            weeklyRepayment: 21404,
            totalRepayment: 556500,
        });
    });

    it("repays a bank loan and maps the result", async () => {
        const game = { teams: [] };
        mockedInvoke.mockResolvedValue({
            game,
            result: {
                message_id: "bank_loan_repaid_2026-03-02",
                amount_paid: 412500,
            },
        });

        const response = await repayBankLoan();

        expect(mockedInvoke).toHaveBeenCalledWith("repay_bank_loan");
        expect(response.game).toBe(game);
        expect(response.result).toEqual({
            messageId: "bank_loan_repaid_2026-03-02",
            amountPaid: 412500,
        });
    });
});
