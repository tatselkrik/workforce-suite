# Workforce Suite source and local deployment releases

The single `workforce-suite` repository is public source for Workforce HR,
Workforce Payroll and Workforce Employee. The native desktops share Rust code;
Android embeds the own-record library under the historical employee-portal path.
The standalone browser app is retired.

Apps advance independently with `hr-v…`, `payroll-v…` and `employee-v…` source tags.
Preserve application IDs, signing keys, installation/data paths and original pay,
receipt and personnel records. Windows numeric file versions differ from display
prereleases; Android versionCode must increase for every installed update.

Public releases provide generic source snapshots only. Company connection and
Firebase profiles, private keys, company files and configured installers/APKs stay
local in ignored deployment folders. Existing configured GitHub release files are
archived and hash-verified locally before being removed for the public transition;
source tags and history remain intact after review. Do not create a private remote.

Review current source, reachable history, screenshots and remote assets before
public publication. Use literal example configuration; never embed the operator's
backend or reuse another company's signing/provider configuration. Subsequent
verified source changes are committed/pushed to this same public repository.

CI uses standard no-charge public runners with read-only permissions, pinned
workflow actions and no artifact/cache uploads or deployment secrets. Private
events skip jobs. A local build, source tag, public release, CI result and actual
company installation are distinct facts and must be reported accurately.

Preserve locally installed originals and backups. New changed company packages
are built/signed/verified/installed locally; never relabel an old binary. The HR
stable updater catalog remains independently configured by its operator. There is
no new public auto-update channel, hosted employee portal or automatic SQL deploy.

README is a product introduction and app tour with reviewed fictional screenshots.
Keep detailed vulnerability findings in ignored local evidence, not in README.
