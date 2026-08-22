# Packaging and Distribution

Expanse uses an automated GitHub Actions workflow to publish packages whenever a new version tag (e.g., `v0.1.0`) is pushed.

## Release Process
1. Update versions in all `Cargo.toml` files.
2. Update the CHANGELOG.
3. Commit and push: `git commit -m "Release vX.Y.Z" && git tag vX.Y.Z && git push origin vX.Y.Z`
4. The `.github/workflows/release.yml` workflow will automatically:
   - Publish `expanse-trie` and `expanse-capi` to crates.io using OIDC trusted publishing.
   - Build native libraries (`.so`, `.dylib`, `.dll`, `.a`, `.lib`) for multiple targets.
   - Build a `.deb` package for Linux.
   - Create a GitHub Release with all the binary artifacts.

## Package Managers

### Cargo
Available as `expanse-trie` and `expanse-capi` on crates.io.

### vcpkg
A `vcpkg` port is available in `extra/vcpkg`. To install it using vcpkg locally, you can use the overlay-ports feature.

### NuGet
For Windows C++ users, an `expanse.nuspec` is provided in `extra/nuget`. You can build a local nuget package with `nuget pack extra/nuget/expanse.nuspec`.

### Pkg-config
For Linux/macOS users, `expanse.pc.in` and `judy.pc.in` templates are available in `extra/pkgconfig/` to integrate with build systems using pkg-config.
