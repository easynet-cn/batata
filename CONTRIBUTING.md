# Contributing

Contributions are welcome! Please feel free to submit issues and pull requests.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/amazing-feature`)
3. Commit your changes (`git commit -m 'Add some amazing feature'`)
4. Push to the branch (`git push origin feature/amazing-feature`)
5. Open a Pull Request

Build, test and release instructions are documented in
[`docs/development.md`](docs/development.md).

## Documentation maintenance rules

To keep the two READMEs (English and Chinese) short and accurate, please follow
these rules when adding or changing features:

1. **README is a landing page, not documentation.** It answers only: what it is,
   why to use it, how to get running fastest, and where to find details.
   Target size is ~200 lines.
2. **Single source of truth (SSOT).**
   - Configuration keys: `conf/application.yml` (fully commented).
   - API details: `docs/compat/`.
   - Everything else: the matching page under `docs/`.

   Never copy/paste that content into a README — link to it instead.
3. **English and Chinese READMEs share one skeleton.** Keep the same sections,
   the same order and the same anchors; translate, do not restructure.
4. **Adding a feature:** update `docs/` and the config comments first, then add
   *one* bullet plus *one* link in both READMEs. Do not expand detail in README.
