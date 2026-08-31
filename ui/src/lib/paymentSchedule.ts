import type { PaymentFrequencyUnit, PaymentSubscription } from "$lib/api";

export const PAYMENT_CALENDAR_SOURCE_ID = "subscription-payments";
export const PAYMENT_CALENDAR_COLOR = "#FBBF24";

interface DateParts {
  year: number;
  month: number;
  day: number;
}

export interface PaymentFrequency {
  interval: number;
  unit: PaymentFrequencyUnit;
}

const millisecondsPerDay = 86_400_000;
const maximumOccurrencesPerRange = 5_000;

export function paymentFrequency(
  subscription: PaymentSubscription,
): PaymentFrequency | null {
  if (
    subscription.frequency_interval !== null &&
    subscription.frequency_interval >= 1 &&
    subscription.frequency_interval <= 999 &&
    subscription.frequency_unit !== null
  ) {
    return {
      interval: subscription.frequency_interval,
      unit: subscription.frequency_unit,
    };
  }

  const normalized = subscription.frequency.trim().toLowerCase();
  const preset: Record<string, PaymentFrequency> = {
    daily: { interval: 1, unit: "day" },
    weekly: { interval: 1, unit: "week" },
    monthly: { interval: 1, unit: "month" },
    quarterly: { interval: 3, unit: "month" },
    yearly: { interval: 1, unit: "year" },
    annually: { interval: 1, unit: "year" },
  };
  if (normalized in preset) return preset[normalized];

  const match = normalized.match(
    /^every\s+([1-9]\d{0,2})\s+(days?|weeks?|months?|years?)$/,
  );
  if (!match) return null;
  const interval = Number(match[1]);
  const unit = match[2]?.replace(/s$/, "") as PaymentFrequencyUnit | undefined;
  return unit ? { interval, unit } : null;
}

export function annualPaymentOccurrences(
  subscription: PaymentSubscription,
): number | null {
  const frequency = paymentFrequency(subscription);
  if (!frequency) return null;
  const yearlyOccurrences = {
    day: 365,
    week: 52,
    month: 12,
    year: 1,
  } satisfies Record<PaymentFrequencyUnit, number>;
  return yearlyOccurrences[frequency.unit] / frequency.interval;
}

export function nextPaymentDateKey(
  subscription: PaymentSubscription,
  fromDateKey = localDateKey(new Date()),
): string | null {
  const schedule = parseSchedule(subscription);
  const fromDate = parseDateKey(fromDateKey);
  if (!schedule || !fromDate) return null;
  const index = firstOccurrenceIndexOnOrAfter(schedule, fromDate);
  return occurrenceDateKey(schedule, index);
}

export function paymentDateKeysBetween(
  subscription: PaymentSubscription,
  startDateKey: string,
  endDateKey: string,
): string[] {
  if (endDateKey < startDateKey) return [];
  const schedule = parseSchedule(subscription);
  const start = parseDateKey(startDateKey);
  const end = parseDateKey(endDateKey);
  if (!schedule || !start || !end) return [];

  const dates: string[] = [];
  let index = firstOccurrenceIndexOnOrAfter(schedule, start);
  while (dates.length < maximumOccurrencesPerRange) {
    const candidate = occurrenceDateKey(schedule, index);
    if (!candidate || candidate > endDateKey) break;
    dates.push(candidate);
    index += 1;
  }
  return dates;
}

interface PaymentSchedule {
  anchor: DateParts;
  frequency: PaymentFrequency;
}

function parseSchedule(
  subscription: PaymentSubscription,
): PaymentSchedule | null {
  const anchor = parseDateKey(subscription.first_paid_on);
  const frequency = paymentFrequency(subscription);
  return anchor && frequency ? { anchor, frequency } : null;
}

