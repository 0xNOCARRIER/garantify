# Releasing

1. Make sure `main` is green (CI passing)
2. Tag the commit: `git tag -a v1.2.3 -m "Release 1.2.3"`
3. Push the tag: `git push origin v1.2.3`
4. GitHub Actions will build and publish:
   - `ghcr.io/0xnocarrier/garantify:1.2.3`
   - `ghcr.io/0xnocarrier/garantify:1.2`
   - `ghcr.io/0xnocarrier/garantify:1`
   - `ghcr.io/0xnocarrier/garantify:latest`
5. Create a GitHub Release from the tag (optional but recommended for changelog visibility)

Use semantic versioning:
- **MAJOR**: breaking changes (DB schema changes requiring manual intervention, config renames)
- **MINOR**: new features, backward-compatible
- **PATCH**: bug fixes
