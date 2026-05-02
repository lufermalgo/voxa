# Code Signing & Notarization

Voxa is currently **not code-signed or notarized**. This document explains why, what it means for users, and how to set it up if you have an Apple Developer account.

## Current Status

Voxa is distributed as an unsigned `.dmg`. This causes two friction points for users:

1. **"App is damaged" error** — macOS Gatekeeper blocks apps downloaded from the internet that aren't signed. Users must run `xattr -cr /Applications/Voxa.app` to remove the quarantine flag.
2. **Accessibility permission issues** — macOS ties Accessibility permissions to the binary's code signature hash. Without a stable signature, permissions can break after updates and need to be re-granted manually.

## Why It's Not Signed

Code signing requires an Apple Developer Program membership ($99 USD/year). This is an open-source project without funding, so we don't currently have one.

## What Users Need to Do

See the [Installation section in README.md](../README.md#installation) for the full steps. In short:

```bash
# Remove quarantine flag after downloading
xattr -cr /Applications/Voxa.app
```

Then grant Accessibility permission in System Settings → Privacy & Security → Accessibility.

## How to Enable Code Signing

If you have an Apple Developer account and want to contribute signed builds:

### 1. Generate Certificates

1. Go to [developer.apple.com/account](https://developer.apple.com/account)
2. Navigate to Certificates, Identifiers & Profiles → Certificates
3. Create a **"Developer ID Application"** certificate
4. Download and install it in your Keychain
5. Export it as a `.p12` file with a password

### 2. Create an App-Specific Password

1. Go to [appleid.apple.com](https://appleid.apple.com/account/manage)
2. Sign-In and Security → App-Specific Passwords
3. Generate a new password for "Voxa CI"

### 3. Find Your Team ID

1. Go to [developer.apple.com/account](https://developer.apple.com/account)
2. Membership Details → Team ID (a 10-character alphanumeric string)

### 4. Configure GitHub Secrets

Add these secrets to the repository (Settings → Secrets and variables → Actions):

| Secret | Value |
|--------|-------|
| `APPLE_CERTIFICATE` | Base64-encoded `.p12` certificate (`base64 -i certificate.p12`) |
| `APPLE_CERTIFICATE_PASSWORD` | Password used when exporting the `.p12` |
| `APPLE_ID` | Your Apple ID email |
| `APPLE_PASSWORD` | The app-specific password from step 2 |
| `APPLE_TEAM_ID` | Your 10-character Team ID |

### 5. Update the Release Workflow

Replace the build and release steps in `.github/workflows/release.yml`:

```yaml
      - name: Build Tauri app (signed + notarized)
        env:
          GITHUB_TOKEN: ${{ secrets.GITHUB_TOKEN }}
          APPLE_CERTIFICATE: ${{ secrets.APPLE_CERTIFICATE }}
          APPLE_CERTIFICATE_PASSWORD: ${{ secrets.APPLE_CERTIFICATE_PASSWORD }}
          APPLE_SIGNING_IDENTITY: "Developer ID Application: Your Name (TEAM_ID)"
          APPLE_ID: ${{ secrets.APPLE_ID }}
          APPLE_PASSWORD: ${{ secrets.APPLE_PASSWORD }}
          APPLE_TEAM_ID: ${{ secrets.APPLE_TEAM_ID }}
        uses: tauri-apps/tauri-action@v0
        with:
          tagName: ${{ github.ref_name }}
          releaseName: 'Voxa ${{ github.ref_name }}'
          releaseDraft: false
          prerelease: false
          args: --target aarch64-apple-darwin
```

Tauri's build tooling automatically handles code signing and notarization when these environment variables are set. The `tauri-action` will:

1. Import the certificate into a temporary keychain
2. Sign the `.app` bundle with `codesign`
3. Submit to Apple's notary service via `xcrun notarytool`
4. Staple the notarization ticket to the `.dmg`

### 6. What Changes for Users

Once signed and notarized:

- No more "app is damaged" error
- No need to run `xattr -cr`
- Accessibility permissions persist across updates (same code signature)
- macOS shows "Voxa is from an identified developer" instead of blocking

## Verifying the Current Build

Users can verify the `.dmg` integrity using the SHA-256 hash shown on the GitHub release page:

```bash
shasum -a 256 ~/Downloads/Voxa_1.2.0_aarch64.dmg
```

Compare the output with the digest listed on the release.

## References

- [Tauri Code Signing Guide](https://tauri.app/distribute/sign/macos/)
- [Apple Developer ID Documentation](https://developer.apple.com/developer-id/)
- [Apple Notarization Documentation](https://developer.apple.com/documentation/security/notarizing-macos-software-before-distribution)
