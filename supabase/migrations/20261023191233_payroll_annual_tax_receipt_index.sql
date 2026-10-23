-- Cover the composite receipt-to-reconciliation foreign key. No record changes.
begin;
create index payroll_tax_slips_reconciliation_idx on public.payroll_tax_slips(company_id,id);
commit;
