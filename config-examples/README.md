# Company deployment configuration

The public repository contains no working company connection. Copy the example
profile into your ignored `deployment-private/` folder and replace both values
with your own project's client configuration. The example is a template, not a
demo backend. Never use a secret/service-role key in an application package.

Each company deploys the versioned migrations and protected functions to its own
Supabase project. The deployment operator packages the publishable profile and
branding; employees do not select a database inside the applications. Access still
requires current Auth and protected membership. Backend deployment, company
bootstrap and initial account provisioning are separate operator responsibilities.

Android also needs that company's Firebase Android configuration in the ignored
`employee-android/app/google-services.json`, matching the retained application ID,
and a local signing identity. Do not copy somebody else's Firebase project or
signing key. Company-configured installers/APKs, profiles, keys and build output
stay local and are never uploaded with public source releases.

The existing private packaging helper uses the operator's installed Workforce HR
profile plus local target/signing guards. It intentionally fails without that
setup. Read its inputs before running it on another workstation. The native
desktop can be built without a profile for its clearly labelled fictional demo.
