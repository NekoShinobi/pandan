<script lang="ts">
  import CalendarClock from "lucide-svelte/icons/calendar-clock";
  import CreditCard from "lucide-svelte/icons/credit-card";
  import Pencil from "lucide-svelte/icons/pencil";
  import Plus from "lucide-svelte/icons/plus";
  import Search from "lucide-svelte/icons/search";
  import Trash2 from "lucide-svelte/icons/trash-2";
  import X from "lucide-svelte/icons/x";
  import { onMount, tick } from "svelte";
  import TypedHeading from "$lib/TypedHeading.svelte";
  import {
    createPaymentSubscription,
    deletePaymentSubscription,
    fetchPaymentSubscriptions,
    updatePaymentSubscription,
    type PaymentSubscription,
    type PaymentSubscriptionInput,
  } from "$lib/api";

  const frequencySuggestions = [
    { label: "Daily", occurrences: 365 },
    { label: "Weekly", occurrences: 52 },
    { label: "Every 2 weeks", occurrences: 26 },
    { label: "Monthly", occurrences: 12 },
    { label: "Every 2 months", occurrences: 6 },
    { label: "Quarterly", occurrences: 4 },
    { label: "Every 6 months", occurrences: 2 },
    { label: "Yearly", occurrences: 1 },
  ] as const;
  const currencySuggestions = [
    "USD",
    "EUR",
    "GBP",
    "CAD",
    "AUD",
    "JPY",
    "CHF",
    "NZD",
    "SEK",
    "NOK",
    "DKK",
  ];
  const microsPerUnit = 1_000_000;
  const maxAmountMicros = 1_000_000_000_000;

  let subscriptions = $state.raw<PaymentSubscription[]>([]);
  let loading = $state(true);
  let pageError = $state("");
  let query = $state("");
  let dialog = $state<HTMLDialogElement>();
  let serviceInput = $state<HTMLInputElement>();
  let editingId = $state<string | null>(null);
  let service = $state("");
  let description = $state("");
  let frequency = $state("Monthly");
  let amount = $state("");
  let currency = $state("USD");
  let firstPaidOn = $state("");
  let formError = $state("");
  let saving = $state(false);
  let deleteId = $state("");

  let filteredSubscriptions = $derived.by(() => {
    const needle = query.trim().toLowerCase();
    if (!needle) return subscriptions;
    return subscriptions.filter((subscription) =>
      [
        subscription.service,
        subscription.description,
        subscription.frequency,
        subscription.currency,
        subscription.first_paid_on,
      ].some((value) => value.toLowerCase().includes(needle)),
    );
  });
  let costTotals = $derived.by(() => {
    const totals: Record<string, number> = {};
    for (const subscription of subscriptions) {
      const occurrences = annualOccurrences(subscription.frequency);
      if (occurrences === null || subscription.amount_micros <= 0) continue;
      totals[subscription.currency] =
        (totals[subscription.currency] ?? 0) +
        subscription.amount_micros * occurrences;
    }
    return Object.entries(totals)
      .map(([totalCurrency, yearlyMicros]) => ({
        currency: totalCurrency,
        daily: Math.round(yearlyMicros / 365),
        weekly: Math.round(yearlyMicros / 52),
        monthly: Math.round(yearlyMicros / 12),
        yearly: yearlyMicros,
      }))
      .sort((left, right) => left.currency.localeCompare(right.currency));
  });
  let excludedCostCount = $derived(
    subscriptions.filter(
      (subscription) =>
        subscription.amount_micros > 0 &&
        annualOccurrences(subscription.frequency) === null,
    ).length,
  );

  onMount(() => {
    void loadSubscriptions();
  });

  async function loadSubscriptions() {
    loading = true;
    pageError = "";
    try {
      subscriptions = await fetchPaymentSubscriptions();
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error
          ? reason.message
          : "Unable to load subscriptions";
    } finally {
      loading = false;
    }
  }

  function captureDialog(node: HTMLDialogElement) {
    dialog = node;
    return () => {
      dialog = undefined;
    };
  }

  function captureServiceInput(node: HTMLInputElement) {
    serviceInput = node;
    return () => {
      serviceInput = undefined;
    };
  }

  async function openCreate() {
    editingId = null;
    service = "";
    description = "";
    frequency = "Monthly";
    amount = "";
    currency = "USD";
    firstPaidOn = new Date().toISOString().slice(0, 10);
    formError = "";
    dialog?.showModal();
    await tick();
    serviceInput?.focus();
  }

  function openEdit(subscription: PaymentSubscription) {
    editingId = subscription.id;
    service = subscription.service;
    description = subscription.description;
    frequency = subscription.frequency;
    amount = amountInputValue(subscription.amount_micros);
    currency = subscription.currency;
    firstPaidOn = subscription.first_paid_on;
    formError = "";
    deleteId = "";
    dialog?.showModal();
  }

  async function save(event: SubmitEvent) {
    event.preventDefault();
    if (saving) return;
    const amountMicros = parseAmountMicros(amount);
    if (amountMicros === null) {
      formError =
        "Enter a valid non-negative amount with up to six decimal places.";
      return;
    }
    const normalizedCurrency = currency.trim().toUpperCase();
    if (!/^[A-Z]{3}$/.test(normalizedCurrency)) {
      formError = "Currency must be a three-letter code such as USD or EUR.";
      return;
    }
    const input: PaymentSubscriptionInput = {
      service: service.trim(),
      description: description.trim(),
      frequency: frequency.trim(),
      amount_micros: amountMicros,
      currency: normalizedCurrency,
      first_paid_on: firstPaidOn,
    };
    saving = true;
    formError = "";
    try {
      const saved = editingId
        ? await updatePaymentSubscription(editingId, input)
        : await createPaymentSubscription(input);
      subscriptions = editingId
        ? subscriptions.map((item) => (item.id === saved.id ? saved : item))
        : [...subscriptions, saved].sort((left, right) =>
            left.service.localeCompare(right.service),
          );
      dialog?.close();
    } catch (reason: unknown) {
      formError =
        reason instanceof Error
          ? reason.message
          : "Unable to save subscription";
    } finally {
      saving = false;
    }
  }

  async function remove(subscription: PaymentSubscription) {
    if (saving) return;
    if (deleteId !== subscription.id) {
      deleteId = subscription.id;
      return;
    }
    saving = true;
    pageError = "";
    try {
      await deletePaymentSubscription(subscription.id);
      subscriptions = subscriptions.filter(
        (item) => item.id !== subscription.id,
      );
      deleteId = "";
    } catch (reason: unknown) {
      pageError =
        reason instanceof Error
          ? reason.message
          : "Unable to remove subscription";
    } finally {
      saving = false;
    }
  }

  function formatDate(value: string) {
    const date = new Date(`${value}T12:00:00`);
    return Number.isNaN(date.valueOf())
      ? value
      : new Intl.DateTimeFormat("en", {
          month: "short",
          day: "numeric",
          year: "numeric",
        }).format(date);
  }

  function annualOccurrences(value: string): number | null {
    const normalized = value.trim().toLowerCase();
    return (
      frequencySuggestions.find(
        (suggestion) => suggestion.label.toLowerCase() === normalized,
      )?.occurrences ?? null
    );
  }

  function parseAmountMicros(value: string): number | null {
    const normalized = value.trim().replaceAll(",", "");
    if (!/^\d{1,10}(?:\.\d{1,6})?$/.test(normalized)) return null;
    const [whole, fraction = ""] = normalized.split(".");
    const micros =
      Number(whole) * microsPerUnit + Number(fraction.padEnd(6, "0"));
    return Number.isSafeInteger(micros) && micros <= maxAmountMicros
      ? micros
      : null;
  }

  function amountInputValue(micros: number) {
    const whole = Math.floor(micros / microsPerUnit);
    const fraction = String(micros % microsPerUnit).padStart(6, "0");
    const trimmed = fraction.replace(/0+$/, "");
    return trimmed ? `${whole}.${trimmed.padEnd(2, "0")}` : `${whole}.00`;
  }

  function formatMoney(micros: number, code: string) {
    const value = micros / microsPerUnit;
    try {
      return new Intl.NumberFormat("en", {
        style: "currency",
        currency: code,
        currencyDisplay: "narrowSymbol",
        minimumFractionDigits: 2,
        maximumFractionDigits: value > 0 && value < 0.01 ? 4 : 2,
      }).format(value);
    } catch {
      return `${code} ${value.toFixed(2)}`;
    }
  }
