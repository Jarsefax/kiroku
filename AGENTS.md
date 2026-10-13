# Repository instructions

- Increase the app version for every completed change to the tool. Do not
  leave a completed change reporting the previous version.
- Use a minor version bump for new features and a patch bump for fixes or
  maintenance. Bump once per completed change, not for each intermediate edit.
- Keep the package version in `Cargo.toml` and Kiroku's entry in `Cargo.lock`
  in sync, and verify the rebuilt executable with `krk --version`.
- The monthly Markdown storage format version is independent of the app
  version; change it only when the storage format changes.
