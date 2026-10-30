# Agency preparation exports

In **Payroll → Government**, generate the agency/month and choose **Export agency
preparation CSV**. This internal worksheet contains the chosen agency's registered
identifiers/name fields, revisions and immutable payroll sources. It is not an
agency upload file or official return. Keep source inclusion and summary totals
distinct; missing identity stays blank/flagged, unknown totals stay UNRESOLVED.

SSS uses SSS identifiers; PhilHealth uses PIN/PEN plus employee birth date/sex;
Pag-IBIG uses MID/employer identifiers; BIR uses TIN/branch and recorded nationality.
Other agencies' identifiers, personal contacts, registration evidence/references,
notes, actor identities and history are excluded. The older generic review CSV
remains free of registered identifiers. Classifications, salary bases, certificates
and filing declarations are not guessed. Earned contribution months and individual
BIR receipt dates retain their meanings; no source record is changed by exporting.

The server supplies one stable projection restricted to Payroll preparation responsibility. Native validation checks
company/person/source scope, canonical identifiers, duplicates and unexpected
fields. After rendering, it rechecks protected access before local file creation;
revocation prevents the write. CSV uses UTF-8/BOM, CRLF, quoted cells and formula
protection. Manager/ordinary Employee own-record access grants no exception.
Identifier leading zeros remain in the CSV bytes; import identifier columns as
text when using a spreadsheet. See [government Excel](GOVERNMENT-EXCEL.md) for native typed workbooks.

The [SSS reference draft](SSS-REFERENCE-DRAFT.md) provides a bounded internal review file
with current-source/readiness and explicit declaration checks. Twelve
fictional monthly vectors match inspected DTO/string methods from the currently
downloadable R3 package, whose base/update writer classes are identical. No SSS
installer, GUI, network helper or old rate-table validator was launched. Integer
cents, bounded widths, explicit declarations and retained namesakes are checked;
no silent truncation/transliteration occurs. Current agency semantics, reserved
slots, encoding and acceptance remain integration gates. See
[format review](GOVERNMENT-FILE-REVIEW.md). These preparation records do not establish official upload acceptance.
