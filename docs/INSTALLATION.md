# Installation Guide

## macOS

### 1. Download

Download the latest `.dmg` from [GitHub Releases](https://github.com/mvacoimbra/where-is-teemo/releases).

### 2. Install

Open the `.dmg` and drag **Where Is Teemo** to your Applications folder.

### 3. Bypass Gatekeeper

The app is not code-signed with an Apple Developer certificate, so macOS will block it on first launch with an **"Apple can't check app for malicious software"** warning.

To open it anyway:

1. Try to open the app normally — macOS will show the warning and refuse to open it
2. Go to **Apple menu > System Settings > Privacy & Security**
3. Scroll down to the **Security** section — you'll see a message about "Where Is Teemo" being blocked
4. Click **Open Anyway**
5. Enter your login password and click **OK**

After this, the app is saved as an exception and you can open it normally from now on.

### 4. Trust the CA Certificate

On first launch, Where Is Teemo generates a local CA certificate to intercept the Riot client's TLS connection. You'll be prompted to trust it in your macOS Keychain — enter your password to allow it.

This certificate is only used locally for the XMPP proxy and is never shared.

## Windows

### 1. Download

Download the latest `.msi` or `.exe` installer from [GitHub Releases](https://github.com/mvacoimbra/where-is-teemo/releases).

### 2. Install

Run the installer. Windows SmartScreen may show a warning since the app is unsigned — click **More info** then **Run anyway**.

### 3. Trust the CA Certificate

On first launch, the app generates a local CA certificate. You may see a Windows Security prompt asking to install it — click **Yes** to allow.
