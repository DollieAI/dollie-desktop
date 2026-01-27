# Windows Store Submission Guide

## Your Store Identity

```
Package/Identity/Name: Dollie.Dollie
Package/Identity/Publisher: CN=B569B1BF-FB02-4CDA-865B-F99702B1BB64
Publisher Display Name: Dollie
Package Family Name: Dollie.Dollie_p5nshxsqd5fd2
Store ID: 9PJFKHT66BHV
```

## Building for Windows Store

### Option 1: GitHub Actions (Recommended)

Set up automated builds that create Store-ready packages.

Add `.github/workflows/build-windows.yml`:

```yaml
name: Build Windows

on:
  push:
    tags:
      - 'v*'
  workflow_dispatch:

jobs:
  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4

      - name: Setup Node
        uses: actions/setup-node@v4
        with:
          node-version: 20

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Install dependencies
        run: |
          cd desktop-app
          npm install

      - name: Build Tauri
        run: |
          cd desktop-app
          npm run tauri build

      - name: Upload MSI
        uses: actions/upload-artifact@v4
        with:
          name: windows-msi
          path: desktop-app/src-tauri/target/release/bundle/msi/*.msi

      - name: Upload NSIS
        uses: actions/upload-artifact@v4
        with:
          name: windows-nsis
          path: desktop-app/src-tauri/target/release/bundle/nsis/*.exe
```

### Option 2: Build Locally on Windows

1. Install prerequisites:
   - [Node.js 20+](https://nodejs.org)
   - [Rust](https://rustup.rs)
   - [Visual Studio Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/)
     - Select "Desktop development with C++"

2. Clone and build:
   ```powershell
   cd desktop-app
   npm install
   npm run tauri build
   ```

3. Find outputs in:
   - `src-tauri/target/release/bundle/msi/Dollie_0.1.0_x64_en-US.msi`
   - `src-tauri/target/release/bundle/nsis/Dollie_0.1.0_x64-setup.exe`

## Submitting to Microsoft Store

### Step 1: Create MSIX Package

Microsoft Store requires MSIX format. Convert your MSI:

1. Download [MSIX Packaging Tool](https://www.microsoft.com/store/productId/9N5LW3JBCXKF) from Microsoft Store

2. Run the tool → "Application package" → "Create your package on this computer"

3. Select your MSI file

4. Fill in package information:
   - Package name: `Dollie.Dollie`
   - Publisher: `CN=B569B1BF-FB02-4CDA-865B-F99702B1BB64`
   - Publisher display name: `Dollie`
   - Version: Match your app version

5. Save the .msix file

### Step 2: Upload to Partner Center

1. Go to [Partner Center](https://partner.microsoft.com/dashboard)

2. Select your Dollie app

3. Go to **Product management** → **Packages**

4. Upload your .msix file

5. Complete the store listing:
   - Screenshots
   - Description
   - Category: Business
   - Privacy policy URL

6. Submit for certification

## Store Listing Content

### Description (suggested)

**Short description:**
Your Agency HQ for the AI Era

**Full description:**
Dollie is your agency headquarters - where all your agency knowledge connects to any AI tool.

- Manage clients, sites, and workflows from one place
- Native desktop experience with system notifications
- Quick access from your system tray
- Automatic dark/light mode support

Built for agencies who want to work smarter with AI.

### Category
Business

### Screenshots Required
- At least 1 screenshot at 1366x768 or larger

## Alternative: Direct Distribution

If you want to distribute outside the Store (with SmartScreen warnings):

1. Build the NSIS installer
2. Host on your website
3. Users will see "Windows protected your PC" on first run
4. They click "More info" → "Run anyway"

For trusted distribution without warnings, you'd need a code signing certificate (~$200-400/year).
