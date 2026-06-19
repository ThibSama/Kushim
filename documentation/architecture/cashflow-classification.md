# Cash-flow Classification

Status: **Approved contract, not yet implemented.** Paired with
[portfolio-performance-contract.md](portfolio-performance-contract.md). This
document is the per-operation canonical matrix used by the historical
performance read model to classify economic effects.

## Source of truth

The 14 operation types and their structural rules are defined by
`portfolio_operations` in `infra/postgres/init/001_init.sql` (`CHECK
chk_portfolio_operations_*` family). This document does not change those
rules; it assigns a **financial classification** to each type.

## Sign conventions

Every "impact" column uses the following conventions:

| Impact | Positive | Negative |
| --- | --- | --- |
| `changes cash` | cash balance goes up | cash balance goes down |
| `changes quantity` | a held position's quantity goes up | a held position's quantity goes down |
| `changes cost basis` | aggregate cost basis goes up | aggregate cost basis goes down |
| `external flow` | external contribution `CF_i > 0` | external withdrawal `CF_i < 0` |
| `realized P&L impact` | a non-zero amount may be persisted to the daily `realized_gain_minor` | none |
| `unrealized P&L impact` | the day's unrealized recomputation may change | none |
| `income impact` | non-zero increment to the day's `income_minor` | none |
| `fee impact` | non-zero increment to the day's `fees_minor` | none |
| `tax impact` | non-zero increment to the day's `taxes_minor` | none |

"`performance support status`" is one of:

- `supported` — the operation is fully classified and contributes
  deterministically to the historical contract;
- `supported_metadata` — the operation is a structural transformation
  (split, spin-off, symbol-change) with deterministic semantics but no
  cash, no realized P&L and no external flow;
