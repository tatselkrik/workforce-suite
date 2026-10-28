# Focused HRIS workspaces and optional desktop sections

This bounded milestone following Account Manager0.1.63 is delivered/installed
in0.1.64. See [the delivery record](WORKSPACES-0.1.64-DELIVERY.md).
Payroll remains inside HRIS during this increment. No Payroll extraction, new HR
case/service/benefits module or portal redesign is included.

## Navigation

Overview remains first. Workspaces show the person's assigned responsibilities;
My work groups personal/team attendance, leave, payslips and assigned hiring or
clearance tasks. Administration retains Account Manager for Administrators and
Settings. The circular Profile stays at the bottom and notices remain accessible.

HR/Admin and Managers retain Employees with Directory, optional Organization chart
and one combined Development section. A Development specialist enters that same
employee workspace at Development. Recruiters enter Hiring; Payroll Officers enter
Payroll. Combined responsibilities retain their union. Other staff retain personal
development and their own published statements, subject to optional visibility.
Current protected role/own/team/assigned scopes remain the authority.

## Company controls

Administrators use Settings > Workspaces for Organization chart, Development,
Hiring, Exit & clearance, Payroll & payslips and Reports. All remain visible by
default. Core navigation, personal profile, notices and Administration remain
available within existing permissions. Settings apply to this dedicated company's
native desktop, not the separate portal.

Visibility changes presentation only. They do not grant/revoke a role, alter or
delete source records, stop payroll calculations/approvals, change balances or
remove portal access. Hidden desktop sections can be shown again. Existing clients
and domain RPC/RLS/Storage contracts remain compatible.

A change arriving while someone edits keeps that draft and explains the section
is hidden. After saving/cancelling, the route falls back to an available workspace.
Source changes retain Administrator choices for explicit review. Pending saves keep
one exact identity/payload; confirmed rejections release the pending marker but
retain choices. Success requires the matching source-bound receipt and fresh read.

## Protected contract

The additive visibility migration stores only a private company configuration and
immutable private audit/receipts. Current active desktop staff read a minimal
company/person-bound context; only protected active Administrators save. Public
RPC wrappers are invoker-only. Direct table access, anonymous callers and ordinary
Employees are denied. The company lock, revision and actor/full-payload receipt
guard concurrent edits, exact retries and atomic audit rollback.

An exact missing-function response on older servers preserves prior all-visible
settings with editing unavailable. Auth, network, malformed, wrong-scope and future
contract failures never silently become defaults. Credentials remain in memory.

## Verification and delivery

Use focused native role/module routing, stale/retry/receipt and real HTTP checks;
local database replay/permission/preservation checks; fictional native screenshots;
then guarded hosted checks and exact signed installer/recovery/private publication/
actual installation. No financial rule, dependency, subscription or hardware is added.
Pause work and the same scheduler after this delivered milestone.