</script>

<section class="subscriptions-page product-page" data-od-id="subscriptions-page">
  <header class="subscriptions-header page-header">
    <div>
      <TypedHeading
        text="$ subscriptions --list"
        odId="subscriptions-heading"
      />
      <p>Keep a private record of services that bill on a regular cadence.</p>
    </div>
    <button class="ui-button ui-button--primary subscriptions-primary" type="button" onclick={openCreate}>
      <Plus size={16} strokeWidth={1.8} aria-hidden="true" /> Add subscription
    </button>
  </header>

  {#if pageError}<p class="subscriptions-error" role="alert">
      {pageError}
    </p>{/if}

  <section
    class="cost-summary"
    aria-labelledby="cost-summary-title"
    data-od-id="subscription-cost-summary"
  >
    <header>
      <div>
        <span>[ COST.NORMALIZED ]</span>
        <h3 id="cost-summary-title">Recurring spend</h3>
      </div>
      <p>
        Annualized from each billing cadence. Currencies stay separate.
        {#if excludedCostCount > 0}
          {excludedCostCount} custom {excludedCostCount === 1
            ? "cadence is"
            : "cadences are"} excluded.
        {/if}
      </p>
    </header>
    {#if costTotals.length > 0}
      <div class="cost-summary-table">
        <div class="cost-summary-labels" aria-hidden="true">
          <span>Currency</span><span>Per day</span><span>Per week</span><span
            >Per month</span
          ><span>Per year</span>
        </div>
        {#each costTotals as total (total.currency)}
          <div
            class="cost-summary-row"
            data-od-id={`subscription-total-${total.currency.toLowerCase()}`}
          >
            <strong>{total.currency}</strong>
            <span
              ><small>Day</small>{formatMoney(
                total.daily,
                total.currency,
              )}</span
            >
            <span
              ><small>Week</small>{formatMoney(
                total.weekly,
                total.currency,
              )}</span
            >
            <span
              ><small>Month</small>{formatMoney(
                total.monthly,
                total.currency,
              )}</span
            >
            <span class="cost-year"
              ><small>Year</small>{formatMoney(
                total.yearly,
                total.currency,
              )}</span
            >
          </div>
        {/each}
      </div>
    {:else}
      <p class="cost-summary-empty">
        Add a cost to a subscription to calculate your recurring spend.
      </p>
    {/if}
  </section>

  <div class="subscriptions-toolbar">
    <label>
      <Search size={15} strokeWidth={1.8} aria-hidden="true" />
      <span class="sr-only">Filter subscriptions</span>
      <input
        type="search"
        bind:value={query}
        placeholder="Filter service, frequency, or date…"
      />
    </label>
    <span>{filteredSubscriptions.length} / {subscriptions.length} services</span
    >
  </div>

  <section
    class="subscriptions-table"
    aria-label="Recurring payment subscriptions"
  >
    <header>
      <span>Service</span><span>Description</span><span>Cost</span><span
        >Frequency</span
      ><span>First paid</span><span>Actions</span>
    </header>
    {#if loading}
      <div class="subscriptions-status">Loading subscriptions…</div>
    {:else}
      {#each filteredSubscriptions as subscription (subscription.id)}
        <article>
          <div class="service-cell">
            <span class="service-icon"
              ><CreditCard size={17} strokeWidth={1.6} /></span
            ><strong>{subscription.service}</strong>
          </div>
          <p>{subscription.description || "No description"}</p>
          <div class="cost-cell">
            <strong
              >{formatMoney(
                subscription.amount_micros,
                subscription.currency,
              )}</strong
            >
            <span>per billing period</span>
          </div>
          <div class="frequency-cell">
            <CalendarClock size={14} strokeWidth={1.7} /><span
              >{subscription.frequency}</span
            >
          </div>
          <time datetime={subscription.first_paid_on}
            >{formatDate(subscription.first_paid_on)}</time
          >
          <div class="row-actions">
            <button
              type="button"
              aria-label={`Edit ${subscription.service}`}
              onclick={() => openEdit(subscription)}
              ><Pencil size={14} /></button
            >
            <button
              class="ui-button ui-button--danger ui-button--icon"
              class:confirm={deleteId === subscription.id}
              type="button"
              aria-label={`Delete ${subscription.service}`}
              onclick={() => remove(subscription)}><Trash2 size={14} /></button
            >
          </div>
        </article>
      {:else}
        <div class="subscriptions-empty">
          <CreditCard size={28} strokeWidth={1.4} aria-hidden="true" />
          <h3>
            {query ? "No matching subscriptions" : "No recurring payments yet"}
          </h3>
          <p>
            {query
              ? "Try a broader search."
              : "Add a service when you want to remember its payment cadence."}
          </p>
          {#if !query}<button type="button" onclick={openCreate}
              >Add your first subscription</button
            >{/if}
        </div>
      {/each}
    {/if}
  </section>

  <dialog
    class="settings-dialog subscription-dialog"
    {@attach captureDialog}
    onclick={(event) => event.target === dialog && dialog?.close()}
  >
    <header>
      <div>
        <span>[ SUBSCRIPTION.EDIT ]</span>
        <h2>{editingId ? "Edit subscription" : "Add subscription"}</h2>
      </div>
      <button class="ui-button ui-button--ghost ui-button--icon" type="button" aria-label="Close" onclick={() => dialog?.close()}
        ><X size={18} /></button
      >
    </header>
    <form onsubmit={save}>
      <div class="subscription-form-scroll">
        <label for="subscription-service">Service</label>
        <input
          id="subscription-service"
          bind:value={service}
          {@attach captureServiceInput}
          maxlength="120"
          placeholder="Service name"
          required
        />

        <label for="subscription-description">Description</label>
        <textarea
          id="subscription-description"
          bind:value={description}
          maxlength="2000"
          rows="4"
          placeholder="What this subscription covers"></textarea>

        <div class="form-grid subscription-cost-fields">
          <div>
            <label for="subscription-amount">Cost per billing period</label>
            <input
              data-od-id="subscription-cost-input"
              id="subscription-amount"
              type="text"
              inputmode="decimal"
              bind:value={amount}
              placeholder="0.00"
              autocomplete="off"
              required
            />
          </div>
          <div>
            <label for="subscription-currency">Currency</label>
            <input
              data-od-id="subscription-currency-input"
              id="subscription-currency"
              value={currency}
              list="currency-options"
              maxlength="3"
              placeholder="USD"
              autocomplete="off"
              required
              oninput={(event) =>
                (currency = event.currentTarget.value
                  .toUpperCase()
                  .replace(/[^A-Z]/g, "")
                  .slice(0, 3))}
            />
            <datalist id="currency-options"
              >{#each currencySuggestions as option (option)}<option
                  value={option}
                ></option>{/each}</datalist
            >
          </div>
        </div>

        <div class="form-grid">
          <div>
            <label for="subscription-frequency">Frequency</label>
            <select
              data-od-id="subscription-frequency-input"
              id="subscription-frequency"
              bind:value={frequency}
              required
            >
              {#if !frequencySuggestions.some((option) => option.label === frequency)}
                <option value={frequency}>{frequency} (custom)</option>
              {/if}
              {#each frequencySuggestions as option (option.label)}
                <option value={option.label}>{option.label}</option>
              {/each}
            </select>
          </div>
          <div>
            <label for="subscription-first-date">First date paid</label>
            <input
              id="subscription-first-date"
              type="date"
              bind:value={firstPaidOn}
              required
            />
          </div>
        </div>

        {#if formError}<p class="subscriptions-form-error" role="alert">
            {formError}
          </p>{/if}
      </div>
      <footer>
        <button class="ui-button ui-button--secondary" type="button" onclick={() => dialog?.close()}>Cancel</button
        ><button class="ui-button ui-button--primary subscriptions-primary" type="submit" disabled={saving}
          >{saving
            ? "Saving…"
            : editingId
              ? "Save changes"
              : "Add subscription"}</button
        >
      </footer>
    </form>
  </dialog>
</section>

<style>
  .subscriptions-page {
    display: grid;
    gap: 18px;
    padding: clamp(24px, 3vw, 42px);
    min-width: 0;
  }
  .subscriptions-header {
    display: flex;
    align-items: end;
    justify-content: space-between;
    gap: 24px;
    padding-bottom: 18px;
    border-bottom: 1px solid var(--border);
  }
  .subscription-dialog header span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.09em;
  }
  .subscriptions-header p {
    margin: 7px 0 0;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  button,
  input,
  textarea,
  select {
    font: inherit;
  }
  button {
    color: inherit;
  }
  .subscriptions-primary {
    display: inline-flex;
    min-height: 42px;
    align-items: center;
    gap: 8px;
    border: 1px solid var(--fg);
    background: var(--fg);
    color: var(--bg);
    padding: 0 16px;
    font-family: var(--font-mono);
    font-size: 11px;
    letter-spacing: 0.04em;
  }
  .subscriptions-primary:hover {
    background: transparent;
    color: var(--fg);
  }
  .subscriptions-error,
  .subscriptions-form-error {
    margin: 0;
    border: 1px solid oklch(60% 0.16 25 / 0.5);
    background: oklch(20% 0.04 25 / 0.75);
    padding: 10px 12px;
    color: oklch(82% 0.09 25);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .cost-summary {
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 92%, transparent);
  }
  .cost-summary > header {
    display: flex;
    min-height: 64px;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
    padding: 12px 16px;
    border-bottom: 1px solid var(--border);
  }
  .cost-summary > header span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.09em;
  }
  .cost-summary h3 {
    margin: 3px 0 0;
    font-family: var(--font-mono);
    font-size: 16px;
    font-weight: 550;
  }
  .cost-summary > header p,
  .cost-summary-empty {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .cost-summary-table {
    display: grid;
  }
  .cost-summary-labels,
  .cost-summary-row {
    display: grid;
    grid-template-columns: 90px repeat(4, minmax(120px, 1fr));
    align-items: center;
    gap: 12px;
    padding-inline: 16px;
  }
  .cost-summary-labels {
    min-height: 34px;
    border-bottom: 1px solid var(--border);
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.07em;
    text-transform: uppercase;
  }
  .cost-summary-row {
    min-height: 66px;
    border-bottom: 1px solid var(--border);
    font-family: var(--font-mono);
  }
  .cost-summary-row:last-child {
    border-bottom: 0;
  }
  .cost-summary-row > strong {
    color: var(--muted);
    font-size: 11px;
    letter-spacing: 0.08em;
  }
  .cost-summary-row > span {
    display: grid;
    gap: 3px;
    font-size: 14px;
    font-weight: 550;
  }
  .cost-summary-row small {
    display: none;
    color: var(--muted);
    font-size: 9px;
    font-weight: 450;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }
  .cost-summary-row .cost-year {
    color: var(--accent);
  }
  .cost-summary-empty {
    margin: 0;
    padding: 24px 16px;
  }
  .subscriptions-toolbar {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }
  .subscriptions-toolbar label {
    display: flex;
    width: min(520px, 100%);
    min-height: 42px;
    align-items: center;
    gap: 9px;
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 90%, transparent);
    padding: 0 12px;
  }
  .subscriptions-toolbar input {
    width: 100%;
    border: 0;
    outline: 0;
    background: transparent;
    color: var(--fg);
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .subscriptions-toolbar > span {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .subscriptions-table {
    border: 1px solid var(--border);
    background: color-mix(in oklch, var(--page-surface, var(--surface)) 92%, transparent);
  }
  .subscriptions-table > header,
  .subscriptions-table > article {
    display: grid;
    grid-template-columns:
      minmax(160px, 1.1fr) minmax(200px, 1.5fr) minmax(120px, 0.75fr)
      minmax(120px, 0.75fr) minmax(120px, 0.75fr) 82px;
    align-items: center;
    gap: 16px;
  }
  .subscriptions-table > header {
    min-height: 42px;
    border-bottom: 1px solid var(--border);
    padding: 0 16px;
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 9px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }
  .subscriptions-table > article {
    min-height: 72px;
    border-bottom: 1px solid var(--border);
    padding: 12px 16px;
  }
  .subscriptions-table > article:last-child {
    border-bottom: 0;
  }
  .service-cell {
    display: flex;
    min-width: 0;
    align-items: center;
    gap: 10px;
  }
  .service-icon {
    display: grid;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    place-items: center;
    border: 1px solid var(--border);
  }
  .service-cell strong {
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 13px;
    font-weight: 550;
  }
  .subscriptions-table article p {
    display: -webkit-box;
    overflow: hidden;
    margin: 0;
    color: var(--muted);
    font-size: 11px;
    line-height: 1.5;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }
  .cost-cell {
    display: grid;
    gap: 2px;
    font-family: var(--font-mono);
  }
  .cost-cell strong {
    font-size: 12px;
    font-weight: 550;
  }
  .cost-cell span {
    color: var(--muted);
    font-size: 9px;
  }
  .frequency-cell {
    display: flex;
    align-items: center;
    gap: 7px;
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .subscriptions-table time {
    color: var(--muted);
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .row-actions {
    display: flex;
    justify-content: flex-end;
    gap: 5px;
  }
  .row-actions button {
    display: grid;
    width: 34px;
    height: 34px;
    place-items: center;
    border: 1px solid var(--border);
    background: transparent;
  }
  .row-actions button:hover {
    border-color: var(--fg);
  }
  .row-actions button.confirm {
    border-color: oklch(62% 0.19 25);
    color: oklch(72% 0.16 25);
  }
  .subscriptions-status {
    padding: 36px;
    color: var(--muted);
    text-align: center;
    font-family: var(--font-mono);
    font-size: 11px;
  }
  .subscriptions-empty {
    display: grid;
    min-height: 330px;
    place-content: center;
    justify-items: center;
    padding: 36px;
    text-align: center;
  }
  .subscriptions-empty h3 {
    margin: 16px 0 6px;
    font-family: var(--font-mono);
    font-size: 16px;
    font-weight: 550;
  }
  .subscriptions-empty p {
    max-width: 45ch;
    margin: 0;
    color: var(--muted);
    font-size: 12px;
    line-height: 1.6;
  }
  .subscriptions-empty button {
    min-height: 40px;
    margin-top: 18px;
    border: 1px solid var(--border);
    background: transparent;
    padding: 0 14px;
    font-family: var(--font-mono);
    font-size: 10px;
  }
  .subscription-dialog {
    width: min(620px, calc(100vw - 32px));
    max-height: min(820px, calc(100dvh - 32px));
    overflow: hidden;
    border: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    color: var(--fg);
    padding: 0;
  }
  .subscription-dialog[open] {
    display: flex;
    flex-direction: column;
  }
  .subscription-dialog::backdrop {
    background: oklch(5% 0 0 / 0.7);
    backdrop-filter: blur(5px);
  }
  .subscription-dialog header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 18px 20px;
    border-bottom: 1px solid var(--border);
  }
  .subscription-dialog h2 {
    margin: 6px 0 0;
    font-family: var(--font-mono);
    font-size: 20px;
    font-weight: 550;
  }
  .subscription-dialog header button {
    width: 36px;
    height: 36px;
    border: 1px solid var(--border);
    background: transparent;
  }
  .subscription-dialog form {
    min-height: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .subscription-form-scroll {
    min-height: 0;
    flex: 1;
    display: grid;
    gap: 9px;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-gutter: stable;
    padding: 20px;
  }
  .subscription-dialog label {
    font-family: var(--font-mono);
    font-size: 10px;
    letter-spacing: 0.04em;
  }
  .subscription-dialog input,
  .subscription-dialog textarea,
  .subscription-dialog select {
    width: 100%;
    border: 1px solid var(--border);
    background: var(--bg);
    color: var(--fg);
    padding: 11px 12px;
  }
  .subscription-dialog input,
  .subscription-dialog select {
    min-height: 44px;
  }
  .subscription-dialog textarea {
    resize: vertical;
    line-height: 1.5;
  }
  .form-grid {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 12px;
    margin-top: 6px;
  }
  .form-grid > div {
    display: grid;
    gap: 8px;
  }
  .subscription-dialog footer {
    flex: 0 0 auto;
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    border-top: 1px solid var(--border);
    background: var(--page-surface, var(--surface));
    padding: 16px 20px;
  }
  .subscription-dialog footer > button:not(.subscriptions-primary) {
    min-height: 42px;
    border: 1px solid var(--border);
    background: transparent;
    padding: 0 16px;
  }
  .sr-only {
    position: absolute;
    width: 1px;
    height: 1px;
    overflow: hidden;
    clip: rect(0 0 0 0);
    white-space: nowrap;
  }
  :focus-visible {
    outline: 2px solid var(--fg);
    outline-offset: 2px;
  }
  @media (max-width: 900px) {
    .subscriptions-table > header {
      display: none;
    }
    .subscriptions-table > article {
      grid-template-columns: 1fr auto;
      gap: 10px;
    }
    .subscriptions-table article p,
    .cost-cell,
    .frequency-cell,
    .subscriptions-table time {
      grid-column: 1;
    }
    .row-actions {
      grid-column: 2;
      grid-row: 1 / span 4;
    }
  }
  @media (max-width: 640px) {
    .subscriptions-header {
      align-items: stretch;
      flex-direction: column;
    }
    .subscriptions-primary {
      justify-content: center;
    }
    .subscriptions-toolbar {
      align-items: stretch;
      flex-direction: column;
    }
    .form-grid {
      grid-template-columns: 1fr;
    }
    .cost-summary > header {
      align-items: flex-start;
      flex-direction: column;
    }
    .cost-summary-labels {
      display: none;
    }
    .cost-summary-row {
      grid-template-columns: repeat(2, minmax(0, 1fr));
      gap: 16px;
      padding-block: 14px;
    }
    .cost-summary-row > strong {
      grid-column: 1 / -1;
    }
    .cost-summary-row small {
      display: block;
    }
  }
</style>