- `unsupported_until_subtype` — the operation does not have unambiguous
  financial semantics yet and triggers
  `daily_return_status = 'unavailable'` with
  `daily_return_unavailable_reason = 'unsupported_operation_semantics'`
  for the affected day. **`aggregate_valuation_status` is unchanged** —
  it continues to reflect only the per-position market-data / FX axes
  (see
  [portfolio-performance-contract.md, section 21](portfolio-performance-contract.md#21-ambiguous-operations));
- `case_dependent` — supported when the reconstruction rule of section 23
  applies, otherwise `unsupported_until_subtype` (only `adjustment` falls
  here).

## Canonical matrix

| operation type | financial category | changes cash | changes quantity | changes cost basis | external flow | realized P&L impact | unrealized P&L impact | income impact | fee impact | tax impact | performance support status | ambiguity / required subtype |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| `buy` | asset acquisition | `−` `gross_amount_minor` | `+` `quantity` | `+` `gross + fees + taxes` (in base) | `0` | no | recomputed on revaluation | no | yes (`buy.fees_minor`) | yes (`buy.taxes_minor`) | `supported` | none |
| `sell` | asset disposal | `+` `gross_amount_minor` | `−` `quantity` | `−` prorata reduction (in base) | `0` | yes (proceeds − cost reduction − fees − taxes) | recomputed on remaining quantity | no | yes (`sell.fees_minor`) | yes (`sell.taxes_minor`) | `supported` | none |
| `deposit` | external contribution | `+` `gross_amount_minor` | none | none | `+` (sign `+`) | no | no | no | no | no | `supported` | none |
| `withdrawal` | external withdrawal | `−` `gross_amount_minor` | none | none | `−` (sign `−`) | no | no | no | no | no | `supported` | none |
| `dividend` | income | `+` `gross_amount_minor` | none | none | `0` (never an external contribution) | no | recomputed on revaluation | yes | no | no (withholding tax not silently inferred — see note 1) | `supported` | distinct dividend withholding tax remains explicit |
| `interest` | income | `+` `gross_amount_minor` | none | none | `0` | no | no | yes | no | no | `supported` | none |
| `fee` | fee | `−` `gross_amount_minor` | none | none | `0` (not an external flow) | no | no | no | yes | no | `supported` | none |
| `tax` | tax | `−` `gross_amount_minor` | none | none | `0` (not an external flow) | no | no | no | no | yes | `supported` | dividend withholding tax kept distinct from generic transaction tax (see note 1) |
| `split` | non-financial metadata | `0` | recomputed via split ratio | none (preserved) | `0` | no | recomputed on revaluation | no | no | no | `supported_metadata` | none for current ratio model |
| `spin_off` | non-financial metadata | `0` | `+` `related_quantity` on related asset | redistribution (rule TBD) | `0` | no | recomputed | no | no | no | `unsupported_until_subtype` | redistribution policy missing |
| `symbol_change` | non-financial metadata | `0` | transferred to related asset | preserved across identity change | `0` | no | recomputed | no | no | no | `unsupported_until_subtype` | identity-mapping policy missing |
| `transfer_in` | external or internal — TBD | `+` `gross_amount_minor` if cash, none if asset | `+` `quantity` if asset, none if cash | `+` if asset (cost = market@date — TBD) | **ambiguous** (see note 2) | case-dependent | case-dependent | no | no | no | `unsupported_until_subtype` | explicit subtype + counterparty required |
| `transfer_out` | external or internal — TBD | `−` `gross_amount_minor` if cash, none if asset | `−` `quantity` if asset, none if cash | `−` if asset (cost prorata) | **ambiguous** | case-dependent | case-dependent | no | no | no | `unsupported_until_subtype` | explicit subtype + counterparty required |
| `adjustment` | correction | payload-dependent | payload-dependent | payload-dependent | payload-dependent (must inherit the corrected operation's category) | payload-dependent (must avoid double counting) | payload-dependent | payload-dependent | payload-dependent | payload-dependent | `case_dependent` (see note 3) | reconstruction must collapse the original + adjustment into a single corrected event |

Notes:

1. **Dividend withholding tax.** A `dividend` operation's
   `gross_amount_minor` is taken as the **cash actually received**. The MVP
   does not silently infer a withholding tax. If a dividend was net of
   withholding, the gross/net distinction must be carried by a separate
   `tax` operation or by a future extension of the operation contract. The
   `tax` operation is a generic transaction tax category; it is **not**
   mechanically marked as withholding.
2. **Transfers.** Until an explicit transfer subtype + counterparty
   information (linked portfolio, linked account, external broker) is
   added to the operation contract, `transfer_in` and `transfer_out` are
   classified as ambiguous. Encountering one of them on a day sets
   `daily_return_status = 'unavailable'` with reason
   `unsupported_operation_semantics`. **It does not change
   `aggregate_valuation_status`**, which continues to reflect only the
   per-position market-data / FX axes. A day with full market-data
   coverage and an ambiguous transfer has
   `aggregate_valuation_status = 'complete'` and
   `daily_return_status = 'unavailable'`. Valuation is still produced
   (positions and cash are reconstructed); only performance is
   suppressed. See
   [portfolio-performance-contract.md, section 21](portfolio-performance-contract.md#21-ambiguous-operations).
3. **Adjustments.** An adjustment is **not** a new independent economic
   event. The historical reconstruction must:
   - resolve `id_corrected_operation` chains back to the original posted
     operation;
   - derive the corrected economic timeline as if the original had carried
     the corrected payload from the start;
   - never count the original payload *and* the adjustment payload
     independently.
   When the adjustment shape provides enough information for the
   reconstruction to apply this rule deterministically, the operation is
   classified `supported` for that day. Otherwise the day is recorded
   with `daily_return_status = 'unavailable'` and
   `daily_return_unavailable_reason = 'unsupported_operation_semantics'`
   (`aggregate_valuation_status` is unchanged — it remains driven by the
   per-position axes). The boundary is defined in section 23 of the
   performance contract.

## Treatment of buy/sell fees and taxes

The matrix above intentionally splits the role of `fees_minor` and
`taxes_minor` on buy/sell:

- they **change cost basis** (buy adds them to `invested_base_minor`; sell
  subtracts them from net proceeds before realized P&L);
- they **simultaneously** increment the day's `fees_minor` and `taxes_minor`
  aggregates so the reporting columns show the true expense.

This is **not** double counting on NAV: the buy/sell already moved cash by
its `cash_amount_minor` (which includes fees and taxes); the aggregates are
*reporting* columns, not re-applied to NAV. The same value is shown in two
correct places:

- as part of the position's cost basis (where it justifies a higher
  acquisition price);
- as part of the daily expense aggregate (where it justifies what the
  user paid in fees / taxes that day).

`Σ fees_minor` over a period therefore answers "how much did the user pay
in fees this period?", regardless of whether the fee was standalone or
embedded in a trade.

## External flows — the only supported set

The MVP recognizes external flows from **two** operation types only:

- `deposit` → `CF_i > 0`;
- `withdrawal` → `CF_i < 0`.

Every other operation type has `external flow = 0` in the Modified Dietz
formula. In particular:

- buys and sells move cash but **internally** (cash ↔ position);
- income operations move cash **internally** (the cash came from the
  position, conceptually);
- fees and taxes move cash but they are **expenses**, not external flows;
- transfers are **ambiguous** until subtyped (see note 2).

This list is exhaustive. Adding a new external flow source requires
updating both this document and section 7 of the performance contract.

## Performance-impact summary

| Impact on portfolio performance | Operation types |
| --- | --- |
| Positive contribution to daily return (without being a flow) | `dividend`, `interest`, position revaluation |
| Negative contribution to daily return (without being a flow) | `fee`, `tax`, position revaluation downward, `buy.fees_minor`, `buy.taxes_minor`, `sell.fees_minor`, `sell.taxes_minor` (via reduced proceeds) |
| Treated as external flow (excluded from numerator beyond V_end − V_begin, weighted in denominator) | `deposit`, `withdrawal` |
| No effect on daily return by themselves | `split`, `spin_off`, `symbol_change` (revaluation happens through the resulting position) |
| Set `daily_return_status = 'unavailable'` (reason `unsupported_operation_semantics`) for the affected day; `aggregate_valuation_status` unchanged | `transfer_in`, `transfer_out`, ambiguous `adjustment` |

## Future operation extensions tracked here

For traceability, the following operation contract extensions are required
to unlock more operations as `supported`:

- explicit `transfer_kind ∈ {asset_in_kind, cash_in_kind, internal_account_move, ...}`;
- counterparty field (`linked_portfolio_id`, `linked_account_ref`);
- explicit `withholding_tax_minor` on `dividend`;
- explicit redistribution rule for `spin_off` (cost-basis split);
- explicit identity-mapping rule for `symbol_change`;
- explicit adjustment shape vocabulary to constrain payloads to those the
  reconstruction can collapse safely.

These extensions are not part of this PR. They are tracked in
[../mvp/deferred-todos.md](../mvp/deferred-todos.md).