function firstOccurrenceIndexOnOrAfter(
  schedule: PaymentSchedule,
  target: DateParts,
): number {
  const anchorKey = formatDateParts(schedule.anchor)!;
  const targetKey = formatDateParts(target)!;
  if (targetKey <= anchorKey) return 0;

  const { interval, unit } = schedule.frequency;
  if (unit === "day" || unit === "week") {
    const stepDays = interval * (unit === "week" ? 7 : 1);
    const elapsedDays = Math.floor(
      (datePartsToUtcTime(target) - datePartsToUtcTime(schedule.anchor)) /
        millisecondsPerDay,
    );
    return Math.max(0, Math.ceil(elapsedDays / stepDays));
  }

  const elapsedUnits =
    unit === "month"
      ? (target.year - schedule.anchor.year) * 12 +
        target.month -
        schedule.anchor.month
      : target.year - schedule.anchor.year;
  let index = Math.max(0, Math.floor(elapsedUnits / interval));
  while (occurrenceDateKey(schedule, index)! < targetKey) index += 1;
  while (index > 0 && occurrenceDateKey(schedule, index - 1)! >= targetKey) {
    index -= 1;
  }
  return index;
}

function occurrenceDateKey(
  schedule: PaymentSchedule,
  index: number,
): string | null {
  const { anchor, frequency } = schedule;
  if (!Number.isSafeInteger(index) || index < 0) return null;

  if (frequency.unit === "day" || frequency.unit === "week") {
    const dayOffset =
      index * frequency.interval * (frequency.unit === "week" ? 7 : 1);
    const date = new Date(
      datePartsToUtcTime(anchor) + dayOffset * millisecondsPerDay,
    );
    return formatUtcDate(date);
  }

  if (frequency.unit === "month") {
    const monthIndex =
      anchor.year * 12 + (anchor.month - 1) + index * frequency.interval;
    const year = Math.floor(monthIndex / 12);
    const month = (monthIndex % 12) + 1;
    return formatDateParts({
      year,
      month,
      day: Math.min(anchor.day, daysInMonth(year, month)),
    });
  }

  const year = anchor.year + index * frequency.interval;
  return formatDateParts({
    year,
    month: anchor.month,
    day: Math.min(anchor.day, daysInMonth(year, anchor.month)),
  });
}

function parseDateKey(value: string): DateParts | null {
  const match = /^(\d{4})-(\d{2})-(\d{2})$/.exec(value);
  if (!match) return null;
  const parts = {
    year: Number(match[1]),
    month: Number(match[2]),
    day: Number(match[3]),
  };
  const date = datePartsToUtcDate(parts);
  return date.getUTCFullYear() === parts.year &&
    date.getUTCMonth() + 1 === parts.month &&
    date.getUTCDate() === parts.day
    ? parts
    : null;
}

function datePartsToUtcDate(parts: DateParts): Date {
  const date = new Date(0);
  date.setUTCHours(0, 0, 0, 0);
  date.setUTCFullYear(parts.year, parts.month - 1, parts.day);
  return date;
}

function datePartsToUtcTime(parts: DateParts): number {
  return datePartsToUtcDate(parts).valueOf();
}

function daysInMonth(year: number, month: number): number {
  const date = new Date(0);
  date.setUTCHours(0, 0, 0, 0);
  date.setUTCFullYear(year, month, 0);
  return date.getUTCDate();
}

function formatUtcDate(date: Date): string | null {
  if (Number.isNaN(date.valueOf())) return null;
  return formatDateParts({
    year: date.getUTCFullYear(),
    month: date.getUTCMonth() + 1,
    day: date.getUTCDate(),
  });
}

function formatDateParts(parts: DateParts): string | null {
  if (parts.year < 0 || parts.year > 9_999) return null;
  return `${String(parts.year).padStart(4, "0")}-${String(parts.month).padStart(2, "0")}-${String(parts.day).padStart(2, "0")}`;
}

function localDateKey(date: Date): string {
  return `${String(date.getFullYear()).padStart(4, "0")}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(date.getDate()).padStart(2, "0")}`;
}
